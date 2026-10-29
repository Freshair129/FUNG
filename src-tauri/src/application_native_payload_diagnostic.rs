//! Test-only bounded original stereo and decoded mono custody before clock/queue/writer.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::Serialize;
use sha2::{Digest, Sha256};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum AcquisitionStrategy {
    SizeHintGated,
    DirectGetBuffer,
}

struct ProbeRequest {
    root: PathBuf,
    strategy: AcquisitionStrategy,
}

thread_local! {
    static REQUEST: RefCell<Option<ProbeRequest>> = const { RefCell::new(None) };
}

pub(super) struct ProbeRequestGuard(Option<ProbeRequest>);

impl Drop for ProbeRequestGuard {
    fn drop(&mut self) {
        REQUEST.with(|request| *request.borrow_mut() = self.0.take());
    }
}

/// Only the explicitly ignored owned probe arms this caller-local, single-use request.
pub(super) fn request(root: PathBuf) -> ProbeRequestGuard {
    request_with_strategy(root, AcquisitionStrategy::SizeHintGated)
}

pub(super) fn request_with_strategy(
    root: PathBuf,
    strategy: AcquisitionStrategy,
) -> ProbeRequestGuard {
    ProbeRequestGuard(
        REQUEST.with(|request| request.replace(Some(ProbeRequest { root, strategy }))),
    )
}

pub(super) fn take_request() -> Option<NativePayloadDiagnostic> {
    REQUEST
        .with(|request| request.borrow_mut().take())
        .map(|request| {
            let mut trace = NativePayloadDiagnostic::new(24 * 48_000, 8192);
            trace.root = Some(request.root);
            trace.acquisition_strategy = request.strategy;
            trace
        })
}

#[derive(Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct PacketReceipt {
    offset_frames: usize,
    frames: usize,
    flags: u32,
    device_index: u64,
    qpc_hns: u64,
    observed_elapsed_ns: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct OriginalPacketReceipt {
    offset_bytes: usize,
    byte_length: usize,
    frames: u32,
    flags: u32,
    device_index: u64,
    qpc_hns: u64,
    observed_elapsed_ns: u64,
    kind: &'static str,
}

pub(super) struct OriginalPacketObservation<'a> {
    pub(super) trace: &'a mut NativePayloadDiagnostic,
    pub(super) index: u64,
    pub(super) qpc: u64,
    // Read-entry observation sampled before GetBuffer, not native QPC or exact copy time.
    pub(super) elapsed: Duration,
}

impl OriginalPacketObservation<'_> {
    /// Same valid acquired-buffer contract as the native decoder. Never reads
    /// empty, SILENT, null or oversized pointers.
    pub(super) unsafe fn record(self, pointer: *const u8, frames: u32, flags: u32) {
        unsafe {
            self.trace
                .record_original(pointer, frames, flags, self.index, self.qpc, self.elapsed);
        }
    }
}

pub(super) struct NativePayloadDiagnostic {
    samples: Vec<i16>,
    packets: Vec<PacketReceipt>,
    max_frames: usize,
    max_packets: usize,
    invalid_reason: Option<&'static str>,
    observed_packets: u64,
    observed_frames: u64,
    native_stop: Option<Result<(), String>>,
    root: Option<PathBuf>,
    original_bytes: Vec<u8>,
    original_packets: Vec<OriginalPacketReceipt>,
    original_invalid_reason: Option<&'static str>,
    original_observed_frames: u64,
    original_observed_packets: u64,
    original_retained_frames: usize,
    acquisition_strategy: AcquisitionStrategy,
}

impl NativePayloadDiagnostic {
    fn new(max_frames: usize, max_packets: usize) -> Self {
        Self {
            samples: Vec::with_capacity(max_frames),
            packets: Vec::with_capacity(max_packets),
            max_frames,
            max_packets,
            invalid_reason: None,
            observed_packets: 0,
            observed_frames: 0,
            native_stop: None,
            root: None,
            original_bytes: Vec::with_capacity(max_frames * 4),
            original_packets: Vec::with_capacity(max_packets),
            original_invalid_reason: None,
            original_observed_frames: 0,
            original_observed_packets: 0,
            original_retained_frames: 0,
            acquisition_strategy: AcquisitionStrategy::SizeHintGated,
        }
    }

    pub(super) fn acquisition_strategy(&self) -> AcquisitionStrategy {
        self.acquisition_strategy
    }

    /// Copies original interleaved PCM16 stereo while WASAPI still owns the
    /// acquired buffer. No allocation, I/O, serialization or hashing here.
    unsafe fn record_original(
        &mut self,
        pointer: *const u8,
        frames: u32,
        flags: u32,
        index: u64,
        qpc: u64,
        elapsed: Duration,
    ) {
        self.original_observed_packets = self.original_observed_packets.saturating_add(1);
        self.original_observed_frames = self
            .original_observed_frames
            .saturating_add(u64::from(frames));
        if self.original_invalid_reason.is_some() {
            return;
        }
        let kind = if frames > 96_000 {
            "invalid_oversize"
        } else if frames == 0 {
            "empty"
        } else if flags & 2 != 0 {
            "silent"
        } else if pointer.is_null() {
            "invalid_null"
        } else {
            "payload"
        };
        if self.original_packets.len() == self.max_packets
            || frames as usize > self.max_frames - self.original_retained_frames
        {
            self.original_invalid_reason =
                Some("bounded_original_payload_overrun; retained prefix is not complete evidence");
            return;
        }
        let offset_bytes = self.original_bytes.len();
        let byte_length = if kind == "payload" {
            frames as usize * 4
        } else {
            0
        };
        if byte_length != 0 {
            // Native contract guarantees a valid 4-byte stereo frame span only
            // for the bounded non-SILENT/non-null case checked above.
            let bytes = unsafe { std::slice::from_raw_parts(pointer, byte_length) };
            self.original_bytes.extend_from_slice(bytes);
        }
        self.original_packets.push(OriginalPacketReceipt {
            offset_bytes,
            byte_length,
            frames,
            flags,
            device_index: index,
            qpc_hns: qpc,
            observed_elapsed_ns: elapsed.as_nanos().min(u128::from(u64::MAX)) as u64,
            kind,
        });
        self.original_retained_frames += frames as usize;
        if kind == "invalid_null" || kind == "invalid_oversize" {
            self.original_invalid_reason = Some("invalid_original_packet; no bytes dereferenced");
        }
    }

    pub(super) fn record(
        &mut self,
        mono: &[i16],
        index: u64,
        flags: u32,
        qpc: u64,
        elapsed: Duration,
    ) {
        self.observed_packets = self.observed_packets.saturating_add(1);
        self.observed_frames = self.observed_frames.saturating_add(mono.len() as u64);
        if self.invalid_reason.is_some() {
            return;
        }
        if mono.len() > self.max_frames - self.samples.len()
            || self.packets.len() == self.max_packets
        {
            self.invalid_reason =
                Some("bounded_native_payload_overrun; retained prefix is not complete evidence");
            return;
        }
        self.packets.push(PacketReceipt {
            offset_frames: self.samples.len(),
            frames: mono.len(),
            flags,
            device_index: index,
            qpc_hns: qpc,
            observed_elapsed_ns: elapsed.as_nanos().min(u128::from(u64::MAX)) as u64,
        });
        self.samples.extend_from_slice(mono);
    }

    pub(super) fn stopped(&mut self, result: &Result<(), String>) {
        self.native_stop = Some(result.clone());
    }

    /// Called only after the shared native driver returned and the writer joined.
    /// No filesystem, serialization or hashing runs on the native read path.
    pub(super) fn flush_after_stop(
        self,
        process_id: u32,
        capture_result: &Result<(), String>,
        writer_result: &Result<(), String>,
    ) -> Result<(), String> {
        let root = self
            .root
            .as_deref()
            .ok_or("native payload probe has no evidence directory")?;
        let pcm: Vec<u8> = self
            .samples
            .iter()
            .flat_map(|sample| sample.to_le_bytes())
            .collect();
        let binary = std::env::current_exe().map_err(|error| error.to_string())?;
        let binary_hash = hash_file(&binary)?;
        let report = serde_json::json!({
            "schema": "fung-decoded-native-mono-v1",
            "acquisitionStrategy": self.acquisition_strategy,
            "boundary": "read_application_packet returned after decode/ReleaseBuffer; before empty filtering, ApplicationPacketClock, queue and writer",
            "format": { "encoding": "pcm-s16le", "sampleRate": 48000, "channels": 1, "sourceChannels": 2, "downmix": "(i32(left)+i32(right))/2; SILENT decoded to zero" },
            "processId": process_id,
            "pcmFile": "decoded-native-mono.pcm",
            "pcmSha256": format!("{:x}", Sha256::digest(&pcm)),
            "pcmByteLength": pcm.len(),
            "diagnosticValid": self.invalid_reason.is_none(),
            "invalidReason": self.invalid_reason,
            "maxFrames": self.max_frames,
            "maxPackets": self.max_packets,
            "observedPackets": self.observed_packets,
            "observedFrames": self.observed_frames,
            "retainedFrames": self.samples.len(),
            "packets": self.packets,
            "nativeStopResult": self.native_stop,
            "captureResult": capture_result,
            "writerResult": writer_result,
            "probeBinary": binary,
            "probeBinarySha256": binary_hash,
            "captureSourceSha256": format!("{:x}", Sha256::digest(include_bytes!("live_application_audio.rs"))),
            "diagnosticSourceSha256": format!("{:x}", Sha256::digest(include_bytes!("application_native_payload_diagnostic.rs"))),
            "packetDriverSourceSha256": format!("{:x}", Sha256::digest(include_bytes!("application_packet_driver.rs"))),
            "scope": "test-only decoded native mono custody; no raw stereo byte, installed audio acceptance or PID repair claim"
        });
        write_new(root, "decoded-native-mono.pcm", &pcm)?;
        write_new(
            root,
            "decoded-native-mono.json",
            &serde_json::to_vec_pretty(&report).map_err(|error| error.to_string())?,
        )?;
        let original_report = serde_json::json!({
            "schema": "fung-original-native-stereo-v1",
            "acquisitionStrategy": self.acquisition_strategy,
            "boundary": "successful GetBuffer while acquired buffer is valid; copied before decoder/downmix and ReleaseBuffer",
            "format": {"encoding": "pcm-s16le", "sampleRate":48000,"channels":2,"bytesPerFrame":4},
            "processId": process_id,
            "pcmFile": "original-native-stereo.pcm",
            "pcmSha256": format!("{:x}", Sha256::digest(&self.original_bytes)),
            "pcmByteLength": self.original_bytes.len(),
            "diagnosticValid": self.original_invalid_reason.is_none(),
            "invalidReason": self.original_invalid_reason,
            "maxFrames": self.max_frames,
            "maxBytes": self.max_frames * 4,
            "maxPackets": self.max_packets,
            "observedPackets": self.original_observed_packets,
            "observedFrames": self.original_observed_frames,
            "retainedFrames": self.original_retained_frames,
            "packets": self.original_packets,
            "nativeStopResult": self.native_stop,
            "captureResult": capture_result,
            "writerResult": writer_result,
            "probeBinary": binary,
            "probeBinarySha256": binary_hash,
            "captureSourceSha256": format!("{:x}", Sha256::digest(include_bytes!("live_application_audio.rs"))),
            "diagnosticSourceSha256": format!("{:x}", Sha256::digest(include_bytes!("application_native_payload_diagnostic.rs"))),
            "packetDriverSourceSha256": format!("{:x}", Sha256::digest(include_bytes!("application_packet_driver.rs"))),
            "association": "one raw receipt per successful GetBuffer; mono receipt follows only successful decode/release; pair ordinals only with matching headers/counts and successful lifecycle outcomes",
            "observedElapsedNsMeaning": "capture-thread elapsed sampled at read entry before GetBuffer; not exact buffer-copy time or native packet clock",
            "scope": "test-only original buffer custody; SILENT has metadata only and no invented raw zero bytes; no native or installed acceptance/PID repair claim"
        });
        write_new(root, "original-native-stereo.pcm", &self.original_bytes)?;
        write_new(
            root,
            "original-native-stereo.json",
            &serde_json::to_vec_pretty(&original_report).map_err(|error| error.to_string())?,
        )
    }
}

fn hash_file(path: &Path) -> Result<String, String> {
    use std::io::Read;
    let mut file = std::fs::File::open(path).map_err(|error| error.to_string())?;
    let mut hash = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let count = file.read(&mut buffer).map_err(|error| error.to_string())?;
        if count == 0 {
            return Ok(format!("{:x}", hash.finalize()));
        }
        hash.update(&buffer[..count]);
    }
}

fn write_new(root: &Path, name: &str, bytes: &[u8]) -> Result<(), String> {
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(root.join(name))
        .map_err(|error| error.to_string())?;
    file.write_all(bytes).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn direct_acquisition_request_is_scoped_single_use_and_strategy_is_receipted() {
        assert!(take_request().is_none());
        let root = tempfile::tempdir().unwrap();
        {
            let _outer = request(PathBuf::from("outer-size-hint"));
            {
                let _direct = request_with_strategy(
                    root.path().to_path_buf(),
                    AcquisitionStrategy::DirectGetBuffer,
                );
                let mut trace = take_request().unwrap();
                assert_eq!(
                    trace.acquisition_strategy(),
                    AcquisitionStrategy::DirectGetBuffer
                );
                assert!(take_request().is_none());
                std::thread::spawn(|| assert!(take_request().is_none()))
                    .join()
                    .unwrap();
                let bytes = [1u8, 0, 255, 255];
                unsafe {
                    trace.record_original(bytes.as_ptr(), 1, 0, 7, 12, Duration::ZERO);
                }
                trace.record(&[0], 7, 0, 12, Duration::ZERO);
                // Direct drain's terminal empty acquisition has no native packet
                // timestamp; both collectors retain that explicit empty receipt.
                unsafe {
                    trace.record_original(std::ptr::null(), 0, 0, 0, 0, Duration::ZERO);
                }
                trace.record(&[], 0, 0, 0, Duration::ZERO);
                trace.stopped(&Ok(()));
                trace.flush_after_stop(42, &Ok(()), &Ok(())).unwrap();
                for file in ["decoded-native-mono.json", "original-native-stereo.json"] {
                    let receipt: serde_json::Value =
                        serde_json::from_slice(&std::fs::read(root.path().join(file)).unwrap())
                            .unwrap();
                    assert_eq!(receipt["acquisitionStrategy"], "direct_get_buffer");
                    assert_eq!(receipt["observedPackets"], 2);
                    assert_eq!(receipt["packets"][1]["frames"], 0);
                    assert_eq!(receipt["packets"][1]["qpcHns"], 0);
                    assert_eq!(
                        receipt["packetDriverSourceSha256"],
                        format!(
                            "{:x}",
                            Sha256::digest(include_bytes!("application_packet_driver.rs"))
                        )
                    );
                }
            }
            let restored = take_request().unwrap();
            assert_eq!(
                restored.acquisition_strategy(),
                AcquisitionStrategy::SizeHintGated
            );
            assert_eq!(restored.root.as_deref(), Some(Path::new("outer-size-hint")));
        }
        assert!(take_request().is_none());
    }

    #[test]
    fn original_payload_bounds_include_silent_frames_and_empty_packet_floods() {
        for (max_frames, max_packets, frames, flags) in [(2, 3, 2, 0), (2, 3, 2, 2), (20, 1, 0, 0)]
        {
            let mut trace = NativePayloadDiagnostic::new(max_frames, max_packets);
            let bytes = [1u8, 2, 3, 4, 5, 6, 7, 8];
            unsafe {
                trace.record_original(bytes.as_ptr(), 1, 0, 10, 100, Duration::ZERO);
                trace.record_original(bytes.as_ptr(), frames, flags, 11, 200, Duration::ZERO);
                trace.record_original(bytes.as_ptr(), 1, 0, 12, 300, Duration::ZERO);
            }
            assert!(trace.original_invalid_reason.is_some());
            assert_eq!(trace.original_bytes, [1, 2, 3, 4]);
            assert_eq!(trace.original_packets.len(), 1);
            assert_eq!(trace.original_retained_frames, 1);
            assert_eq!(trace.original_observed_packets, 3);
            assert!(trace.original_bytes.capacity() <= max_frames * 4);
            assert!(trace.original_packets.capacity() <= max_packets);
            assert!(
                trace.invalid_reason.is_none(),
                "raw overrun must not invalidate decoded collection"
            );
        }
    }

    #[test]
    fn original_payload_offsets_hash_and_silent_semantics_remain_independent() {
        let root = tempfile::tempdir().unwrap();
        let mut trace = NativePayloadDiagnostic::new(8, 4);
        trace.root = Some(root.path().to_path_buf());
        let a = [0, 128, 255, 127, 255, 255, 1, 0];
        let b = [0, 1, 0, 2];
        unsafe {
            trace.record_original(a.as_ptr(), 2, 1, 10, 100, Duration::from_nanos(7));
            trace.record_original(std::ptr::dangling(), 2, 2, 12, 200, Duration::from_nanos(8));
            trace.record_original(std::ptr::dangling(), 0, 4, 14, 300, Duration::from_nanos(9));
            trace.record_original(b.as_ptr(), 1, 0, 14, 400, Duration::from_nanos(10));
        }
        assert_eq!(trace.original_bytes, [a.to_vec(), b.to_vec()].concat());
        assert_eq!(
            trace
                .original_packets
                .iter()
                .map(|p| (p.offset_bytes, p.byte_length, p.kind))
                .collect::<Vec<_>>(),
            [
                (0, 8, "payload"),
                (8, 0, "silent"),
                (8, 0, "empty"),
                (8, 4, "payload")
            ]
        );
        trace.stopped(&Err("native stop failed".into()));
        trace
            .flush_after_stop(77, &Err("release failed".into()), &Ok(()))
            .unwrap();
        let bytes = std::fs::read(root.path().join("original-native-stereo.pcm")).unwrap();
        let report: serde_json::Value = serde_json::from_slice(
            &std::fs::read(root.path().join("original-native-stereo.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(report["pcmSha256"], format!("{:x}", Sha256::digest(&bytes)));
        assert_eq!(report["pcmByteLength"], 12);
        assert_eq!(report["observedFrames"], 5);
        assert_eq!(report["retainedFrames"], 5);
        assert_eq!(
            report["nativeStopResult"],
            serde_json::json!({"Err":"native stop failed"})
        );
        assert_eq!(
            report["captureResult"],
            serde_json::json!({"Err":"release failed"})
        );
        assert_eq!(
            report["diagnosticValid"], true,
            "collector completeness is not a native success result"
        );
    }

    #[test]
    fn native_payload_diagnostic_request_is_single_use_scoped_and_thread_local() {
        assert!(take_request().is_none());
        {
            let _guard = request(PathBuf::from("outer"));
            {
                let _inner = request(PathBuf::from("inner"));
                assert_eq!(
                    take_request().unwrap().root.unwrap(),
                    PathBuf::from("inner")
                );
                assert!(take_request().is_none());
                std::thread::spawn(|| assert!(take_request().is_none()))
                    .join()
                    .unwrap();
            }
            assert_eq!(
                take_request().unwrap().root.unwrap(),
                PathBuf::from("outer")
            );
        }
        assert!(take_request().is_none());
    }

    #[test]
    fn native_payload_diagnostic_post_stop_files_reconcile_exact_pcm_hash_and_metadata() {
        let root = tempfile::tempdir().unwrap();
        let mut trace = NativePayloadDiagnostic::new(4, 2);
        trace.root = Some(root.path().to_path_buf());
        trace.record(
            &[i16::MIN, -1, 0, 32767],
            7,
            4,
            123,
            Duration::from_nanos(8),
        );
        trace.record(&[], 11, 2, 456, Duration::from_nanos(9));
        assert!(!root.path().join("decoded-native-mono.pcm").exists());
        trace.stopped(&Ok(()));
        trace
            .flush_after_stop(42, &Err("clock failed after native decode".into()), &Ok(()))
            .unwrap();
        let pcm = std::fs::read(root.path().join("decoded-native-mono.pcm")).unwrap();
        assert_eq!(pcm, [0, 128, 255, 255, 0, 0, 255, 127]);
        let report: serde_json::Value = serde_json::from_slice(
            &std::fs::read(root.path().join("decoded-native-mono.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(
            report["pcmSha256"],
            "a0b3d8f5dbcdc6dac337b3de2e92fe62e1391e6a031d577ca3f7b4b804ac9c79"
        );
        assert_eq!(report["pcmByteLength"], 8);
        assert_eq!(report["retainedFrames"], 4);
        assert_eq!(report["diagnosticValid"], true);
        assert_eq!(report["packets"][1]["offsetFrames"], 4);
        assert_eq!(report["packets"][1]["frames"], 0);
        assert_eq!(report["nativeStopResult"], serde_json::json!({"Ok": null}));
        assert_eq!(
            report["captureResult"],
            serde_json::json!({"Err": "clock failed after native decode"})
        );
    }

    // Catches sample mutation/loss, metadata reassociation and dropping empty reads.
    #[test]
    fn native_payload_diagnostic_retains_exact_decoded_mono_and_all_packet_receipts() {
        let mut trace = NativePayloadDiagnostic::new(6, 3);
        let source = [i16::MIN, -1, 0, 32767];
        trace.record(&source, 700, 1, 9_000_000, Duration::from_nanos(123));
        trace.record(&[], 704, 4, 9_000_834, Duration::from_nanos(124));
        trace.record(&[0, 0], 704, 2, 9_000_834, Duration::from_nanos(125));
        assert_eq!(trace.samples, [i16::MIN, -1, 0, 32767, 0, 0]);
        assert_eq!(
            trace.packets,
            [
                PacketReceipt {
                    offset_frames: 0,
                    frames: 4,
                    flags: 1,
                    device_index: 700,
                    qpc_hns: 9_000_000,
                    observed_elapsed_ns: 123
                },
                PacketReceipt {
                    offset_frames: 4,
                    frames: 0,
                    flags: 4,
                    device_index: 704,
                    qpc_hns: 9_000_834,
                    observed_elapsed_ns: 124
                },
                PacketReceipt {
                    offset_frames: 4,
                    frames: 2,
                    flags: 2,
                    device_index: 704,
                    qpc_hns: 9_000_834,
                    observed_elapsed_ns: 125
                },
            ]
        );
        assert_eq!(trace.invalid_reason, None);
    }

    // Catches silent truncation and post-overflow growth, including empty-read floods.
    #[test]
    fn native_payload_diagnostic_overrun_invalidates_without_partial_packet_or_growth() {
        for (max_frames, max_packets, overflow) in [(2, 4, vec![8, 9]), (20, 1, vec![])] {
            let mut trace = NativePayloadDiagnostic::new(max_frames, max_packets);
            trace.record(&[7], 1, 0, 100, Duration::ZERO);
            trace.record(&overflow, 2, 4, 200, Duration::ZERO);
            assert!(
                trace.invalid_reason.is_some(),
                "overrun must invalidate all reconciliation claims"
            );
            trace.record(&[10], 3, 0, 300, Duration::ZERO);
            assert_eq!(trace.samples, [7]);
            assert_eq!(trace.packets.len(), 1);
            assert!(trace.samples.capacity() <= max_frames);
            assert!(trace.packets.capacity() <= max_packets);
        }
    }
}
