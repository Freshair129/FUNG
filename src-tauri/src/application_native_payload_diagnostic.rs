//! Test-only bounded custody of decoded native mono, before clock/queue/writer.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::Serialize;
use sha2::{Digest, Sha256};

thread_local! {
    static REQUEST: RefCell<Option<PathBuf>> = const { RefCell::new(None) };
}

pub(super) struct ProbeRequestGuard(Option<PathBuf>);

impl Drop for ProbeRequestGuard {
    fn drop(&mut self) {
        REQUEST.with(|request| *request.borrow_mut() = self.0.take());
    }
}

/// Only the explicitly ignored owned probe arms this caller-local, single-use request.
pub(super) fn request(root: PathBuf) -> ProbeRequestGuard {
    ProbeRequestGuard(REQUEST.with(|request| request.replace(Some(root))))
}

pub(super) fn take_request() -> Option<NativePayloadDiagnostic> {
    REQUEST
        .with(|request| request.borrow_mut().take())
        .map(|root| {
            let mut trace = NativePayloadDiagnostic::new(24 * 48_000, 8192);
            trace.root = Some(root);
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
            "scope": "test-only decoded native mono custody; no raw stereo byte, installed audio acceptance or PID repair claim"
        });
        write_new(root, "decoded-native-mono.pcm", &pcm)?;
        write_new(
            root,
            "decoded-native-mono.json",
            &serde_json::to_vec_pretty(&report).map_err(|error| error.to_string())?,
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
