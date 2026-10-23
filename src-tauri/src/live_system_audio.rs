//! Native endpoint loopback retains packet flags that CPAL does not expose.
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};

use crate::live_meeting::{
    cut_capture_chunk, process_capture_sample_batch_with_gap_reason, CaptureEvent, CaptureReady,
    CaptureSampleBatch, CHANNEL_SYSTEM, MIN_FINAL_CHUNK_MS,
};
use windows::core::PCWSTR;
use windows::Win32::Media::Audio::*;
use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_ALL};
use windows::Win32::System::Performance::{QueryPerformanceCounter, QueryPerformanceFrequency};

const RATE: u64 = 48_000;

struct PacketClock {
    origin_hns: u64,
    end: u64,
    last_index: Option<u64>,
    last_timestamp: Option<u64>,
    previous_frames: u64,
    max_future_hns: u64,
}

impl PacketClock {
    fn new(origin_hns: u64, max_future_hns: u64) -> Self {
        Self {
            origin_hns,
            end: 0,
            last_index: None,
            last_timestamp: None,
            previous_frames: 0,
            max_future_hns,
        }
    }
    #[allow(clippy::too_many_arguments)]
    fn position(
        &mut self,
        timestamp: u64,
        now: u64,
        index: u64,
        frames: u64,
        discontinuity: bool,
        timestamp_error: bool,
    ) -> Result<(u64, Option<u64>), String> {
        if timestamp_error {
            return Err("system loopback packet has TIMESTAMP_ERROR".into());
        }
        if frames > RATE * 2 {
            return Err("system loopback packet exceeds bounded size".into());
        }
        if timestamp > now.saturating_add(self.max_future_hns) {
            return Err("system loopback packet exceeds endpoint clock latency bound".into());
        }
        if timestamp < self.origin_hns && self.origin_hns - timestamp > self.max_future_hns {
            return Err("system loopback packet predates stream start".into());
        }
        if self.last_index.is_some_and(|last| index < last) {
            return Err("system loopback device position went backwards".into());
        }
        let to_frames = |hns: u64| -> Result<u64, String> {
            u64::try_from((u128::from(hns) * u128::from(RATE) + 5_000_000) / 10_000_000)
                .map_err(|_| "system loopback timestamp exceeded frame range".into())
        };
        let first_packet = self.last_timestamp.is_none();
        let startup_packet = first_packet && index == 0;
        let first = if let Some(previous) = self.last_timestamp {
            let delta = timestamp
                .checked_sub(previous)
                .filter(|delta| *delta > 0)
                .ok_or("system loopback timestamp went backwards")?;
            let packet_duration = self.previous_frames * 10_000_000 / RATE;
            let idle = delta.saturating_sub(packet_duration);
            // Keep continuous audio on its sample clock. Per-packet clock
            // rounding/jitter must not accumulate into fabricated gaps or loss.
            let idle_frames = if idle > 20_000 { to_frames(idle)? } else { 0 };
            self.end
                .checked_add(idle_frames)
                .ok_or("system loopback frame range overflow")?
        } else {
            to_frames(timestamp.saturating_sub(self.origin_hns))?
        };
        if discontinuity && !startup_packet && first == self.end {
            return Err("system loopback lost audio with unknown duration".into());
        }
        // A flagged first packet at native position zero is a startup marker.
        // A nonzero first position can instead mean the initial buffer overran.
        let silence = (first > self.end && (!discontinuity || startup_packet)).then_some(self.end);
        self.last_timestamp = Some(timestamp);
        self.previous_frames = frames;
        self.end = first
            .checked_add(frames)
            .ok_or("system loopback frame range overflow")?;
        // Native packet positions must remain monotonic.
        self.last_index = Some(index);
        Ok((first, silence))
    }
}

fn win<T>(result: windows::core::Result<T>) -> Result<T, String> {
    result.map_err(|error| format!("Windows system audio: {error}"))
}

fn qpc_hns(frequency: i64) -> Result<u64, String> {
    let mut counter = 0;
    win(unsafe { QueryPerformanceCounter(&mut counter) })?;
    if counter < 0 || frequency <= 0 {
        return Err("invalid Windows capture clock".into());
    }
    u64::try_from(counter as u128 * 10_000_000 / frequency as u128)
        .map_err(|_| "Windows capture clock overflow".into())
}

struct RunningClient(IAudioClient);
impl Drop for RunningClient {
    fn drop(&mut self) {
        let _ = unsafe { self.0.Stop() };
    }
}

struct ComApartment;
impl Drop for ComApartment {
    fn drop(&mut self) {
        wasapi::deinitialize();
    }
}

pub(crate) fn spawn_system_capture_thread(
    name: String,
    stop: Arc<AtomicBool>,
    tx: mpsc::SyncSender<CaptureEvent>,
    root: PathBuf,
    fragment_ms: u64,
) -> Result<CaptureReady, String> {
    let (ready_tx, ready_rx) = mpsc::channel();
    std::thread::Builder::new()
        .name("fung-system-loopback".into())
        .spawn(move || {
            let mut readiness = Some(ready_tx);
            let result = capture(&name, &stop, &tx, &root, fragment_ms, &mut readiness);
            if let Some(ready) = readiness {
                let _ = ready.send(Err(result
                    .err()
                    .unwrap_or_else(|| "system loopback stopped before start".into())));
            } else if let Err(error) = result {
                let _ = tx.send(CaptureEvent::StreamFailed {
                    channel: CHANNEL_SYSTEM,
                    error,
                });
            }
        })
        .map_err(|error| format!("start system capture thread: {error}"))?;
    ready_rx
        .recv_timeout(Duration::from_secs(10))
        .map_err(|error| format!("system capture readiness: {error}"))?
        .map(|device_name| CaptureReady { device_name })
}

fn capture(
    name: &str,
    stop: &AtomicBool,
    tx: &mpsc::SyncSender<CaptureEvent>,
    root: &std::path::Path,
    fragment_ms: u64,
    readiness: &mut Option<mpsc::Sender<Result<String, String>>>,
) -> Result<(), String> {
    wasapi::initialize_mta()
        .ok()
        .map_err(|error| error.to_string())?;
    let _apartment = ComApartment;
    // Keep the CPAL-selected endpoint. Equal friendly names are ambiguous;
    // fail instead of broadening capture to a different/default endpoint.
    let collection = wasapi::DeviceEnumerator::new()
        .map_err(|e| e.to_string())?
        .get_device_collection(&wasapi::Direction::Render)
        .map_err(|e| e.to_string())?;
    let mut ids = Vec::new();
    for index in 0..collection.get_nbr_devices().map_err(|e| e.to_string())? {
        let device = collection
            .get_device_at_index(index)
            .map_err(|e| e.to_string())?;
        if device.get_friendlyname().map_err(|e| e.to_string())? == name {
            ids.push(device.get_id().map_err(|e| e.to_string())?);
        }
    }
    if ids.len() != 1 {
        return Err("selected system audio endpoint is absent or its name is ambiguous".into());
    }
    let id: Vec<u16> = ids[0].encode_utf16().chain(Some(0)).collect();
    let enumerator: IMMDeviceEnumerator =
        win(unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL) })?;
    let device = win(unsafe { enumerator.GetDevice(PCWSTR(id.as_ptr())) })?;
    let client: IAudioClient = win(unsafe { device.Activate(CLSCTX_ALL, None) })?;
    let format = WAVEFORMATEX {
        wFormatTag: 1,
        nChannels: 2,
        nSamplesPerSec: RATE as u32,
        nAvgBytesPerSec: RATE as u32 * 4,
        nBlockAlign: 4,
        wBitsPerSample: 16,
        cbSize: 0,
    };
    win(unsafe {
        client.Initialize(
            AUDCLNT_SHAREMODE_SHARED,
            AUDCLNT_STREAMFLAGS_LOOPBACK
                | AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM
                | AUDCLNT_STREAMFLAGS_SRC_DEFAULT_QUALITY,
            20_000_000,
            0,
            &format,
            None,
        )
    })?;
    let packets: IAudioCaptureClient = win(unsafe { client.GetService() })?;
    let latency_hns = win(unsafe { client.GetStreamLatency() })?.max(0) as u64;
    let buffer_frames = win(unsafe { client.GetBufferSize() })?;
    let max_future_hns = latency_hns.max(u64::from(buffer_frames) * 10_000_000 / RATE);
    let mut frequency = 0;
    win(unsafe { QueryPerformanceFrequency(&mut frequency) })?;
    let origin_hns = qpc_hns(frequency)?;
    win(unsafe { client.Start() })?;
    let running = RunningClient(client);
    if let Some(ready) = readiness.take() {
        let _ = ready.send(Ok(name.to_string()));
    }
    let mut clock = PacketClock::new(origin_hns, max_future_hns);
    let mut samples = Vec::new();
    let mut timeline = 0;
    let mut expected = 0;
    let mut sequence = 0;
    let mut level = Instant::now();
    let samples_per_chunk = (RATE * fragment_ms / 1000) as usize;
    let result = (|| -> Result<(), String> {
        loop {
            let stopping = stop.load(Ordering::SeqCst);
            if stopping {
                win(unsafe { running.0.Stop() })?;
            }
            while win(unsafe { packets.GetNextPacketSize() })? > 0 {
                let mut ptr = std::ptr::null_mut();
                let (mut frames, mut flags, mut index, mut timestamp) = (0, 0, 0, 0);
                win(unsafe {
                    packets.GetBuffer(
                        &mut ptr,
                        &mut frames,
                        &mut flags,
                        Some(&mut index),
                        Some(&mut timestamp),
                    )
                })?;
                // Always release the native packet before disk/queue work.
                let mono = if frames as u64 > RATE * 2 {
                    Err("system loopback packet exceeds bounded buffer".to_string())
                } else if flags & AUDCLNT_BUFFERFLAGS_SILENT.0 as u32 != 0 {
                    Ok(vec![0; frames as usize])
                } else if ptr.is_null() && frames > 0 {
                    Err("system loopback returned a null non-silent buffer".to_string())
                } else if frames == 0 {
                    Ok(Vec::new())
                } else {
                    let bytes = unsafe { std::slice::from_raw_parts(ptr, frames as usize * 4) };
                    Ok(bytes
                        .chunks_exact(4)
                        .map(|frame| {
                            let left = i16::from_le_bytes([frame[0], frame[1]]) as i32;
                            let right = i16::from_le_bytes([frame[2], frame[3]]) as i32;
                            ((left + right) / 2) as i16
                        })
                        .collect())
                };
                win(unsafe { packets.ReleaseBuffer(frames) })?;
                let mono = mono?;
                if frames == 0 {
                    continue;
                }
                let observed_now_hns = qpc_hns(frequency)?;
                if clock.last_index.is_none() {
                    eprintln!("SYSTEM_FIRST_PACKET timestamp={timestamp} now={observed_now_hns} origin={origin_hns} index={index} frames={frames} flags={flags:#x} latency_hns={latency_hns} buffer_frames={buffer_frames}");
                }
                let (first_sample, silence_before) = clock.position(
                    timestamp,
                    observed_now_hns,
                    index,
                    u64::from(frames),
                    flags & AUDCLNT_BUFFERFLAGS_DATA_DISCONTINUITY.0 as u32 != 0,
                    flags & AUDCLNT_BUFFERFLAGS_TIMESTAMP_ERROR.0 as u32 != 0,
                ).map_err(|error| format!("{error}; timestamp={timestamp} now={observed_now_hns} origin={origin_hns} index={index} frames={frames} flags={flags:#x} latency_hns={latency_hns} buffer_frames={buffer_frames}"))?;
                process_capture_sample_batch_with_gap_reason(
                    CaptureSampleBatch {
                        first_sample,
                        samples: mono,
                        silence_before,
                    },
                    CHANNEL_SYSTEM,
                    RATE as u32,
                    samples_per_chunk,
                    root,
                    tx,
                    &mut samples,
                    &mut timeline,
                    &mut expected,
                    &mut sequence,
                    &mut level,
                    "system_loopback_discontinuity",
                );
            }
            if stopping {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        Ok(())
    })();
    // Errors retain every already accepted sample, including a short tail.
    if !samples.is_empty()
        && (result.is_err() || samples.len() >= (RATE * MIN_FINAL_CHUNK_MS / 1000) as usize)
    {
        let take = samples.len();
        cut_capture_chunk(
            CHANNEL_SYSTEM,
            RATE as u32,
            root,
            tx,
            &mut samples,
            &mut timeline,
            &mut sequence,
            take,
        );
    } else if !samples.is_empty() {
        let _ = tx.send(CaptureEvent::SourceGap {
            channel: CHANNEL_SYSTEM,
            start_ms: (timeline * 1000 / RATE) as i64,
            end_ms: (expected * 1000 / RATE) as i64,
            reason: "final_tail_too_short",
        });
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_nonzero_first_index_keeps_startup_overrun_evidence() {
        let mut clock = PacketClock::new(1_000_000_000, 20_000_000);
        let (first_sample, silence_before) = clock
            .position(1_000_200_000, 1_000_300_000, 480, 480, true, false)
            .unwrap();
        assert_eq!((first_sample, silence_before), (960, None));
        let root = tempfile::tempdir().unwrap();
        let (tx, rx) = mpsc::sync_channel(8);
        let mut samples = Vec::new();
        let (mut timeline, mut expected, mut sequence) = (0, 0, 0);
        let mut level = Instant::now();
        process_capture_sample_batch_with_gap_reason(
            CaptureSampleBatch {
                first_sample,
                samples: vec![123; 480],
                silence_before,
            },
            CHANNEL_SYSTEM,
            RATE as u32,
            480,
            root.path(),
            &tx,
            &mut samples,
            &mut timeline,
            &mut expected,
            &mut sequence,
            &mut level,
            "system_loopback_discontinuity",
        );
        assert!(matches!(
            rx.recv().unwrap(),
            CaptureEvent::SourceGap {
                start_ms: 0,
                end_ms: 20,
                reason: "system_loopback_discontinuity",
                ..
            }
        ));
        assert!(
            matches!(rx.recv().unwrap(), CaptureEvent::Chunk(chunk) if (chunk.start_ms, chunk.end_ms)==(20,30))
        );
    }

    #[test]
    fn native_contiguous_packets_ignore_nominal_qpc_drift() {
        for interval in [99_990, 100_010] {
            let mut clock = PacketClock::new(1_000_000_000, 20_000_000);
            for packet in 0..1000 {
                let timestamp = 1_000_000_000 + packet * interval;
                assert_eq!(
                    clock
                        .position(
                            timestamp,
                            timestamp + 100_000,
                            packet * 480,
                            480,
                            false,
                            false
                        )
                        .unwrap(),
                    (packet * 480, None)
                );
            }
        }
    }

    #[test]
    fn native_first_discontinuity_is_startup_but_later_discontinuity_is_loss() {
        let mut clock = PacketClock::new(1_000_000_000, 20_000_000);
        assert_eq!(
            clock
                .position(1_490_000_000, 1_490_100_000, 0, 480, true, false)
                .unwrap(),
            (2_352_000, Some(0))
        );
        assert_eq!(
            clock
                .position(1_590_000_000, 1_590_100_000, 480_000, 480, true, false)
                .unwrap(),
            (2_832_000, None)
        );
    }

    #[test]
    fn native_discontinuity_reaches_wav_timeline_as_loss_instead_of_silence() {
        let root = tempfile::tempdir().unwrap();
        let (tx, rx) = mpsc::sync_channel(16);
        let mut clock = PacketClock::new(1_000_000_000, 20_000_000);
        let mut samples = Vec::new();
        let (mut timeline, mut expected, mut sequence) = (0, 0, 0);
        let mut level = Instant::now();
        for (timestamp, index, discontinuity) in [
            (1_000_000_000, 0, false),
            (1_000_200_000, 960, false),
            (1_000_600_000, 2880, true),
        ] {
            let (first_sample, silence_before) = clock
                .position(
                    timestamp,
                    timestamp + 100_000,
                    index,
                    480,
                    discontinuity,
                    false,
                )
                .unwrap();
            process_capture_sample_batch_with_gap_reason(
                CaptureSampleBatch {
                    first_sample,
                    samples: vec![123; 480],
                    silence_before,
                },
                CHANNEL_SYSTEM,
                RATE as u32,
                480,
                root.path(),
                &tx,
                &mut samples,
                &mut timeline,
                &mut expected,
                &mut sequence,
                &mut level,
                "system_loopback_discontinuity",
            );
        }
        let events: Vec<_> = rx.try_iter().collect();
        let gaps: Vec<_> = events
            .iter()
            .filter_map(|event| match event {
                CaptureEvent::SourceGap {
                    start_ms,
                    end_ms,
                    reason,
                    ..
                } => Some((*start_ms, *end_ms, *reason)),
                _ => None,
            })
            .collect();
        assert_eq!(gaps, vec![(30, 60, "system_loopback_discontinuity")]);
        let chunks: Vec<_> = events
            .iter()
            .filter_map(|event| match event {
                CaptureEvent::Chunk(chunk) => Some(chunk),
                _ => None,
            })
            .collect();
        assert_eq!(
            chunks
                .iter()
                .map(|chunk| (chunk.start_ms, chunk.end_ms))
                .collect::<Vec<_>>(),
            vec![(0, 10), (10, 20), (20, 30), (60, 70)]
        );
        assert!(hound::WavReader::open(&chunks[1].file_path)
            .unwrap()
            .samples::<i16>()
            .all(|sample| sample.unwrap() == 0));
        assert!(crate::live_meeting::capture_outcome(70, gaps.len(), 0, 0)
            .failure_reason
            .is_some());
    }

    #[test]
    fn native_packet_clock_preserves_idle_but_never_hides_flagged_loss() {
        let mut clock = PacketClock::new(1_000_000_000, 20_000_000);
        // 49s render idle, then 10ms of actual samples at 48kHz.
        assert_eq!(
            clock
                .position(1_490_000_000, 1_490_100_000, 2_352_000, 480, false, false)
                .unwrap(),
            (2_352_000, Some(0))
        );
        assert_eq!(
            clock
                .position(1_490_100_000, 1_490_800_000, 2_352_480, 480, false, false)
                .unwrap(),
            (2_352_480, None)
        );
        // Clean resumed packet: callback delivery delay cannot alter its offset.
        assert_eq!(
            clock
                .position(1_590_000_000, 1_590_100_000, 2_832_000, 480, false, false)
                .unwrap(),
            (2_832_000, Some(2_352_960))
        );
        // A native discontinuity retains the gap as loss, never silence.
        assert_eq!(
            clock
                .position(1_690_000_000, 1_690_100_000, 3_312_000, 480, true, false)
                .unwrap(),
            (3_312_000, None)
        );
        assert!(clock
            .position(1_690_100_000, 1_690_200_000, 3_312_480, 480, false, true)
            .is_err());
        assert!(clock
            .position(1_690_000_000, 1_690_200_000, 3_312_000, 480, false, false)
            .is_err());
        assert!(clock
            .position(9_000_000_000, 1_690_200_000, 3_312_480, 480, false, false)
            .is_err());
    }
}
