//! Windows process-loopback capture and active render-session inventory.
//!
//! The application list is derived from active WASAPI render sessions. Capture
//! itself targets the selected process tree through Windows process loopback;
//! it never falls back to the machine-wide render mix.

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LiveCaptureApplication {
    pub(crate) process_id: u32,
    pub(crate) name: String,
}

#[cfg(windows)]
mod windows_impl {
    use std::collections::BTreeMap;
    use std::collections::VecDeque;
    use std::mem::size_of;
    use std::path::Path;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::mpsc;
    use std::sync::Arc;
    use std::thread;
    use std::time::{Duration, Instant};

    use wasapi::{
        initialize_mta, AudioClient, DeviceEnumerator, Direction, SampleType, SessionState,
        StreamMode, WaveFormat,
    };
    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
    use windows_sys::Win32::System::SystemInformation::OSVERSIONINFOW;
    use windows_sys::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_QUERY_LIMITED_INFORMATION,
    };

    use crate::live_meeting::{
        cut_capture_chunk, process_capture_sample_batch, CaptureEvent, CaptureReady,
        CaptureSampleBatch, CHANNEL_SYSTEM, MIN_FINAL_CHUNK_MS,
    };

    use super::LiveCaptureApplication;

    const MIN_PROCESS_LOOPBACK_BUILD: u32 = 20_348;
    const PROCESS_LOOPBACK_INCLUDE_TREE: bool = true;
    const APP_CAPTURE_FORMAT_RATE: u32 = 48_000;
    const APP_CAPTURE_CHANNELS: usize = 2;
    const APP_CAPTURE_WAIT_MS: u32 = 100;

    #[link(name = "ntdll")]
    unsafe extern "system" {
        fn RtlGetVersion(version_info: *mut OSVERSIONINFOW) -> i32;
    }

    fn windows_build_number() -> Result<u32, String> {
        let mut version = OSVERSIONINFOW {
            dwOSVersionInfoSize: size_of::<OSVERSIONINFOW>() as u32,
            ..Default::default()
        };
        // RtlGetVersion returns the actual kernel build and is not affected by
        // the application compatibility manifest used by GetVersionExW.
        let status = unsafe { RtlGetVersion(&mut version) };
        if status < 0 {
            return Err(format!("อ่านเวอร์ชัน Windows ไม่สำเร็จ (NTSTATUS {status:#x})"));
        }
        Ok(version.dwBuildNumber)
    }

    pub(crate) fn capture_supported() -> Result<bool, String> {
        Ok(windows_build_number()? >= MIN_PROCESS_LOOPBACK_BUILD)
    }

    fn process_image_name(process_id: u32) -> Option<String> {
        let handle: HANDLE =
            unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, process_id) };
        if handle.is_null() {
            return None;
        }
        let mut buffer = vec![0u16; 32_768];
        let mut length = buffer.len() as u32;
        let success =
            unsafe { QueryFullProcessImageNameW(handle, 0, buffer.as_mut_ptr(), &mut length) } != 0;
        unsafe { CloseHandle(handle) };
        if !success || length == 0 {
            return None;
        }
        let path = String::from_utf16_lossy(&buffer[..length as usize]);
        Path::new(&path)
            .file_name()
            .and_then(|name| name.to_str())
            .map(str::to_string)
    }

    fn session_name(display_name: String, identifier: String, process_id: u32) -> String {
        if !display_name.trim().is_empty() {
            return display_name.trim().to_string();
        }
        if let Some(name) = process_image_name(process_id) {
            return name;
        }
        if let Some(name) = Path::new(&identifier.replace('\\', "/"))
            .file_name()
            .and_then(|name| name.to_str())
            .filter(|name| !name.is_empty())
        {
            return name.to_string();
        }
        format!("แอปเสียง PID {process_id}")
    }

    pub(crate) fn active_audio_applications() -> Result<Vec<LiveCaptureApplication>, String> {
        initialize_mta()
            .ok()
            .map_err(|error| format!("เริ่ม Windows audio service ไม่สำเร็จ ({error})"))?;
        let enumerator = DeviceEnumerator::new()
            .map_err(|error| format!("อ่านรายการ audio endpoint ไม่สำเร็จ: {error}"))?;
        let devices = enumerator
            .get_device_collection(&Direction::Render)
            .map_err(|error| format!("อ่าน audio endpoint ขาออกไม่สำเร็จ: {error}"))?;
        let mut applications = BTreeMap::<u32, String>::new();
        for device in &devices {
            let Ok(device) = device else { continue };
            let Ok(manager) = device.get_iaudiosessionmanager() else {
                continue;
            };
            let Ok(sessions) = manager.get_audiosessionenumerator() else {
                continue;
            };
            let Ok(count) = sessions.get_count() else {
                continue;
            };
            for index in 0..count {
                let Ok(session) = sessions.get_session(index) else {
                    continue;
                };
                if session.get_state().ok() != Some(SessionState::Active) {
                    continue;
                }
                let Ok(process_id) = session.get_process_id() else {
                    continue;
                };
                if process_id == 0 {
                    continue;
                }
                let display_name = session.get_display_name().unwrap_or_default();
                let identifier = session.get_session_identifier().unwrap_or_default();
                let name = session_name(display_name, identifier, process_id);
                applications
                    .entry(process_id)
                    .and_modify(|existing| {
                        if existing.starts_with("แอปเสียง PID ") && !name.starts_with("แอปเสียง PID ")
                        {
                            *existing = name.clone();
                        }
                    })
                    .or_insert(name);
            }
        }
        Ok(applications
            .into_iter()
            .map(|(process_id, name)| LiveCaptureApplication { process_id, name })
            .collect())
    }

    pub(crate) fn validate_active_audio_application(
        process_id: u32,
    ) -> Result<LiveCaptureApplication, String> {
        if !capture_supported()? {
            return Err(format!(
                "การจับเสียงรายแอปต้องใช้ Windows build {MIN_PROCESS_LOOPBACK_BUILD} ขึ้นไป"
            ));
        }
        active_audio_applications()?
            .into_iter()
            .find(|application| application.process_id == process_id)
            .ok_or_else(|| "แอปที่เลือกไม่มีเสียงออกแล้ว — รีเฟรชรายการแล้วเลือกใหม่".to_string())
    }

    pub(crate) fn spawn_application_capture_thread(
        process_id: u32,
        application_name: String,
        stop: Arc<AtomicBool>,
        chunk_tx: mpsc::SyncSender<CaptureEvent>,
        chunks_dir: std::path::PathBuf,
        fragment_ms: u64,
    ) -> Result<CaptureReady, String> {
        let (ready_tx, ready_rx) = mpsc::channel::<Result<String, String>>();
        thread::Builder::new()
            .name(format!("fung-audio-app-{process_id}"))
            .spawn(move || {
                let mut readiness = Some(ready_tx);
                let result = capture_application_loopback(
                    process_id,
                    &application_name,
                    stop,
                    &chunk_tx,
                    &chunks_dir,
                    fragment_ms,
                    &mut readiness,
                );
                if let Some(ready) = readiness {
                    let message = result
                        .as_ref()
                        .err()
                        .cloned()
                        .unwrap_or_else(|| "การจับเสียงแอปหยุดก่อนเริ่ม".to_string());
                    let _ = ready.send(Err(message));
                } else if let Err(error) = result {
                    let _ = chunk_tx.send(CaptureEvent::StreamFailed {
                        channel: CHANNEL_SYSTEM,
                        error,
                    });
                }
            })
            .map_err(|error| format!("เปิด thread จับเสียงแอปไม่สำเร็จ: {error}"))?;

        match ready_rx.recv_timeout(Duration::from_secs(10)) {
            Ok(Ok(device_name)) => Ok(CaptureReady { device_name }),
            Ok(Err(error)) => Err(error),
            Err(error) => Err(format!("เตรียมจับเสียงแอปไม่ทันเวลา: {error}")),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn capture_application_loopback(
        process_id: u32,
        application_name: &str,
        stop: Arc<AtomicBool>,
        chunk_tx: &mpsc::SyncSender<CaptureEvent>,
        chunks_dir: &Path,
        fragment_ms: u64,
        readiness: &mut Option<mpsc::Sender<Result<String, String>>>,
    ) -> Result<(), String> {
        initialize_mta()
            .ok()
            .map_err(|error| format!("เริ่ม Windows audio service ไม่สำเร็จ ({error})"))?;
        let desired_format = WaveFormat::new(
            16,
            16,
            &SampleType::Int,
            APP_CAPTURE_FORMAT_RATE as usize,
            APP_CAPTURE_CHANNELS,
            None,
        );
        let mut client =
            AudioClient::new_application_loopback_client(process_id, PROCESS_LOOPBACK_INCLUDE_TREE)
                .map_err(|error| format!("เปิด process loopback ไม่สำเร็จ: {error}"))?;
        let mode = StreamMode::EventsShared {
            autoconvert: true,
            buffer_duration_hns: 0,
        };
        client
            .initialize_client(&desired_format, &Direction::Capture, &mode)
            .map_err(|error| format!("ตั้งค่า stream ของแอปไม่สำเร็จ: {error}"))?;
        let event = client
            .set_get_eventhandle()
            .map_err(|error| format!("สร้าง event ของ stream แอปไม่สำเร็จ: {error}"))?;
        let capture = client
            .get_audiocaptureclient()
            .map_err(|error| format!("เปิด audio capture client ของแอปไม่สำเร็จ: {error}"))?;
        client
            .start_stream()
            .map_err(|error| format!("เริ่ม stream ของแอปไม่สำเร็จ: {error}"))?;
        if let Some(ready) = readiness.take() {
            let _ = ready.send(Ok(application_name.to_string()));
        }

        let sample_rate = APP_CAPTURE_FORMAT_RATE;
        let block_align = APP_CAPTURE_CHANNELS * 2;
        let samples_per_chunk = (sample_rate as u64 * fragment_ms / 1000) as usize;
        let min_final_samples = (sample_rate as u64 * MIN_FINAL_CHUNK_MS / 1000) as usize;
        let mut accumulator = Vec::with_capacity(samples_per_chunk + 4096);
        let mut timeline_samples = 0u64;
        let mut expected_sample = 0u64;
        let mut local_seq = 0u32;
        let mut last_level_event = Instant::now();
        let mut bytes = VecDeque::<u8>::new();
        let mut stream_error = None;

        while !stop.load(Ordering::SeqCst) {
            if let Err(error) = event.wait_for_event(APP_CAPTURE_WAIT_MS) {
                if stop.load(Ordering::SeqCst) {
                    break;
                }
                stream_error = Some(format!("รอสัญญาณเสียงจากแอปไม่สำเร็จ: {error}"));
                break;
            }

            loop {
                let packet_frames = capture
                    .get_next_packet_size()
                    .map_err(|error| format!("อ่านขนาด packet จากแอปไม่สำเร็จ: {error}"))?
                    .unwrap_or(0);
                if packet_frames == 0 {
                    break;
                }
                capture
                    .read_from_device_to_deque(&mut bytes)
                    .map_err(|error| format!("อ่าน audio packet จากแอปไม่สำเร็จ: {error}"))?;
                let usable_bytes = bytes.len() - bytes.len() % block_align;
                let mut mono = Vec::with_capacity(usable_bytes / block_align);
                for _ in 0..(usable_bytes / block_align) {
                    let left = i16::from_le_bytes([
                        bytes.pop_front().unwrap_or(0),
                        bytes.pop_front().unwrap_or(0),
                    ]) as i32;
                    let right = i16::from_le_bytes([
                        bytes.pop_front().unwrap_or(0),
                        bytes.pop_front().unwrap_or(0),
                    ]) as i32;
                    mono.push(((left + right) / 2) as i16);
                }
                if mono.is_empty() {
                    continue;
                }
                let batch = CaptureSampleBatch {
                    first_sample: expected_sample,
                    samples: mono,
                };
                process_capture_sample_batch(
                    batch,
                    CHANNEL_SYSTEM,
                    sample_rate,
                    samples_per_chunk,
                    chunks_dir,
                    chunk_tx,
                    &mut accumulator,
                    &mut timeline_samples,
                    &mut expected_sample,
                    &mut local_seq,
                    &mut last_level_event,
                );
            }
        }

        let _ = client.stop_stream();
        if accumulator.len() >= min_final_samples {
            let take = accumulator.len();
            cut_capture_chunk(
                CHANNEL_SYSTEM,
                sample_rate,
                chunks_dir,
                chunk_tx,
                &mut accumulator,
                &mut timeline_samples,
                &mut local_seq,
                take,
            );
        } else if !accumulator.is_empty() {
            let start_ms = (timeline_samples * 1000 / sample_rate as u64) as i64;
            let end_ms =
                ((timeline_samples + accumulator.len() as u64) * 1000 / sample_rate as u64) as i64;
            if end_ms > start_ms {
                let _ = chunk_tx.send(CaptureEvent::SourceGap {
                    channel: CHANNEL_SYSTEM,
                    start_ms,
                    end_ms,
                    reason: "application_loopback_tail_too_short",
                });
            }
        }
        if let Some(error) = stream_error {
            return Err(error);
        }
        Ok(())
    }
}

#[cfg(windows)]
pub(crate) use windows_impl::{
    active_audio_applications, capture_supported, spawn_application_capture_thread,
    validate_active_audio_application,
};

#[cfg(not(windows))]
pub(crate) fn capture_supported() -> Result<bool, String> {
    Ok(false)
}

#[cfg(not(windows))]
pub(crate) fn active_audio_applications() -> Result<Vec<LiveCaptureApplication>, String> {
    Err("การจับเสียงรายแอปใช้ได้เฉพาะบน Windows".to_string())
}

#[cfg(not(windows))]
pub(crate) fn validate_active_audio_application(
    _process_id: u32,
) -> Result<LiveCaptureApplication, String> {
    Err("การจับเสียงรายแอปใช้ได้เฉพาะบน Windows".to_string())
}

#[cfg(not(windows))]
pub(crate) fn spawn_application_capture_thread(
    _process_id: u32,
    _application_name: String,
    _stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
    _chunk_tx: std::sync::mpsc::SyncSender<crate::live_meeting::CaptureEvent>,
    _chunks_dir: std::path::PathBuf,
    _fragment_ms: u64,
) -> Result<crate::live_meeting::CaptureReady, String> {
    Err("การจับเสียงรายแอปใช้ได้เฉพาะบน Windows".to_string())
}
