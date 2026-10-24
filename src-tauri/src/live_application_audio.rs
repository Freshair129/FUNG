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
    use windows::Win32::Media::Audio::{
        IAudioCaptureClient, AUDCLNT_BUFFERFLAGS_DATA_DISCONTINUITY, AUDCLNT_BUFFERFLAGS_SILENT,
        AUDCLNT_BUFFERFLAGS_TIMESTAMP_ERROR,
    };
    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
    use windows_sys::Win32::System::SystemInformation::OSVERSIONINFOW;
    use windows_sys::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_QUERY_LIMITED_INFORMATION,
    };

    use crate::live_meeting::{
        cut_capture_chunk, process_capture_sample_batch_with_gap_reason, CaptureEvent,
        CaptureReady, CaptureSampleBatch, CHANNEL_SYSTEM,
    };

    use super::LiveCaptureApplication;

    const MIN_PROCESS_LOOPBACK_BUILD: u32 = 20_348;
    const PROCESS_LOOPBACK_INCLUDE_TREE: bool = true;
    const APP_CAPTURE_FORMAT_RATE: u32 = 48_000;
    const APP_CAPTURE_CHANNELS: usize = 2;
    const APP_CAPTURE_WAIT_MS: u32 = 100;
    // At most 256 independently bounded (<= 2 s) PCM packets: <= 49.2 MB.
    // With measured 10 ms virtual packets this buffers 2.56 s of writer stalls.
    const APP_WRITER_QUEUE_PACKETS: usize = 256;

    fn application_capture_mode() -> StreamMode {
        // The virtual client ignores larger polling buffer requests. Preserve
        // supported event mode and keep all disk/event work off its reader.
        StreamMode::EventsShared {
            autoconvert: true,
            buffer_duration_hns: 0,
        }
    }

    struct ApplicationCaptureApartment;

    impl Drop for ApplicationCaptureApartment {
        fn drop(&mut self) {
            wasapi::deinitialize();
        }
    }

    /// The pointer must describe the PCM16 stereo packet just acquired from
    /// WASAPI unless SILENT is set, in which case Windows permits a null pointer.
    /// Release runs even when validation or decoding fails, before the caller
    /// performs timeline validation, disk writes or channel sends.
    unsafe fn decode_and_release_application_packet(
        pointer: *const u8,
        frames: u32,
        flags: u32,
        release: impl FnOnce() -> Result<(), String>,
    ) -> Result<Vec<i16>, String> {
        let decoded = (|| {
            if frames > APP_CAPTURE_FORMAT_RATE * 2 {
                return Err("audio packet จากแอปเกินขนาด buffer ที่อนุญาต".to_string());
            }
            if frames == 0 {
                return Ok(Vec::new());
            }
            if flags & AUDCLNT_BUFFERFLAGS_SILENT.0 as u32 != 0 {
                return Ok(vec![0; frames as usize]);
            }
            if pointer.is_null() {
                return Err("audio packet จากแอปเป็น null โดยไม่มีธง SILENT".to_string());
            }
            let bytes = unsafe {
                std::slice::from_raw_parts(pointer, frames as usize * APP_CAPTURE_CHANNELS * 2)
            };
            Ok(bytes
                .as_chunks::<{ APP_CAPTURE_CHANNELS * 2 }>()
                .0
                .iter()
                .map(|frame| {
                    let left = i16::from_le_bytes([frame[0], frame[1]]) as i32;
                    let right = i16::from_le_bytes([frame[2], frame[3]]) as i32;
                    ((left + right) / 2) as i16
                })
                .collect())
        })();
        let released = release();
        let samples = decoded?;
        released?;
        Ok(samples)
    }

    fn read_application_packet(
        capture: &IAudioCaptureClient,
    ) -> Result<(Vec<i16>, u64, u32, u64), String> {
        let mut pointer = std::ptr::null_mut();
        let (mut frames, mut flags, mut index, mut timestamp) = (0, 0, 0, 0);
        unsafe {
            capture.GetBuffer(
                &mut pointer,
                &mut frames,
                &mut flags,
                Some(&mut index),
                Some(&mut timestamp),
            )
        }
        .map_err(|error| format!("อ่าน audio packet จากแอปไม่สำเร็จ: {error}"))?;
        // A successful empty read has no packet to release.
        if frames == 0 {
            return Ok((Vec::new(), index, flags, timestamp));
        }
        let samples = unsafe {
            decode_and_release_application_packet(pointer, frames, flags, || {
                capture
                    .ReleaseBuffer(frames)
                    .map_err(|error| format!("คืน audio packet จากแอปไม่สำเร็จ: {error}"))
            })
        }?;
        Ok((samples, index, flags, timestamp))
    }

    struct InventoryApartment;
    impl Drop for InventoryApartment {
        fn drop(&mut self) {
            wasapi::deinitialize();
        }
    }

    fn on_inventory_thread<T: Send + 'static>(
        read: impl FnOnce() -> Result<T, String> + Send + 'static,
    ) -> Result<T, String> {
        thread::Builder::new()
            .name("fung-audio-inventory".into())
            .spawn(move || {
                initialize_mta()
                    .ok()
                    .map_err(|error| format!("เริ่ม Windows audio service ไม่สำเร็จ ({error})"))?;
                let _apartment = InventoryApartment;
                read()
            })
            .map_err(|error| format!("เปิด audio inventory thread ไม่สำเร็จ: {error}"))?
            .join()
            .map_err(|_| "audio inventory thread หยุดทำงาน".to_string())?
    }

    fn packet_first_sample(
        origin: &mut Option<u64>,
        index: u64,
        expected: u64,
        discontinuity: bool,
        timestamp_error: bool,
    ) -> Result<u64, String> {
        if timestamp_error {
            return Err("ตำแหน่งเวลาเสียงจากแอปไม่น่าเชื่อถือ".into());
        }
        let first_packet = origin.is_none();
        let base = *origin.get_or_insert(index);
        let first = index.checked_sub(base).ok_or("ตำแหน่งเสียงจากแอปย้อนกลับ")?;
        if first < expected || (!first_packet && discontinuity && first == expected) {
            return Err("เสียงจากแอปขาดช่วงโดยไม่ทราบขนาด หรืออุปกรณ์รีเซ็ตตำแหน่ง".into());
        }
        Ok(first)
    }

    #[derive(Debug, Default)]
    enum ApplicationPositionMode {
        #[default]
        Undetermined,
        DeviceIndex,
        Qpc,
    }

    #[derive(Debug)]
    struct ApplicationPacketStamp {
        index: u64,
        qpc: u64,
        frames: u64,
    }

    #[derive(Debug, Default)]
    struct ApplicationPacketClock {
        mode: ApplicationPositionMode,
        native_origin: Option<u64>,
        qpc_origin: Option<(u64, Duration)>,
        previous: Option<ApplicationPacketStamp>,
    }

    impl ApplicationPacketClock {
        #[allow(clippy::too_many_arguments)]
        fn position(
            &mut self,
            index: u64,
            qpc: u64,
            frames: u64,
            flags: u32,
            expected: u64,
            observed_elapsed: Duration,
        ) -> Result<u64, String> {
            let discontinuity = flags & AUDCLNT_BUFFERFLAGS_DATA_DISCONTINUITY.0 as u32 != 0;
            let timestamp_error = flags & AUDCLNT_BUFFERFLAGS_TIMESTAMP_ERROR.0 as u32 != 0;
            if timestamp_error {
                return Err("ตำแหน่งเวลาเสียงจากแอปไม่น่าเชื่อถือ".into());
            }
            let Some(previous) = self.previous.as_ref() else {
                let first = packet_first_sample(
                    &mut self.native_origin,
                    index,
                    expected,
                    discontinuity,
                    false,
                )?;
                self.qpc_origin = Some((qpc, observed_elapsed));
                self.previous = Some(ApplicationPacketStamp { index, qpc, frames });
                return Ok(first);
            };

            if matches!(self.mode, ApplicationPositionMode::Undetermined) {
                self.mode = if previous.index == 0 && index == 0 && !discontinuity {
                    // Process-loopback virtual clients can leave device position
                    // at zero. Establish this only on clean packets with valid
                    // QPC below, never after an established counter resets.
                    ApplicationPositionMode::Qpc
                } else {
                    ApplicationPositionMode::DeviceIndex
                };
            }
            let first = match self.mode {
                ApplicationPositionMode::DeviceIndex => packet_first_sample(
                    &mut self.native_origin,
                    index,
                    expected,
                    discontinuity,
                    false,
                )?,
                ApplicationPositionMode::Qpc => {
                    let (origin, observed_origin) = self.qpc_origin.unwrap();
                    if origin == 0 || qpc <= previous.qpc {
                        return Err("QPC ของ process loopback ไม่เดินหน้าหรือไม่พร้อมใช้งาน".into());
                    }
                    let elapsed_hns =
                        observed_elapsed.saturating_sub(observed_origin).as_nanos() / 100;
                    // Bound virtual-client timestamps independently of PCM
                    // duration. A malformed initial QPC cannot create years of
                    // timeline gaps. Two seconds is the packet safety ceiling.
                    if u128::from(qpc.saturating_sub(origin)) > elapsed_hns + 20_000_000 {
                        return Err("QPC ของ process loopback เกินช่วงเวลาที่บันทึกจริง".into());
                    }
                    let delta_frames = u64::try_from(
                        (u128::from(qpc - previous.qpc) * u128::from(APP_CAPTURE_FORMAT_RATE)
                            + 5_000_000)
                            / 10_000_000,
                    )
                    .map_err(|_| "QPC ของ process loopback เกินขอบเขตจำนวนเฟรม")?;
                    // Count contiguous PCM exactly rather than accumulating
                    // nominal-rate/QPC drift. Packet-time jitter below 1 ms is
                    // not evidence of a missing packet; native flags still apply.
                    let tolerance = u64::from(APP_CAPTURE_FORMAT_RATE / 1000);
                    #[cfg(test)]
                    if delta_frames.abs_diff(previous.frames) > tolerance {
                        eprintln!(
                            "APPLICATION_PACKET_TIME_ANOMALY index={index} previous_index={} qpc_hns={qpc} previous_qpc_hns={} frames={frames} previous_frames={} delta_frames={delta_frames} flags=0x{flags:x} expected={expected} observed_us={}",
                            previous.index, previous.qpc, previous.frames,
                            observed_elapsed.as_micros()
                        );
                    }
                    if delta_frames.abs_diff(previous.frames) <= tolerance {
                        if discontinuity {
                            return Err("เสียงจากแอปขาดช่วงโดยไม่ทราบขนาด".into());
                        }
                        expected
                    } else if delta_frames < previous.frames {
                        return Err("เวลา packet เสียงจากแอปซ้อนทับเสียงก่อนหน้า".into());
                    } else {
                        expected
                            .checked_add(delta_frames - previous.frames)
                            .ok_or("ตำแหน่ง packet เสียงจากแอปเกินขอบเขต")?
                    }
                }
                ApplicationPositionMode::Undetermined => unreachable!(),
            };
            self.previous = Some(ApplicationPacketStamp { index, qpc, frames });
            Ok(first)
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn finish_application_capture(
        result: Result<(), String>,
        sample_rate: u32,
        chunks_dir: &Path,
        chunk_tx: &mpsc::SyncSender<CaptureEvent>,
        accumulator: &mut Vec<i16>,
        timeline_samples: &mut u64,
        local_seq: &mut u32,
    ) -> Result<(), String> {
        // Captured PCM is durable regardless of fragment length or stop outcome.
        // ASR minimum input duration must not discard valid source audio.
        if !accumulator.is_empty() {
            let take = accumulator.len();
            cut_capture_chunk(
                CHANNEL_SYSTEM,
                sample_rate,
                chunks_dir,
                chunk_tx,
                accumulator,
                timeline_samples,
                local_seq,
                take,
            );
        }
        result
    }

    #[derive(Debug)]
    struct ApplicationQueueFailure {
        start: u64,
        end: u64,
        reason: &'static str,
    }

    fn queue_application_packet(
        sender: &mpsc::SyncSender<CaptureSampleBatch>,
        captured_end: &mut u64,
        batch: CaptureSampleBatch,
    ) -> Result<(), ApplicationQueueFailure> {
        let start = batch.first_sample;
        let end = start
            .checked_add(batch.samples.len() as u64)
            .ok_or(ApplicationQueueFailure {
                start,
                end: start,
                reason: "application_writer_position_overflow",
            })?;
        // Independent of writer progress, including the final rejected packet.
        *captured_end = end;
        sender
            .try_send(batch)
            .map_err(|error| ApplicationQueueFailure {
                start,
                end,
                reason: match error {
                    mpsc::TrySendError::Full(_) => "application_writer_queue_full",
                    mpsc::TrySendError::Disconnected(_) => "application_writer_disconnected",
                },
            })
    }

    fn write_application_packets(
        packets: mpsc::Receiver<CaptureSampleBatch>,
        sample_rate: u32,
        samples_per_chunk: usize,
        chunks_dir: &Path,
        chunk_tx: &mpsc::SyncSender<CaptureEvent>,
    ) -> Result<(), String> {
        let mut accumulator = Vec::with_capacity(samples_per_chunk + 4096);
        let mut timeline_samples = 0;
        let mut writer_expected = 0;
        let mut local_seq = 0;
        let mut last_level_event = Instant::now();
        #[cfg(test)]
        let slow_write_ms = std::env::var("FUNG_APPLICATION_PROBE_SLOW_WRITE_MS")
            .ok()
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(0)
            .min(1000);
        for batch in packets {
            #[cfg(test)]
            if slow_write_ms > 0 && accumulator.len() + batch.samples.len() >= samples_per_chunk {
                thread::sleep(Duration::from_millis(slow_write_ms));
            }
            process_capture_sample_batch_with_gap_reason(
                batch,
                CHANNEL_SYSTEM,
                sample_rate,
                samples_per_chunk,
                chunks_dir,
                chunk_tx,
                &mut accumulator,
                &mut timeline_samples,
                &mut writer_expected,
                &mut local_seq,
                &mut last_level_event,
                "application_loopback_discontinuity",
            );
        }
        finish_application_capture(
            Ok(()),
            sample_rate,
            chunks_dir,
            chunk_tx,
            &mut accumulator,
            &mut timeline_samples,
            &mut local_seq,
        )
    }

    #[cfg(test)]
    mod capture_regression_tests {
        use super::*;

        #[test]
        fn application_event_reader_uses_bounded_writer_queue() {
            assert!(
                matches!(
                    application_capture_mode(),
                    StreamMode::EventsShared {
                        autoconvert: true,
                        buffer_duration_hns: 0,
                    }
                ),
                "the virtual client only supports its minimum native buffer"
            );
            assert_eq!(APP_WRITER_QUEUE_PACKETS, 256);
        }

        #[test]
        fn bounded_application_queue_reports_final_dropped_packet_without_blocking() {
            let (tx, rx) = mpsc::sync_channel(1);
            let mut captured_end = 0;
            let packet = |first_sample| CaptureSampleBatch {
                first_sample,
                silence_before: None,
                samples: vec![1234; 480],
            };
            queue_application_packet(&tx, &mut captured_end, packet(0)).unwrap();
            let failure =
                queue_application_packet(&tx, &mut captured_end, packet(480)).unwrap_err();
            assert_eq!((failure.start, failure.end, captured_end), (480, 960, 960));
            assert_eq!(failure.reason, "application_writer_queue_full");
            assert_eq!(rx.try_recv().unwrap().first_sample, 0);
            assert!(rx.try_recv().is_err());
            drop(rx);
            let failure =
                queue_application_packet(&tx, &mut captured_end, packet(960)).unwrap_err();
            assert_eq!(
                (failure.start, failure.end, captured_end),
                (960, 1440, 1440)
            );
            assert_eq!(failure.reason, "application_writer_disconnected");
        }

        #[test]
        fn application_packet_reader_does_not_use_null_unsafe_wasapi_copy() {
            let source = include_str!("live_application_audio.rs");
            let unsafe_copy_call = [".read_from_device", "_to_deque("].concat();
            assert!(
                !source.contains(&unsafe_copy_call),
                "wasapi 0.26 copies a possibly-null SILENT packet before checking its flags"
            );
        }

        #[test]
        fn virtual_zero_device_positions_use_advancing_native_qpc() {
            let mut clock = ApplicationPacketClock::default();
            for packet in 0..5 {
                assert_eq!(
                    clock
                        .position(
                            0,
                            1_000_000_000 + packet * 100_000,
                            480,
                            0,
                            packet * 480,
                            Duration::from_millis(packet * 10),
                        )
                        .unwrap(),
                    packet * 480,
                );
            }
            assert!(matches!(clock.mode, ApplicationPositionMode::Qpc));
        }

        #[test]
        fn virtual_qpc_clock_counts_contiguous_pcm_without_accumulated_rate_drift() {
            let mut clock = ApplicationPacketClock::default();
            for packet in 0..1000 {
                // Native clock is 100 ppm from the requested sample rate.
                let qpc_delta = packet * 100_010;
                assert_eq!(
                    clock
                        .position(
                            0,
                            1_000_000_000 + qpc_delta,
                            480,
                            0,
                            packet * 480,
                            Duration::from_nanos(qpc_delta * 100),
                        )
                        .unwrap(),
                    packet * 480,
                );
            }
        }

        #[test]
        fn virtual_qpc_clock_preserves_measured_loss_and_rejects_unmeasurable_loss() {
            let make_clock = || {
                let mut clock = ApplicationPacketClock::default();
                clock
                    .position(0, 1_000_000_000, 480, 0, 0, Duration::ZERO)
                    .unwrap();
                clock
                    .position(0, 1_000_100_000, 480, 0, 480, Duration::from_millis(10))
                    .unwrap();
                clock
            };
            assert_eq!(
                make_clock()
                    .position(
                        0,
                        1_000_300_000,
                        480,
                        AUDCLNT_BUFFERFLAGS_DATA_DISCONTINUITY.0 as u32,
                        960,
                        Duration::from_millis(30),
                    )
                    .unwrap(),
                1440,
            );
            assert!(make_clock()
                .position(
                    0,
                    1_000_200_000,
                    480,
                    AUDCLNT_BUFFERFLAGS_DATA_DISCONTINUITY.0 as u32,
                    960,
                    Duration::from_millis(20),
                )
                .is_err());
            assert!(make_clock()
                .position(
                    0,
                    1_000_200_000,
                    480,
                    AUDCLNT_BUFFERFLAGS_TIMESTAMP_ERROR.0 as u32,
                    960,
                    Duration::from_millis(20),
                )
                .is_err());
            assert!(make_clock()
                .position(0, 1_000_090_000, 480, 0, 960, Duration::from_millis(20),)
                .is_err());
            assert!(make_clock()
                .position(0, 9_000_000_000, 480, 0, 960, Duration::from_millis(20),)
                .is_err());
        }

        #[test]
        fn established_device_counter_reset_does_not_switch_to_virtual_qpc_mode() {
            let mut clock = ApplicationPacketClock::default();
            assert_eq!(
                clock
                    .position(100_000, 1_000_000_000, 480, 0, 0, Duration::ZERO)
                    .unwrap(),
                0
            );
            assert_eq!(
                clock
                    .position(
                        100_480,
                        1_000_100_000,
                        480,
                        0,
                        480,
                        Duration::from_millis(10)
                    )
                    .unwrap(),
                480
            );
            assert!(matches!(clock.mode, ApplicationPositionMode::DeviceIndex));
            assert!(clock
                .position(0, 1_000_200_000, 480, 0, 960, Duration::from_millis(20))
                .is_err());
            let mut unavailable_qpc = ApplicationPacketClock::default();
            unavailable_qpc
                .position(0, 0, 480, 0, 0, Duration::ZERO)
                .unwrap();
            assert!(unavailable_qpc
                .position(0, 100_000, 480, 0, 480, Duration::from_millis(10))
                .is_err());
        }

        #[test]
        #[ignore = "records the explicitly selected process into a fresh isolated directory"]
        fn native_application_loopback_probe() {
            let process_id: u32 = std::env::var("FUNG_APPLICATION_PROBE_PID")
                .expect("set the controlled audio fixture PID")
                .parse()
                .unwrap();
            let root = std::path::PathBuf::from(
                std::env::var("FUNG_APPLICATION_PROBE_DIR").expect("set fresh evidence directory"),
            );
            assert!(root.is_absolute() && !root.exists());
            let application = validate_active_audio_application(process_id).unwrap();
            std::fs::create_dir_all(&root).unwrap();
            let stop = Arc::new(AtomicBool::new(false));
            let (tx, rx) = mpsc::sync_channel(256);
            let ready = spawn_application_capture_thread(
                process_id,
                application.name,
                stop.clone(),
                tx,
                root.clone(),
                2_000,
            )
            .unwrap();
            std::fs::write(root.join("ready.txt"), &ready.device_name).unwrap();
            eprintln!("APPLICATION_CAPTURE_READY: PID {process_id}, 12-second isolated probe");
            let started = Instant::now();
            let mut chunks = Vec::new();
            let mut faults = Vec::new();
            loop {
                if started.elapsed() >= Duration::from_secs(12) {
                    stop.store(true, Ordering::SeqCst);
                }
                match rx.recv_timeout(Duration::from_millis(100)) {
                    Ok(CaptureEvent::Chunk(chunk)) => chunks.push(chunk),
                    Ok(CaptureEvent::Level { .. }) => {}
                    Ok(CaptureEvent::SourceGap {
                        start_ms,
                        end_ms,
                        reason,
                        ..
                    }) => faults.push(format!("gap {start_ms}..{end_ms} {reason}")),
                    Ok(CaptureEvent::StreamFailed { error, .. }) => faults.push(error),
                    Ok(CaptureEvent::ChunkWriteFailed { error, .. }) => faults.push(error),
                    Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    Err(mpsc::RecvTimeoutError::Timeout) => {
                        assert!(started.elapsed() < Duration::from_secs(22));
                    }
                }
            }
            let mut nonzero = 0u64;
            let mut peak = 0i32;
            let mut metadata = Vec::new();
            for chunk in chunks {
                let mut wav = hound::WavReader::open(&chunk.file_path).unwrap();
                for sample in wav.samples::<i16>() {
                    let sample = i32::from(sample.unwrap());
                    if sample != 0 {
                        nonzero += 1;
                    }
                    peak = peak.max(sample.abs());
                }
                metadata.push(serde_json::json!({"startMs":chunk.start_ms,"endMs":chunk.end_ms,"file":chunk.file_path}));
            }
            let report = serde_json::json!({"processId":process_id,"device":ready.device_name,"chunks":metadata,"faults":faults,"nonzeroSamples":nonzero,"peak":peak,"scope":"native selected-process capture only; no installed UI/transcription claim"});
            std::fs::write(
                root.join("probe-report.json"),
                serde_json::to_vec_pretty(&report).unwrap(),
            )
            .unwrap();
            eprintln!("{report}");
            assert!(faults.is_empty());
            assert!(nonzero > 0, "controlled fixture must produce actual audio");
        }

        #[test]
        fn application_packet_silent_null_decodes_zero_pcm_and_releases_once() {
            let mut releases = 0;
            let samples = unsafe {
                decode_and_release_application_packet(
                    std::ptr::null(),
                    480,
                    AUDCLNT_BUFFERFLAGS_SILENT.0 as u32,
                    || {
                        releases += 1;
                        Ok(())
                    },
                )
            }
            .unwrap();
            assert_eq!(samples, vec![0; 480]);
            assert_eq!(releases, 1);
        }

        #[test]
        fn application_packet_invalid_pointer_and_size_still_release() {
            for (frames, flags) in [
                (1, 0),
                (
                    APP_CAPTURE_FORMAT_RATE * 2 + 1,
                    AUDCLNT_BUFFERFLAGS_SILENT.0 as u32,
                ),
            ] {
                let mut releases = 0;
                let result = unsafe {
                    decode_and_release_application_packet(std::ptr::null(), frames, flags, || {
                        releases += 1;
                        Ok(())
                    })
                };
                assert!(result.is_err());
                assert_eq!(releases, 1);
            }
        }

        #[test]
        fn application_packet_decodes_stereo_without_losing_release_failures() {
            let bytes = [1000i16.to_le_bytes(), (-500i16).to_le_bytes()].concat();
            let mut releases = 0;
            let samples = unsafe {
                decode_and_release_application_packet(bytes.as_ptr(), 1, 0, || {
                    releases += 1;
                    Ok(())
                })
            }
            .unwrap();
            assert_eq!(samples, vec![250]);
            assert_eq!(releases, 1);
            let result = unsafe {
                decode_and_release_application_packet(bytes.as_ptr(), 1, 0, || {
                    Err("native release failed".into())
                })
            };
            assert_eq!(result.unwrap_err(), "native release failed");
        }

        #[test]
        fn inventory_worker_owns_mta_even_for_sta_caller() {
            thread::spawn(|| {
                wasapi::initialize_sta().ok().unwrap();
                let result = on_inventory_thread(|| {
                    assert!(
                        wasapi::initialize_sta().is_err(),
                        "inventory worker must already own MTA"
                    );
                    Ok(())
                });
                wasapi::deinitialize();
                result.unwrap();
            })
            .join()
            .unwrap();
        }

        #[test]
        #[ignore = "reads native active audio-session inventory for explicit UAT diagnosis"]
        fn active_inventory_apartment_probe() {
            let sta = thread::spawn(|| {
                wasapi::initialize_sta().ok().unwrap();
                let result = active_audio_applications();
                wasapi::deinitialize();
                result
            })
            .join()
            .unwrap();
            let mta = thread::spawn(active_audio_applications).join().unwrap();
            eprintln!("INVENTORY_STA={:?}", sta);
            eprintln!("INVENTORY_MTA={:?}", mta);
            assert!(
                sta.is_ok(),
                "production inventory must support a Tauri STA caller"
            );
            assert!(mta.is_ok(), "MTA inventory must be readable");
        }

        #[test]
        fn packet_positions_preserve_native_gaps_and_reject_unmeasured_loss() {
            let mut origin = None;
            assert_eq!(
                packet_first_sample(&mut origin, 100_000, 0, true, false).unwrap(),
                0
            );
            assert_eq!(
                packet_first_sample(&mut origin, 100_480, 480, false, false).unwrap(),
                480
            );
            assert_eq!(
                packet_first_sample(&mut origin, 101_440, 960, true, false).unwrap(),
                1440
            );
            assert!(packet_first_sample(&mut origin, 101_920, 1920, true, false).is_err());
            assert!(packet_first_sample(&mut origin, 100_000, 1920, false, false).is_err());
            assert!(packet_first_sample(&mut origin, 102_400, 1920, false, true).is_err());
        }

        #[test]
        fn normal_stop_preserves_thirty_millisecond_captured_tail() {
            let root = tempfile::tempdir().unwrap();
            let (tx, rx) = mpsc::sync_channel(8);
            let mut samples = vec![1234; 1440];
            let mut timeline = 96000;
            let mut sequence = 1;
            finish_application_capture(
                Ok(()),
                48000,
                root.path(),
                &tx,
                &mut samples,
                &mut timeline,
                &mut sequence,
            )
            .unwrap();
            let CaptureEvent::Chunk(chunk) = rx.recv().unwrap() else {
                panic!("already-captured 30 ms tail must be durable, not a source gap")
            };
            assert_eq!((chunk.start_ms, chunk.end_ms), (2000, 2030));
            let values: Vec<_> = hound::WavReader::open(chunk.file_path)
                .unwrap()
                .into_samples::<i16>()
                .map(Result::unwrap)
                .collect();
            assert_eq!(values, vec![1234; 1440]);
            assert!(samples.is_empty());
            assert_eq!(timeline, 97440);
            assert_eq!(sequence, 2);
            assert!(rx.try_recv().is_err());
        }

        #[test]
        fn packet_failure_keeps_captured_tail_before_reporting_error() {
            let root = tempfile::tempdir().unwrap();
            let (tx, rx) = mpsc::sync_channel(8);
            let mut samples = vec![1234; 500];
            let mut timeline = 0;
            let mut sequence = 0;
            let result = finish_application_capture(
                Err("packet read failed".into()),
                1000,
                root.path(),
                &tx,
                &mut samples,
                &mut timeline,
                &mut sequence,
            );
            assert_eq!(result.unwrap_err(), "packet read failed");
            assert!(samples.is_empty());
            let CaptureEvent::Chunk(chunk) = rx.recv().unwrap() else {
                panic!("tail was lost")
            };
            assert_eq!((chunk.start_ms, chunk.end_ms), (0, 500));
            let values: Vec<_> = hound::WavReader::open(chunk.file_path)
                .unwrap()
                .into_samples::<i16>()
                .map(Result::unwrap)
                .collect();
            assert_eq!(values, vec![1234; 500]);
        }

        #[test]
        fn native_packet_gap_reaches_durable_chunk_timeline_and_gap_evidence() {
            let root = tempfile::tempdir().unwrap();
            let (tx, rx) = mpsc::sync_channel(8);
            let mut origin = None;
            let mut samples = Vec::new();
            let mut timeline = 0;
            let mut expected = 0;
            let mut sequence = 0;
            let mut last_level = Instant::now();
            for (index, discontinuity) in [(100_000, true), (101_440, true)] {
                let first = packet_first_sample(&mut origin, index, expected, discontinuity, false)
                    .unwrap();
                process_capture_sample_batch_with_gap_reason(
                    CaptureSampleBatch {
                        first_sample: first,
                        samples: vec![1234; 480],
                        silence_before: None,
                    },
                    CHANNEL_SYSTEM,
                    48000,
                    96000,
                    root.path(),
                    &tx,
                    &mut samples,
                    &mut timeline,
                    &mut expected,
                    &mut sequence,
                    &mut last_level,
                    "application_loopback_discontinuity",
                );
            }
            let CaptureEvent::Chunk(chunk) = rx.recv().unwrap() else {
                panic!("pre-gap tail lost")
            };
            assert_eq!((chunk.start_ms, chunk.end_ms), (0, 10));
            let CaptureEvent::SourceGap {
                start_ms,
                end_ms,
                reason,
                ..
            } = rx.recv().unwrap()
            else {
                panic!("gap not reported")
            };
            assert_eq!(
                (start_ms, end_ms, reason),
                (10, 30, "application_loopback_discontinuity")
            );
            finish_application_capture(
                Err("device stopped".into()),
                48000,
                root.path(),
                &tx,
                &mut samples,
                &mut timeline,
                &mut sequence,
            )
            .unwrap_err();
            let CaptureEvent::Chunk(chunk) = rx.recv().unwrap() else {
                panic!("post-gap tail lost")
            };
            assert_eq!((chunk.start_ms, chunk.end_ms), (30, 40));
        }
    }

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
        on_inventory_thread(read_active_audio_applications)
    }

    fn read_active_audio_applications() -> Result<Vec<LiveCaptureApplication>, String> {
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
                        terminal: true,
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
        let _apartment = ApplicationCaptureApartment;
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
        let mode = application_capture_mode();
        let capture_started = Instant::now();
        client
            .initialize_client(&desired_format, &Direction::Capture, &mode)
            .map_err(|error| format!("ตั้งค่า stream ของแอปไม่สำเร็จ: {error}"))?;
        let buffer_frames = client
            .get_buffer_size()
            .map_err(|error| format!("อ่านความจุ buffer ของแอปไม่สำเร็จ: {error}"))?;
        #[cfg(test)]
        eprintln!("APPLICATION_CAPTURE_BUFFER requested_hns=0 actual_frames={buffer_frames} rate={APP_CAPTURE_FORMAT_RATE} channels={APP_CAPTURE_CHANNELS} bits=16 writer_queue_packets={APP_WRITER_QUEUE_PACKETS}");
        if buffer_frames == 0 {
            return Err("buffer เสียงแอปไม่มีความจุ".into());
        }
        let event = client
            .set_get_eventhandle()
            .map_err(|error| format!("สร้าง event ของ stream แอปไม่สำเร็จ: {error}"))?;
        // wasapi 0.26's copy helpers dereference SILENT packets before reading
        // their flags. Use its public native interface to read/release safely.
        let capture: IAudioCaptureClient = unsafe { client.as_iaudioclient().GetService() }
            .map_err(|error| format!("เปิด audio capture client ของแอปไม่สำเร็จ: {error}"))?;
        client
            .start_stream()
            .map_err(|error| format!("เริ่ม stream ของแอปไม่สำเร็จ: {error}"))?;
        if let Some(ready) = readiness.take() {
            let _ = ready.send(Ok(application_name.to_string()));
        }

        let sample_rate = APP_CAPTURE_FORMAT_RATE;
        let samples_per_chunk = (sample_rate as u64 * fragment_ms / 1000) as usize;
        let mut expected_sample = 0u64;
        let mut packet_clock = ApplicationPacketClock::default();
        let mut packet_number = 0u64;
        let mut stop_attempted = false;
        let mut queue_failure = None;

        thread::scope(|scope| {
            let (packet_tx, packet_rx) = mpsc::sync_channel(APP_WRITER_QUEUE_PACKETS);
            let writer = scope.spawn(move || {
                write_application_packets(
                    packet_rx,
                    sample_rate,
                    samples_per_chunk,
                    chunks_dir,
                    chunk_tx,
                )
            });
            let capture_result = (|| -> Result<(), String> {
                loop {
                    if stop.load(Ordering::SeqCst) {
                        // Stop production before draining the last native buffer;
                        // otherwise a normal stop can discard accepted final PCM.
                        stop_attempted = true;
                        client
                            .stop_stream()
                            .map_err(|error| format!("หยุด stream แอปไม่สำเร็จ: {error}"))?;
                    } else {
                        if let Err(error) = event.wait_for_event(APP_CAPTURE_WAIT_MS) {
                            if matches!(error, wasapi::WasapiError::EventTimeout) {
                                continue;
                            }
                            return Err(format!("รอสัญญาณเสียงจากแอปไม่สำเร็จ: {error}"));
                        }
                    }

                    loop {
                        let packet_frames = unsafe { capture.GetNextPacketSize() }
                            .map_err(|error| format!("อ่านขนาด packet จากแอปไม่สำเร็จ: {error}"))?;
                        if packet_frames == 0 {
                            break;
                        }
                        let (mono, packet_index, packet_flags, packet_qpc) =
                            read_application_packet(&capture)?;
                        if mono.is_empty() {
                            continue;
                        }
                        packet_number += 1;
                        let first_sample = packet_clock.position(
                        packet_index,
                        packet_qpc,
                        mono.len() as u64,
                        packet_flags,
                        expected_sample,
                        capture_started.elapsed(),
                    )
                    .map_err(|error| {
                        format!(
                            "{error}; packet={packet_number}, frames={}, flags=0x{packet_flags:x}, index={packet_index}, clock={packet_clock:?}, expected={expected_sample}, qpc_hns={packet_qpc}",
                            mono.len()
                        )
                    })?;
                        let batch = CaptureSampleBatch {
                            silence_before: None,
                            first_sample,
                            samples: mono,
                        };
                        if let Err(failure) =
                            queue_application_packet(&packet_tx, &mut expected_sample, batch)
                        {
                            let error = format!(
                                "{}; captured_samples={}..{}",
                                failure.reason, failure.start, failure.end
                            );
                            queue_failure = Some(failure);
                            return Err(error);
                        }
                    }
                    if stop_attempted {
                        break;
                    }
                }

                Ok(())
            })();
            let stop_result = if stop_attempted {
                Ok(())
            } else {
                client
                    .stop_stream()
                    .map_err(|error| format!("หยุด stream แอปไม่สำเร็จ: {error}"))
            };
            // Stop native production before potentially waiting on disk/event work.
            // Closing the queue drains every accepted packet and flushes its tail.
            drop(packet_tx);
            let writer_result = writer
                .join()
                .map_err(|_| "thread บันทึกเสียงแอปหยุดทำงาน".to_string())
                .and_then(|result| result);
            if let Some(failure) = queue_failure {
                if failure.end > failure.start {
                    let _ = chunk_tx.send(CaptureEvent::SourceGap {
                        channel: CHANNEL_SYSTEM,
                        start_ms: (failure.start * 1000 / u64::from(sample_rate)) as i64,
                        end_ms: (failure.end * 1000 / u64::from(sample_rate)) as i64,
                        reason: failure.reason,
                    });
                }
            }
            capture_result.and(stop_result).and(writer_result)
        })
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
