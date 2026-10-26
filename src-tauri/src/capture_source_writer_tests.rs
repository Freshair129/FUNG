use super::*;
use crate::{capture_source_custody as custody, genesis_adapter as ga};
use genesis_block_native::*;
use serde_json::{json, Value};
use std::fs;
struct Fixture {
    _temp: tempfile::TempDir,
    s: Arc<Storage>,
    project: String,
    recording: String,
    regs: Vec<CaptureSourceRegistration>,
}
fn fixture(count: usize) -> Fixture {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("project");
    fs::create_dir(&root).unwrap();
    let s = Arc::new(
        Storage::open(OpenOptions {
            path: temp.path().join("db").display().to_string(),
            page_cache_mb: Some(16),
            read_only: Some(false),
            vector_dim: Some(4),
            retention: None,
        })
        .unwrap(),
    );
    ga::install(&s).unwrap();
    let project = uuid::Uuid::new_v4().to_string();
    let recording = uuid::Uuid::new_v4().to_string();
    ga::commit_rows(&s,vec![ga::upsert("projects",json!({"id":project,"name":"test","storage_path":root,"created_at":"t","updated_at":"t"})),ga::upsert("recordings",json!({"id":recording,"project_id":project,"source":"microphone","canonical_audio_path":"unused","status":"recording","duration_ms":0,"created_at":"t","updated_at":"t"})),ga::upsert("mobile_recording_checkpoints",json!({"id":recording,"recording_id":recording,"safe_offset_ms":0,"segment_count":0,"last_checksum":null,"updated_at":"t"}))]).unwrap();
    let regs = (0..count)
        .map(|i| CaptureSourceRegistration {
            id: uuid::Uuid::new_v4().to_string(),
            epoch: 1,
            kind: "endpoint".into(),
            endpoint_id: Some(format!("endpoint-{i}")),
            label: format!("app-{i}"),
            sample_rate: 48000,
            clock_origin_qpc_hns: "1234".into(),
            metadata: json!({"policyVersion":1}),
        })
        .collect::<Vec<_>>();
    custody::register_capture_sources(&s, &attempt(&s), &project, &recording, &regs).unwrap();
    Fixture {
        _temp: temp,
        s,
        project,
        recording,
        regs,
    }
}
fn attempt(s: &Storage) -> custody::SourceCommitAttempt {
    custody::begin_source_commit(s, &uuid::Uuid::new_v4().to_string(), "2026-10-26T00:00:00Z")
        .unwrap()
}
fn group(f: &Fixture) -> CaptureSourceWriterGroup {
    CaptureSourceWriterGroup::new(f.s.clone(), f.project.clone(), f.recording.clone()).unwrap()
}
fn packet(r: &CaptureSourceRegistration, first: i64, frames: usize) -> SourcePcmPacket {
    SourcePcmPacket {
        source_id: r.id.clone(),
        epoch: r.epoch,
        first_frame: first,
        pcm: (first..first + frames as i64)
            .map(|x| (x % 2000) as i16 - 1000)
            .collect(),
    }
}
fn audio(f: &Fixture) -> Vec<Value> {
    let mut r = ga::query(
        &f.s,
        "audio_chunks",
        &["id", "file_path", "start_ms", "end_ms", "sequence_no"],
        vec![],
        1000,
    )
    .unwrap();
    r.sort_by_key(|x| x["audio_chunks.sequence_no"].as_i64().unwrap());
    r
}
fn samples(path: &Value) -> Vec<i16> {
    hound::WavReader::open(path.as_str().unwrap())
        .unwrap()
        .samples::<i16>()
        .map(Result::unwrap)
        .collect()
}
#[test]
fn exact_two_second_chunk_and_one_frame_tail_bind_real_pcm() {
    let f = fixture(1);
    let mut g = group(&f);
    let mut w = g
        .spawn(SourceWriterOptions::new(f.regs[0].clone()))
        .unwrap();
    let a = packet(&f.regs[0], 0, 96000);
    let b = packet(&f.regs[0], 96000, 1);
    let expected = [a.pcm.clone(), b.pcm.clone()].concat();
    w.try_push(a).unwrap();
    w.try_push(b).unwrap();
    let report = w.finish();
    assert!(report.first_failure.is_none(), "{report:?}");
    assert_eq!(report.durable_chunks, 2);
    assert_eq!(report.durable_frames, 96001);
    assert!(report.drained);
    let rows = audio(&f);
    assert_eq!(rows.len(), 2);
    assert_eq!(
        rows.iter()
            .flat_map(|v| samples(&v["audio_chunks.file_path"]))
            .collect::<Vec<_>>(),
        expected
    );
    assert!(
        custody::require_capture_source_access(&f.s, &f.recording).is_err(),
        "writer must not close all sources itself"
    );
    custody::complete_capture_sources(&f.s, &attempt(&f.s), &f.recording).unwrap();
    custody::require_capture_source_access(&f.s, &f.recording).unwrap();
}
#[test]
fn interleaved_sources_have_independent_writers_and_common_max_duration() {
    let f = fixture(2);
    let mut g = group(&f);
    let mut a = g
        .spawn(SourceWriterOptions::new(f.regs[0].clone()))
        .unwrap();
    let mut b = g
        .spawn(SourceWriterOptions::new(f.regs[1].clone()))
        .unwrap();
    a.try_push(packet(&f.regs[0], 0, 96000)).unwrap();
    b.try_push(packet(&f.regs[1], 0, 48001)).unwrap();
    a.try_push(packet(&f.regs[0], 96000, 1)).unwrap();
    assert!(a.finish().first_failure.is_none());
    assert!(b.finish().first_failure.is_none());
    custody::complete_capture_sources(&f.s, &attempt(&f.s), &f.recording).unwrap();
    custody::require_capture_source_access(&f.s, &f.recording).unwrap();
    let rows = ga::query(
        &f.s,
        "capture_source_chunks",
        &["source_id", "chunk_json"],
        vec![],
        10,
    )
    .unwrap();
    assert_eq!(rows.len(), 3);
    let rec = ga::query(&f.s, "recordings", &["duration_ms"], vec![], 1).unwrap();
    assert_eq!(rec[0]["recordings.duration_ms"], 2000);
}
#[test]
fn gap_quarantines_before_private_pcm_and_never_invents_silence() {
    let f = fixture(1);
    let mut g = group(&f);
    let mut w = g
        .spawn(SourceWriterOptions::new(f.regs[0].clone()))
        .unwrap();
    w.try_push(packet(&f.regs[0], 0, 4800)).unwrap();
    assert_eq!(
        w.try_push(packet(&f.regs[0], 9600, 4800)).unwrap_err().kind,
        SourceWriterFailureKind::Gap
    );
    let report = w.finish();
    assert_eq!(
        report.first_failure.unwrap().kind,
        SourceWriterFailureKind::Gap
    );
    assert!(report.quarantine_persisted);
    assert_eq!(report.durable_frames, 9600);
    let rows = audio(&f);
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0]["audio_chunks.end_ms"], 100);
    assert_eq!(rows[1]["audio_chunks.start_ms"], 200);
    assert!(custody::require_capture_source_access(&f.s, &f.recording).is_err());
}
#[test]
fn queue_full_preserves_out_of_band_first_failure_and_drains_bounded_terminal_packet() {
    let f = fixture(1);
    let mut g = group(&f);
    let gate = Arc::new(TestGate::default());
    let mut options = SourceWriterOptions::new(f.regs[0].clone());
    options.queue_capacity = 1;
    options.gate = Some(gate.clone());
    let mut w = g.spawn(options).unwrap();
    w.try_push(packet(&f.regs[0], 0, 4800)).unwrap();
    assert_eq!(
        w.try_push(packet(&f.regs[0], 4800, 4800)).unwrap_err().kind,
        SourceWriterFailureKind::QueueOverflow
    );
    gate.release();
    let report = w.finish();
    assert_eq!(
        report.first_failure.unwrap().kind,
        SourceWriterFailureKind::QueueOverflow
    );
    assert!(report.quarantine_persisted && report.drained);
    assert_eq!(report.durable_frames, 9600);
    assert!(custody::require_capture_source_access(&f.s, &f.recording).is_err());
}
#[test]
fn overlap_never_rewinds_or_binds_duplicate_frames() {
    let f = fixture(1);
    let mut g = group(&f);
    let mut w = g
        .spawn(SourceWriterOptions::new(f.regs[0].clone()))
        .unwrap();
    w.try_push(packet(&f.regs[0], 0, 1000)).unwrap();
    assert_eq!(
        w.try_push(packet(&f.regs[0], 500, 1000)).unwrap_err().kind,
        SourceWriterFailureKind::Overlap
    );
    let r = w.finish();
    assert_eq!(r.durable_frames, 1000);
    assert!(r.quarantine_persisted);
}
#[test]
fn untrusted_startup_offset_is_private_failure_not_fabricated_leading_pcm() {
    let f = fixture(1);
    let mut g = group(&f);
    let mut w = g
        .spawn(SourceWriterOptions::new(f.regs[0].clone()))
        .unwrap();
    assert_eq!(
        w.try_push(packet(&f.regs[0], 48, 48)).unwrap_err().kind,
        SourceWriterFailureKind::Gap
    );
    let r = w.finish();
    assert!(r.quarantine_persisted);
    let rows = audio(&f);
    assert_eq!(rows[0]["audio_chunks.start_ms"], 1);
    assert_eq!(samples(&rows[0]["audio_chunks.file_path"]).len(), 48);
}
#[test]
fn custody_failure_retains_private_orphans_without_successful_audio_publication() {
    let f = fixture(1);
    let mut g = group(&f);
    let gate = Arc::new(TestGate::default());
    let mut options = SourceWriterOptions::new(f.regs[0].clone());
    options.gate = Some(gate.clone());
    let mut w = g.spawn(options).unwrap();
    w.try_push(packet(&f.regs[0], 0, 96000)).unwrap();
    w.try_push(packet(&f.regs[0], 96000, 96000)).unwrap();
    let mut rec = ga::query(
        &f.s,
        "recordings",
        &[
            "id",
            "project_id",
            "source",
            "canonical_audio_path",
            "status",
            "duration_ms",
            "created_at",
            "updated_at",
        ],
        vec![],
        1,
    )
    .unwrap()
    .remove(0);
    rec = Value::Object(
        rec.as_object()
            .unwrap()
            .iter()
            .map(|(k, v)| (k.strip_prefix("recordings.").unwrap().into(), v.clone()))
            .collect(),
    );
    rec["status"] = json!("completed");
    ga::commit_rows(&f.s, vec![ga::upsert("recordings", rec)]).unwrap();
    gate.release();
    let r = w.finish();
    assert_eq!(
        r.first_failure.unwrap().kind,
        SourceWriterFailureKind::Persistence
    );
    assert_eq!(r.durable_chunks, 0);
    assert!(
        !r.durable_counts_known,
        "opaque failed commit/replay cannot claim globally certain totals"
    );
    assert!(audio(&f).is_empty());
    assert_eq!(r.private_orphans.len(), 2);
    for p in r.private_orphans {
        assert!(p.is_file());
    }
}
#[test]
fn wrong_identity_is_not_written_and_empty_finish_is_explicit_failure() {
    let f = fixture(1);
    let mut g = group(&f);
    let mut w = g
        .spawn(SourceWriterOptions::new(f.regs[0].clone()))
        .unwrap();
    let mut p = packet(&f.regs[0], 0, 100);
    p.source_id = uuid::Uuid::new_v4().to_string();
    assert_eq!(
        w.try_push(p).unwrap_err().kind,
        SourceWriterFailureKind::WrongIdentity
    );
    let r = w.finish();
    assert_eq!(r.durable_chunks, 0);
    assert!(r.quarantine_persisted);
    assert!(audio(&f).is_empty());
    let f = fixture(1);
    let mut g = group(&f);
    let r = g
        .spawn(SourceWriterOptions::new(f.regs[0].clone()))
        .unwrap()
        .finish();
    assert_eq!(
        r.first_failure.unwrap().kind,
        SourceWriterFailureKind::EmptySource
    );
    assert!(r.quarantine_persisted);
}

#[test]
fn native_fault_uses_out_of_band_signal_without_fake_pcm_and_retains_first_cause() {
    let f = fixture(1);
    let mut g = group(&f);
    let gate = Arc::new(TestGate::default());
    let mut options = SourceWriterOptions::new(f.regs[0].clone());
    options.gate = Some(gate.clone());
    let mut w = g.spawn(options).unwrap();
    w.try_push(packet(&f.regs[0], 0, 1000)).unwrap();
    w.signal_failure("unknown native session");
    w.signal_failure("later device removal");
    gate.release();
    let report = w.finish();
    let failure = report
        .first_failure
        .expect("native fault must never turn into healthy completion");
    assert_eq!(failure.kind, SourceWriterFailureKind::NativeSourceFailure);
    assert_eq!(failure.detail, "unknown native session");
    assert!(report.quarantine_persisted);
    assert_eq!(report.durable_frames, 1000);
    assert!(custody::require_capture_source_access(&f.s, &f.recording).is_err());
}

#[test]
fn long_multibyte_native_failure_remains_bounded_and_durably_quarantines() {
    let f = fixture(1);
    let mut g = group(&f);
    let mut w = g
        .spawn(SourceWriterOptions::new(f.regs[0].clone()))
        .unwrap();
    w.try_push(packet(&f.regs[0], 0, 100)).unwrap();
    w.signal_failure(&"แ".repeat(2000));
    let r = w.finish();
    assert!(r.quarantine_persisted, "{r:?}");
    assert!(r.first_failure.unwrap().detail.len() <= 1024);
}

#[test]
fn oversized_vector_allocation_cannot_bypass_copied_pcm_queue_budget() {
    let f = fixture(1);
    let mut g = group(&f);
    let mut w = g
        .spawn(SourceWriterOptions::new(f.regs[0].clone()))
        .unwrap();
    let mut p = packet(&f.regs[0], 0, 1);
    p.pcm.reserve(1_000_000);
    assert_eq!(
        w.try_push(p).unwrap_err().kind,
        SourceWriterFailureKind::PacketTooLarge
    );
    let report = w.finish();
    assert_eq!(report.durable_frames, 0);
    assert!(report.quarantine_persisted);
}

struct ExpectedOwnership {
    packet: SourcePcmPacket,
    ptr: usize,
    capacity: usize,
}
fn expect_ownership(p: &SourcePcmPacket) -> ExpectedOwnership {
    ExpectedOwnership {
        packet: p.clone(),
        ptr: p.pcm.as_ptr() as usize,
        capacity: p.pcm.capacity(),
    }
}
fn returned(rejection: SourcePushRejection, expected: ExpectedOwnership) -> SourceWriterFailure {
    let SourcePushRejection {
        failure,
        disposition,
    } = rejection;
    match disposition {
        SourcePacketDisposition::Returned(packet) => {
            assert_eq!(packet.source_id, expected.packet.source_id);
            assert_eq!(packet.epoch, expected.packet.epoch);
            assert_eq!(packet.first_frame, expected.packet.first_frame);
            assert_eq!(packet.pcm, expected.packet.pcm);
            assert_eq!(
                packet.pcm.as_ptr() as usize,
                expected.ptr,
                "must return original allocation, not reconstruct a copy"
            );
            assert_eq!(packet.pcm.capacity(), expected.capacity);
        }
        other => panic!("expected ownership Returned, got {other:?}"),
    };
    failure
}
fn terminal(
    rejection: SourcePushRejection,
    source: &CaptureSourceRegistration,
    first: i64,
    frames: usize,
) -> SourceWriterFailure {
    let SourcePushRejection {
        failure,
        disposition,
    } = rejection;
    match disposition {
        SourcePacketDisposition::RetainedTerminal {
            source_id,
            epoch,
            first_frame,
            frames: retained,
        } => {
            assert_eq!(source_id, source.id);
            assert_eq!(epoch, source.epoch);
            assert_eq!(first_frame, first);
            assert_eq!(retained, frames)
        }
        other => panic!("expected RetainedTerminal, got {other:?}"),
    };
    failure
}
fn same_failure(a: &SourceWriterFailure, b: &SourceWriterFailure) {
    assert_eq!(a.kind, b.kind);
    assert_eq!(a.detail, b.detail);
    assert_eq!(a.expected_frame, b.expected_frame);
    assert_eq!(a.observed_frame, b.observed_frame);
    assert_eq!(a.packet_frames, b.packet_frames);
}
#[test]
fn recoverable_after_poison_returns_original_packet_and_immutable_first_fault() {
    let f = fixture(1);
    let mut g = group(&f);
    let mut w = g
        .spawn(SourceWriterOptions::new(f.regs[0].clone()))
        .unwrap();
    w.signal_failure("native source removed");
    let p = packet(&f.regs[0], 0, 101);
    let expected = expect_ownership(&p);
    let first = returned(*w.try_push_recoverable(p).unwrap_err(), expected);
    assert_eq!(first.kind, SourceWriterFailureKind::NativeSourceFailure);
    assert_eq!(first.detail, "native source removed");
    let mut p = packet(&f.regs[0], 1000, 19);
    p.source_id = "WRONG_ID".into();
    let expected = expect_ownership(&p);
    let later = returned(*w.try_push_recoverable(p).unwrap_err(), expected);
    same_failure(&first, &later);
    let report = w.finish();
    same_failure(&first, &report.first_failure.unwrap());
    assert_eq!(report.durable_frames, 0);
    assert!(report.quarantine_persisted);
    assert!(audio(&f).is_empty());
}
#[test]
fn recoverable_first_gap_retains_only_terminal_then_returns_later_same_kind_packet() {
    let f = fixture(1);
    let mut g = group(&f);
    let mut w = g
        .spawn(SourceWriterOptions::new(f.regs[0].clone()))
        .unwrap();
    let a = packet(&f.regs[0], 0, 1000);
    let b = packet(&f.regs[0], 2000, 10);
    let expected = [a.pcm.clone(), b.pcm.clone()].concat();
    w.try_push_recoverable(a).unwrap();
    let first = terminal(
        *w.try_push_recoverable(b).unwrap_err(),
        &f.regs[0],
        2000,
        10,
    );
    assert_eq!(first.kind, SourceWriterFailureKind::Gap);
    let c = packet(&f.regs[0], 2010, 20);
    let ownership = expect_ownership(&c);
    let later = returned(*w.try_push_recoverable(c).unwrap_err(), ownership);
    same_failure(&first, &later);
    let report = w.finish();
    assert_eq!(report.durable_frames, 1010);
    assert!(report.quarantine_persisted);
    let actual = audio(&f)
        .iter()
        .flat_map(|v| samples(&v["audio_chunks.file_path"]))
        .collect::<Vec<_>>();
    assert_eq!(
        actual, expected,
        "retained terminal exactly once; returned later packet never bound"
    );
}
#[test]
fn recoverable_first_full_retains_exactly_once_and_later_full_cause_returns_new_ownership() {
    let f = fixture(1);
    let mut g = group(&f);
    let gate = Arc::new(TestGate::default());
    let mut options = SourceWriterOptions::new(f.regs[0].clone());
    options.queue_capacity = 1;
    options.gate = Some(gate.clone());
    let mut w = g.spawn(options).unwrap();
    let a = packet(&f.regs[0], 0, 100);
    let b = packet(&f.regs[0], 100, 120);
    let expected = [a.pcm.clone(), b.pcm.clone()].concat();
    w.try_push_recoverable(a).unwrap();
    let first = terminal(
        *w.try_push_recoverable(b).unwrap_err(),
        &f.regs[0],
        100,
        120,
    );
    assert_eq!(first.kind, SourceWriterFailureKind::QueueOverflow);
    let c = packet(&f.regs[0], 220, 140);
    let ownership = expect_ownership(&c);
    let later = returned(*w.try_push_recoverable(c).unwrap_err(), ownership);
    same_failure(&first, &later);
    gate.release();
    let report = w.finish();
    assert_eq!(report.durable_frames, 220);
    assert!(report.quarantine_persisted);
    let actual = audio(&f)
        .iter()
        .flat_map(|v| samples(&v["audio_chunks.file_path"]))
        .collect::<Vec<_>>();
    assert_eq!(actual, expected);
}
#[test]
fn recoverable_receiver_disconnected_returns_original_pcm_not_false_terminal_receipt() {
    let f = fixture(1);
    let mut g = group(&f);
    let gate = Arc::new(TestGate::default());
    let mut options = SourceWriterOptions::new(f.regs[0].clone());
    options.gate = Some(gate.clone());
    let mut w = g.spawn(options).unwrap();
    let (tx, rx) = std::sync::mpsc::sync_channel(1);
    drop(rx);
    drop(w.tx.replace(tx));
    let p = packet(&f.regs[0], 0, 321);
    let ownership = expect_ownership(&p);
    let failure = returned(*w.try_push_recoverable(p).unwrap_err(), ownership);
    assert_eq!(failure.kind, SourceWriterFailureKind::Persistence);
    assert_eq!(failure.detail, "source writer disconnected");
    gate.release();
    let report = w.finish();
    assert_eq!(report.durable_frames, 0);
    assert!(report.quarantine_persisted);
    assert!(audio(&f).is_empty());
}
#[test]
fn recoverable_invalid_returns_are_ownership_only_never_custody_permission() {
    let f = fixture(1);
    let mut g = group(&f);
    let mut w = g
        .spawn(SourceWriterOptions::new(f.regs[0].clone()))
        .unwrap();
    let mut p = packet(&f.regs[0], 0, 17);
    p.source_id = uuid::Uuid::new_v4().to_string();
    let expected = expect_ownership(&p);
    let failure = returned(*w.try_push_recoverable(p).unwrap_err(), expected);
    assert_eq!(failure.kind, SourceWriterFailureKind::WrongIdentity);
    assert_eq!(w.finish().durable_frames, 0);
    assert!(audio(&f).is_empty());
    let f = fixture(1);
    let mut g = group(&f);
    let mut w = g
        .spawn(SourceWriterOptions::new(f.regs[0].clone()))
        .unwrap();
    let mut p = packet(&f.regs[0], 0, 1);
    p.pcm.reserve(1_000_000);
    let expected = expect_ownership(&p);
    let failure = returned(*w.try_push_recoverable(p).unwrap_err(), expected);
    assert_eq!(failure.kind, SourceWriterFailureKind::PacketTooLarge);
    assert_eq!(w.finish().durable_frames, 0);
    assert!(audio(&f).is_empty());
}
#[test]
fn recoverable_overlap_returns_original_without_rewinding_valid_accepted_pcm() {
    let f = fixture(1);
    let mut g = group(&f);
    let mut w = g
        .spawn(SourceWriterOptions::new(f.regs[0].clone()))
        .unwrap();
    let a = packet(&f.regs[0], 0, 100);
    let expected = a.pcm.clone();
    w.try_push_recoverable(a).unwrap();
    let p = packet(&f.regs[0], 50, 100);
    let ownership = expect_ownership(&p);
    let failure = returned(*w.try_push_recoverable(p).unwrap_err(), ownership);
    assert_eq!(failure.kind, SourceWriterFailureKind::Overlap);
    let report = w.finish();
    assert_eq!(report.durable_frames, 100);
    assert_eq!(
        audio(&f)
            .iter()
            .flat_map(|v| samples(&v["audio_chunks.file_path"]))
            .collect::<Vec<_>>(),
        expected
    );
}
