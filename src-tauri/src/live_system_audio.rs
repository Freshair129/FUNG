//! Native endpoint loopback retains packet flags that CPAL does not expose.
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};

use crate::live_meeting::{
    flush_capture_tail, process_capture_sample_batch_with_gap_reason, CaptureEvent, CaptureReady,
    CaptureSampleBatch, CHANNEL_SYSTEM,
};
use windows::core::{Interface, PCWSTR};
use windows::Win32::Media::Audio::*;
use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_ALL};
use windows::Win32::System::Performance::{QueryPerformanceCounter, QueryPerformanceFrequency};

const RATE: u64 = 48_000;

use crate::endpoint_packet_sink::{EndpointPacketProducer, PacketStamp};
fn validate_sink_selection(
    selection: &EndpointSelection,
    sink: &Option<EndpointPacketProducer>,
) -> Result<(), String> {
    if let Some(sink) = sink {
        if *selection != EndpointSelection::ExactId(sink.scope().endpoint_id().to_owned()) {
            return Err("source packet sink must use its own explicitly bound endpoint ID".into());
        }
    }
    Ok(())
}
struct NativePacketObservation {
    index: u64,
    qpc: u64,
    observed_now_hns: u64,
    flags: u32,
}
fn dispatch_endpoint_packet(
    packet_sink: &mut Option<EndpointPacketProducer>,
    batch: CaptureSampleBatch,
    observation: NativePacketObservation,
    legacy: impl FnOnce(CaptureSampleBatch),
) -> Result<(), String> {
    if let Some(sink) = packet_sink.as_mut() {
        sink.push_native(
            PacketStamp {
                first_sample: batch.first_sample,
                silence_before: batch.silence_before,
                native_index: observation.index,
                native_qpc_hns: observation.qpc,
                observed_qpc_hns: observation.observed_now_hns,
                native_flags: observation.flags,
            },
            batch.samples,
        )
        .map_err(|fault| format!("source packet sink failed: {fault:?}"))
    } else {
        legacy(batch);
        Ok(())
    }
}
#[cfg(test)]
mod packet_seam_tests {
    use super::*;
    use crate::endpoint_packet_sink::{packet_channel, FrameSpan, SinkRead, SourcePacketScope};
    #[test]
    fn source_sink_requires_its_own_explicit_endpoint_before_native_activation() {
        let scope =
            SourcePacketScope::new(uuid::Uuid::new_v4(), 1, "opaque-A".into(), 48000, 1).unwrap();
        let (producer, _receiver) = packet_channel(scope, 1).unwrap();
        let sink = Some(producer);
        assert!(validate_sink_selection(
            &EndpointSelection::LegacyFriendlyName("Speakers".into()),
            &sink
        )
        .is_err());
        assert!(
            validate_sink_selection(&EndpointSelection::ExactId("opaque-B".into()), &sink).is_err()
        );
        assert!(
            validate_sink_selection(&EndpointSelection::ExactId("opaque-A".into()), &sink).is_ok()
        );
        assert!(validate_sink_selection(
            &EndpointSelection::LegacyFriendlyName("Speakers".into()),
            &None
        )
        .is_ok());
    }
    #[test]
    fn raw_source_sink_retains_one_frame_tail_and_native_flags_off_legacy_writer() {
        const N: u64 = 10_000_000;
        const B: u64 = N + 10_000;
        let scope =
            SourcePacketScope::new(uuid::Uuid::new_v4(), 1, "opaque".into(), 48000, B).unwrap();
        let (mut producer, mut receiver) = packet_channel(scope, 2).unwrap();
        producer.bind_native_clock(N, 48000).unwrap();
        producer.confirm_native_ready(N + 1000).unwrap();
        producer
            .push_native(
                PacketStamp {
                    first_sample: 0,
                    silence_before: None,
                    native_index: 1,
                    native_qpc_hns: N,
                    observed_qpc_hns: B - 1,
                    native_flags: 0,
                },
                vec![9; 48],
            )
            .unwrap();
        let mut sink = Some(producer);
        dispatch_endpoint_packet(
            &mut sink,
            CaptureSampleBatch {
                first_sample: 48,
                samples: vec![0],
                silence_before: None,
            },
            NativePacketObservation {
                index: 777,
                qpc: B,
                observed_now_hns: B + 1,
                flags: 2,
            },
            |_| panic!("source packets must not enter legacy disk writer"),
        )
        .unwrap();
        sink.take().unwrap().finish(Ok(()));
        let SinkRead::Packet(packet) = receiver.recv_timeout(Duration::ZERO).unwrap() else {
            panic!()
        };
        assert_eq!(packet.frames, FrameSpan { start: 0, end: 1 });
        assert_eq!(packet.samples, vec![0]);
        assert_eq!(packet.native_stamp.native_flags, 2);
        assert_eq!(packet.native_stamp.native_index, 777);
        assert_eq!(packet.native_stamp.native_qpc_hns, B);
        assert_eq!(packet.native_stamp.observed_qpc_hns, B + 1);
    }
    #[test]
    fn no_source_sink_keeps_existing_legacy_batch_and_idle_metadata() {
        let mut called = false;
        dispatch_endpoint_packet(
            &mut None,
            CaptureSampleBatch {
                first_sample: 960,
                samples: vec![9; 480],
                silence_before: Some(480),
            },
            NativePacketObservation {
                index: 7,
                qpc: 8,
                observed_now_hns: 9,
                flags: 0,
            },
            |batch| {
                called = true;
                assert_eq!(batch.first_sample, 960);
                assert_eq!(batch.samples, vec![9; 480]);
                assert_eq!(batch.silence_before, Some(480));
            },
        )
        .unwrap();
        assert!(called);
    }
}

/// A legacy display-name choice is deliberately distinct from a native opaque ID.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum EndpointSelection {
    LegacyFriendlyName(String),
    ExactId(String),
}

fn resolve_endpoint_id(
    selection: &EndpointSelection,
    legacy_matches: impl FnOnce(&str) -> Result<Vec<String>, String>,
    exact_lookup: impl FnOnce(&str) -> Result<String, String>,
) -> Result<String, String> {
    match selection {
        EndpointSelection::LegacyFriendlyName(name) => {
            let ids = legacy_matches(name)?;
            if ids.len() != 1 {
                return Err(
                    "selected system audio endpoint is absent or its name is ambiguous".into(),
                );
            }
            Ok(ids[0].clone())
        }
        EndpointSelection::ExactId(id) => {
            if id.is_empty() || id.len() > 4096 || id.contains('\0') {
                return Err("invalid opaque audio endpoint ID".into());
            }
            let observed = exact_lookup(id)?;
            if observed != *id {
                return Err("native endpoint identity changed during lookup".into());
            }
            Ok(observed)
        }
    }
}

#[cfg(test)]
mod selection_tests {
    use super::*;
    #[test]
    fn legacy_duplicate_name_still_fails_and_never_uses_default() {
        let result = resolve_endpoint_id(
            &EndpointSelection::LegacyFriendlyName("Speakers".into()),
            |_| Ok(vec!["id-A".into(), "id-B".into()]),
            |_| panic!("no exact fallback"),
        );
        assert!(result.is_err());
    }
    #[test]
    fn exact_id_bypasses_duplicate_or_broken_friendly_name_inventory() {
        let result = resolve_endpoint_id(
            &EndpointSelection::ExactId("opaque-A".into()),
            |_| panic!("opaque selection must not enumerate/match names"),
            |id| Ok(id.into()),
        );
        assert_eq!(result.unwrap(), "opaque-A");
    }
    #[test]
    fn exact_missing_id_never_falls_back_to_a_name_or_default() {
        let result = resolve_endpoint_id(
            &EndpointSelection::ExactId("missing".into()),
            |_| panic!("no name fallback"),
            |_| Err("native GetDevice: absent".into()),
        );
        assert_eq!(result.unwrap_err(), "native GetDevice: absent");
    }
    #[test]
    fn exact_alias_roundtrip_mismatch_is_not_silently_accepted() {
        let result = resolve_endpoint_id(
            &EndpointSelection::ExactId("opaque-A".into()),
            |_| panic!("no enumeration"),
            |_| Ok("opaque-B".into()),
        );
        assert!(result.unwrap_err().contains("identity"));
    }
    #[test]
    fn exact_invalid_ids_are_rejected_before_native_lookup() {
        for id in ["".into(), "bad\0id".into(), "x".repeat(4097)] {
            assert!(resolve_endpoint_id(
                &EndpointSelection::ExactId(id),
                |_| Ok(vec!["wrong".into()]),
                |_| panic!("invalid input must not reach native lookup")
            )
            .is_err());
        }
    }
    #[test]
    fn legacy_unique_friendly_name_preserves_existing_selection() {
        let result = resolve_endpoint_id(
            &EndpointSelection::LegacyFriendlyName("Speakers".into()),
            |name| {
                assert_eq!(name, "Speakers");
                Ok(vec!["old-ID".into()])
            },
            |_| panic!("legacy selection must retain old unique-name behavior"),
        );
        assert_eq!(result.unwrap(), "old-ID");
    }
}

struct PacketClock {
    origin_hns: u64,
    end: u64,
    last_index: Option<u64>,
    last_timestamp: Option<u64>,
    previous_frames: u64,
    max_future_hns: u64,
    startup_buffer_hns: u64,
    latest_empty_hns: u64,
}

impl PacketClock {
    fn new(origin_hns: u64, max_future_hns: u64, startup_buffer_hns: u64) -> Self {
        Self {
            origin_hns,
            end: 0,
            last_index: None,
            last_timestamp: None,
            previous_frames: 0,
            max_future_hns,
            startup_buffer_hns,
            latest_empty_hns: origin_hns,
        }
    }
    fn observe_empty(&mut self, before_query_hns: u64) -> Result<(), String> {
        if before_query_hns < self.latest_empty_hns {
            return Err("system loopback observation clock went backwards".into());
        }
        if self.last_timestamp.is_none() {
            self.latest_empty_hns = before_query_hns;
        }
        Ok(())
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
        let since_empty = now
            .checked_sub(self.latest_empty_hns)
            .ok_or("system loopback observation clock went backwards")?;
        let startup_packet = first_packet && since_empty <= self.startup_buffer_hns;
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
        // Loopback may attach to an already-running renderer, so its first
        // native position need not be zero. A first flag is a startup marker
        // only while recent empty-buffer/start observation rules out starvation.
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
    spawn_selected_endpoint_capture_thread(
        EndpointSelection::LegacyFriendlyName(name),
        stop,
        tx,
        root,
        fragment_ms,
        None,
    )
    .map(|ready| CaptureReady {
        device_name: ready.device_name,
    })
}

/// Shared reader entry point. Exact selection never enumerates display names or
/// follows a current default. A routed caller must provide a source-private root.
pub(crate) fn spawn_selected_endpoint_capture_thread(
    selection: EndpointSelection,
    stop: Arc<AtomicBool>,
    tx: mpsc::SyncSender<CaptureEvent>,
    root: PathBuf,
    fragment_ms: u64,
    packet_sink: Option<EndpointPacketProducer>,
) -> Result<CaptureReady, String> {
    validate_sink_selection(&selection, &packet_sink)?;
    if fragment_ms == 0 || fragment_ms > 60_000 {
        return Err("endpoint fragment duration outside supported bounds".into());
    }
    let (ready_tx, ready_rx) = mpsc::channel();
    std::thread::Builder::new()
        .name("fung-endpoint-loopback".into())
        .spawn(move || {
            let mut readiness = Some(ready_tx);
            let result = capture_selected(
                &selection,
                &stop,
                &tx,
                &root,
                fragment_ms,
                &mut readiness,
                packet_sink,
            );
            if let Some(ready) = readiness {
                let _ = ready.send(Err(result
                    .err()
                    .unwrap_or_else(|| "endpoint capture stopped before start".into())));
            } else if let Err(error) = result {
                let _ = tx.send(CaptureEvent::StreamFailed {
                    channel: CHANNEL_SYSTEM,
                    error,
                    terminal: true,
                });
            }
        })
        .map_err(|error| format!("start endpoint capture thread: {error}"))?;
    ready_rx
        .recv_timeout(Duration::from_secs(10))
        .map_err(|error| format!("endpoint capture readiness: {error}"))?
}

fn capture_selected(
    selection: &EndpointSelection,
    stop: &AtomicBool,
    tx: &mpsc::SyncSender<CaptureEvent>,
    root: &std::path::Path,
    fragment_ms: u64,
    readiness: &mut Option<mpsc::Sender<Result<CaptureReady, String>>>,
    mut packet_sink: Option<EndpointPacketProducer>,
) -> Result<(), String> {
    let result = capture_selected_inner(
        selection,
        stop,
        tx,
        root,
        fragment_ms,
        readiness,
        &mut packet_sink,
    );
    // The inner reader's native client has stopped/dropped before closing this
    // private queue. On any early setup or packet error preserve its real reason.
    if let Some(sink) = packet_sink {
        sink.finish(result.clone());
    }
    result
}

fn capture_selected_inner(
    selection: &EndpointSelection,
    stop: &AtomicBool,
    tx: &mpsc::SyncSender<CaptureEvent>,
    root: &std::path::Path,
    fragment_ms: u64,
    readiness: &mut Option<mpsc::Sender<Result<CaptureReady, String>>>,
    packet_sink: &mut Option<EndpointPacketProducer>,
) -> Result<(), String> {
    wasapi::initialize_mta()
        .ok()
        .map_err(|error| error.to_string())?;
    let _apartment = ComApartment;
    let enumerator: IMMDeviceEnumerator =
        win(unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL) })?;
    let endpoint_id = resolve_endpoint_id(
        selection,
        |name| {
            // Preserve the legacy unique-friendly-name behavior, including errors.
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
            Ok(ids)
        },
        |id| {
            let id: Vec<u16> = id.encode_utf16().chain(Some(0)).collect();
            let device = win(unsafe { enumerator.GetDevice(PCWSTR(id.as_ptr())) })?;
            wasapi::Device::from_immdevice(device)
                .map_err(|e| e.to_string())?
                .get_id()
                .map_err(|e| e.to_string())
        },
    )?;
    let id: Vec<u16> = endpoint_id.encode_utf16().chain(Some(0)).collect();
    let device = win(unsafe { enumerator.GetDevice(PCWSTR(id.as_ptr())) })?;
    if win(unsafe { device.GetState() })? != DEVICE_STATE_ACTIVE {
        return Err("selected endpoint is not active".into());
    }
    let flow: IMMEndpoint = win(device.cast())?;
    if win(unsafe { flow.GetDataFlow() })? != eRender {
        return Err("selected endpoint is not a render device".into());
    }
    let wrapper = wasapi::Device::from_immdevice(device.clone()).map_err(|e| e.to_string())?;
    if wrapper.get_id().map_err(|e| e.to_string())? != endpoint_id {
        return Err("selected endpoint identity changed".into());
    }
    let device_name = wrapper.get_friendlyname().map_err(|e| e.to_string())?;
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
    let buffer_duration_hns = u64::from(buffer_frames) * 10_000_000 / RATE;
    if buffer_duration_hns == 0 {
        return Err("system loopback reported zero buffer capacity".into());
    }
    let max_future_hns = latency_hns.max(buffer_duration_hns);
    let mut frequency = 0;
    win(unsafe { QueryPerformanceFrequency(&mut frequency) })?;
    let origin_hns = qpc_hns(frequency)?;
    if let Some(sink) = packet_sink.as_mut() {
        sink.bind_native_clock(origin_hns, RATE as u32)
            .map_err(|fault| format!("source packet clock binding failed: {fault:?}"))?;
    }
    win(unsafe { client.Start() })?;
    let running = RunningClient(client);
    if let Some(sink) = packet_sink.as_mut() {
        sink.confirm_native_ready(qpc_hns(frequency)?)
            .map_err(|fault| format!("source missed native-ready deadline: {fault:?}"))?;
    }
    if let Some(ready) = readiness.take() {
        let _ = ready.send(Ok(CaptureReady { device_name }));
    }
    let mut clock = PacketClock::new(origin_hns, max_future_hns, buffer_duration_hns);
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
            loop {
                // Timestamp before querying: preemption after the API returns
                // must not make an old empty observation look artificially fresh.
                let before_query_hns = qpc_hns(frequency)?;
                if win(unsafe { packets.GetNextPacketSize() })? == 0 {
                    clock.observe_empty(before_query_hns)?;
                    break;
                }
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
                        .as_chunks::<4>()
                        .0
                        .iter()
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
                    eprintln!("SYSTEM_FIRST_PACKET timestamp={timestamp} now={observed_now_hns} origin={origin_hns} index={index} frames={frames} flags={flags:#x} latency_hns={latency_hns} buffer_frames={buffer_frames} last_empty_hns={} startup_buffer_hns={buffer_duration_hns}", clock.latest_empty_hns);
                }
                let (first_sample, silence_before) = clock.position(
                    timestamp,
                    observed_now_hns,
                    index,
                    u64::from(frames),
                    flags & AUDCLNT_BUFFERFLAGS_DATA_DISCONTINUITY.0 as u32 != 0,
                    flags & AUDCLNT_BUFFERFLAGS_TIMESTAMP_ERROR.0 as u32 != 0,
                ).map_err(|error| format!("{error}; timestamp={timestamp} now={observed_now_hns} origin={origin_hns} index={index} frames={frames} flags={flags:#x} latency_hns={latency_hns} buffer_frames={buffer_frames} last_empty_hns={} startup_buffer_hns={buffer_duration_hns}", clock.latest_empty_hns))?;
                dispatch_endpoint_packet(
                    packet_sink,
                    CaptureSampleBatch {
                        first_sample,
                        samples: mono,
                        silence_before,
                    },
                    NativePacketObservation {
                        index,
                        qpc: timestamp,
                        observed_now_hns,
                        flags,
                    },
                    |batch| {
                        process_capture_sample_batch_with_gap_reason(
                            batch,
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
                        )
                    },
                )?;
            }
            if stopping {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        Ok(())
    })();
    // Preserve legacy tails exactly as before. Source PCM packets have already
    // transferred ownership into the bounded sink, including a one-frame tail.
    if packet_sink.is_none() {
        flush_capture_tail(
            CHANNEL_SYSTEM,
            RATE as u32,
            root,
            tx,
            &mut samples,
            &mut timeline,
            &mut sequence,
        );
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_already_playing_engine_establishes_arbitrary_first_index_baseline() {
        let mut clock = PacketClock::new(3_564_052_695_895, 20_000_000, 20_000_000);
        // Actual active-engine positive control: the renderer predates this capture client.
        assert_eq!(
            clock
                .position(
                    3_564_053_142_311,
                    3_564_052_806_721,
                    4_137_600,
                    480,
                    true,
                    false
                )
                .unwrap(),
            (2143, Some(0))
        );
    }

    #[test]
    fn native_starved_initial_buffer_keeps_loss_evidence_even_at_index_zero() {
        let mut clock = PacketClock::new(1_000_000_000, 20_000_000, 20_000_000);
        let (first_sample, silence_before) = clock
            .position(1_030_000_000, 1_030_100_000, 0, 480, true, false)
            .unwrap();
        assert_eq!((first_sample, silence_before), (144_000, None));
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
                end_ms: 3000,
                reason: "system_loopback_discontinuity",
                ..
            }
        ));
        assert!(
            matches!(rx.recv().unwrap(), CaptureEvent::Chunk(chunk) if (chunk.start_ms, chunk.end_ms)==(3000,3010))
        );
    }

    #[test]
    fn native_contiguous_packets_ignore_nominal_qpc_drift() {
        for interval in [99_990, 100_010] {
            let mut clock = PacketClock::new(1_000_000_000, 20_000_000, 20_000_000);
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
        let mut clock = PacketClock::new(1_000_000_000, 20_000_000, 20_000_000);
        clock.observe_empty(1_489_900_000).unwrap();
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
        let mut clock = PacketClock::new(1_000_000_000, 20_000_000, 20_000_000);
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
        assert!(
            crate::live_meeting::capture_outcome(70, gaps.len(), 0, 0, 0)
                .failure_reason
                .is_some()
        );
    }

    #[test]
    fn native_packet_clock_preserves_idle_but_never_hides_flagged_loss() {
        let mut clock = PacketClock::new(1_000_000_000, 20_000_000, 20_000_000);
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
