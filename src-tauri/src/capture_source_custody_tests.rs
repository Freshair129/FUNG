use super::*;
use crate::genesis_adapter as ga;
use sha2::{Digest, Sha256};
use std::fs;
fn begin_source_commit(s: &Storage) -> SourceCommitAttempt {
    super::begin_source_commit(s, &uuid::Uuid::new_v4().to_string(), "2026-10-26T00:00:00Z")
        .unwrap()
}
struct Fixture {
    _temp: tempfile::TempDir,
    s: Storage,
    project: String,
    recording: String,
    root: PathBuf,
    reg: CaptureSourceRegistration,
}
fn fixture() -> Fixture {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("project");
    fs::create_dir(&root).unwrap();
    let s = Storage::open(OpenOptions {
        path: tmp.path().join("db").display().to_string(),
        page_cache_mb: Some(16),
        read_only: Some(false),
        vector_dim: Some(4),
        retention: None,
    })
    .unwrap();
    ga::install(&s).unwrap();
    let project = uuid::Uuid::new_v4().to_string();
    let recording = uuid::Uuid::new_v4().to_string();
    ga::commit_rows(&s,vec![ga::upsert("projects",json!({"id":project,"name":"test","storage_path":root,"created_at":"t","updated_at":"t"})),ga::upsert("recordings",json!({"id":recording,"project_id":project,"source":"microphone","canonical_audio_path":"unused","status":"recording","duration_ms":0,"created_at":"t","updated_at":"t"})),ga::upsert("mobile_recording_checkpoints",json!({"id":recording,"recording_id":recording,"safe_offset_ms":0,"segment_count":0,"last_checksum":null,"updated_at":"t"}))]).unwrap();
    Fixture {
        _temp: tmp,
        s,
        project,
        recording,
        root,
        reg: CaptureSourceRegistration {
            id: uuid::Uuid::new_v4().to_string(),
            epoch: 1,
            kind: "endpoint".into(),
            endpoint_id: Some("native-stable-id".into()),
            label: "test app".into(),
            sample_rate: 48000,
            clock_origin_qpc_hns: "1234".into(),
            metadata: json!({"policyVersion":1}),
        },
    }
}
fn reg(f: &Fixture) {
    register_capture_sources(
        &f.s,
        &begin_source_commit(&f.s),
        &f.project,
        &f.recording,
        std::slice::from_ref(&f.reg),
    )
    .unwrap();
}
fn chunk(
    f: &Fixture,
    r: &CaptureSourceRegistration,
    seq: i64,
    start: i64,
    end: i64,
) -> CaptureSourceChunk {
    let dir = f
        .root
        .join("live")
        .join(&f.recording)
        .join("sources")
        .join(&r.id)
        .join(format!("epoch-{}", r.epoch));
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join(format!("chunk-{seq}.wav"));
    let mut w = hound::WavWriter::create(
        &path,
        hound::WavSpec {
            channels: 1,
            sample_rate: 48000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        },
    )
    .unwrap();
    for i in start..end {
        w.write_sample((i % 1000) as i16).unwrap();
    }
    w.finalize().unwrap();
    CaptureSourceChunk {
        id: uuid::Uuid::new_v4().to_string(),
        source_id: r.id.clone(),
        epoch: r.epoch,
        sequence: seq,
        path: path.clone(),
        start_frame: start,
        end_frame: end,
        start_ms: start / 48,
        end_ms: end / 48,
        checksum: format!("{:x}", Sha256::digest(fs::read(path).unwrap())),
    }
}
fn row(s: &Storage, t: &str, id: &str) -> Value {
    ga::query(
        s,
        t,
        &["id"],
        vec![RelationalFilter::equal(&format!("{t}.id"), json!(id))],
        1,
    )
    .unwrap()
    .into_iter()
    .next()
    .map(|v| {
        Value::Object(
            v.as_object()
                .unwrap()
                .iter()
                .map(|(k, v)| {
                    (
                        k.strip_prefix(&format!("{t}.")).unwrap().to_owned(),
                        v.clone(),
                    )
                })
                .collect(),
        )
    })
    .unwrap()
}
#[test]
fn registers_additive_schema_preserves_legacy_recording() {
    let f = fixture();
    reg(&f);
    assert_eq!(row(&f.s, "capture_sources", &f.reg.id)["id"], f.reg.id);
    assert!(require_capture_source_access(&f.s, &f.recording).is_err());
}
#[test]
fn append_atomic_source_binding_checkpoint_retry_and_conflict() {
    let f = fixture();
    reg(&f);
    let c = chunk(&f, &f.reg, 1, 0, 4800);
    let a = begin_source_commit(&f.s);
    assert!(!append_capture_source_chunk(&f.s, &a, &f.project, &f.recording, &c).unwrap());
    assert!(append_capture_source_chunk(&f.s, &a, &f.project, &f.recording, &c).unwrap());
    assert_eq!(row(&f.s, "audio_chunks", &c.id)["id"], c.id);
    assert_eq!(row(&f.s, "capture_source_chunks", &c.id)["id"], c.id);
    let mut changed = c.clone();
    changed.end_ms += 1;
    assert!(append_capture_source_chunk(
        &f.s,
        &begin_source_commit(&f.s),
        &f.project,
        &f.recording,
        &changed
    )
    .is_err());
}
#[test]
fn stale_frontier_cannot_partially_append() {
    let f = fixture();
    reg(&f);
    let c = chunk(&f, &f.reg, 1, 0, 4800);
    let stale = begin_source_commit(&f.s);
    ga::commit_rows(&f.s,vec![ga::upsert("projects",json!({"id":f.project,"name":"changed","storage_path":f.root,"created_at":"t","updated_at":"t"}))]).unwrap();
    assert!(append_capture_source_chunk(&f.s, &stale, &f.project, &f.recording, &c).is_err());
    assert!(ga::query(&f.s, "audio_chunks", &["id"], vec![], 10)
        .unwrap()
        .is_empty());
    assert!(
        ga::query(&f.s, "capture_source_chunks", &["id"], vec![], 10)
            .unwrap()
            .is_empty()
    );
}
#[test]
fn interleaved_sources_keep_global_max_duration() {
    let f = fixture();
    let mut r = f.reg.clone();
    r.id = uuid::Uuid::new_v4().to_string();
    r.endpoint_id = Some("second".into());
    register_capture_sources(
        &f.s,
        &begin_source_commit(&f.s),
        &f.project,
        &f.recording,
        &[f.reg.clone(), r.clone()],
    )
    .unwrap();
    append_capture_source_chunk(
        &f.s,
        &begin_source_commit(&f.s),
        &f.project,
        &f.recording,
        &chunk(&f, &f.reg, 1, 0, 9600),
    )
    .unwrap();
    append_capture_source_chunk(
        &f.s,
        &begin_source_commit(&f.s),
        &f.project,
        &f.recording,
        &chunk(&f, &r, 1, 0, 4800),
    )
    .unwrap();
    let rows = ga::query(&f.s, "recordings", &["duration_ms"], vec![], 2).unwrap();
    assert_eq!(rows[0]["recordings.duration_ms"], 200);
    let cp = ga::query(
        &f.s,
        "mobile_recording_checkpoints",
        &["safe_offset_ms", "segment_count"],
        vec![],
        2,
    )
    .unwrap();
    assert_eq!(cp[0]["mobile_recording_checkpoints.safe_offset_ms"], 200);
    assert_eq!(cp[0]["mobile_recording_checkpoints.segment_count"], 2);
}
#[test]
fn mismatched_source_clock_origins_cannot_register_a_shared_timeline() {
    let f = fixture();
    let mut other = f.reg.clone();
    other.id = uuid::Uuid::new_v4().to_string();
    other.endpoint_id = Some("second".into());
    other.clock_origin_qpc_hns = "2345".into();
    assert!(register_capture_sources(
        &f.s,
        &begin_source_commit(&f.s),
        &f.project,
        &f.recording,
        &[f.reg.clone(), other],
    )
    .is_err());
    assert!(ga::query(&f.s, "capture_sources", &["id"], vec![], 10)
        .unwrap()
        .is_empty());
}
#[test]
fn cross_scope_and_unquarantined_gap_reject_before_audio_commit() {
    let f = fixture();
    reg(&f);
    let c = chunk(&f, &f.reg, 1, 480, 4800);
    assert!(append_capture_source_chunk(
        &f.s,
        &begin_source_commit(&f.s),
        &f.project,
        &f.recording,
        &c
    )
    .is_err());
    let c = chunk(&f, &f.reg, 1, 0, 4800);
    assert!(append_capture_source_chunk(
        &f.s,
        &begin_source_commit(&f.s),
        "foreign",
        &f.recording,
        &c
    )
    .is_err());
    assert!(ga::query(&f.s, "audio_chunks", &["id"], vec![], 10)
        .unwrap()
        .is_empty());
}
#[test]
fn quarantine_is_monotonic_even_after_queued_evidence_and_completion() {
    let f = fixture();
    reg(&f);
    quarantine_capture_source(&f.s, &begin_source_commit(&f.s), &f.reg.id, "unknown-owner")
        .unwrap();
    append_capture_source_chunk(
        &f.s,
        &begin_source_commit(&f.s),
        &f.project,
        &f.recording,
        &chunk(&f, &f.reg, 1, 480, 4800),
    )
    .unwrap();
    complete_capture_sources(&f.s, &begin_source_commit(&f.s), &f.recording).unwrap();
    assert!(require_capture_source_access(&f.s, &f.recording).is_err());
    assert!(register_capture_sources(
        &f.s,
        &begin_source_commit(&f.s),
        &f.project,
        &f.recording,
        std::slice::from_ref(&f.reg)
    )
    .is_err());
}
#[test]
fn completion_requires_nonempty_sources_then_allows_trusted_read() {
    let f = fixture();
    reg(&f);
    assert!(complete_capture_sources(&f.s, &begin_source_commit(&f.s), &f.recording).is_err());
    append_capture_source_chunk(
        &f.s,
        &begin_source_commit(&f.s),
        &f.project,
        &f.recording,
        &chunk(&f, &f.reg, 1, 0, 4800),
    )
    .unwrap();
    complete_capture_sources(&f.s, &begin_source_commit(&f.s), &f.recording).unwrap();
    require_capture_source_access(&f.s, &f.recording).unwrap();
}

#[test]
fn quarantined_completion_closes_evidence_ingest() {
    let f = fixture();
    reg(&f);
    quarantine_capture_source(&f.s, &begin_source_commit(&f.s), &f.reg.id, "unknown-owner")
        .unwrap();
    append_capture_source_chunk(
        &f.s,
        &begin_source_commit(&f.s),
        &f.project,
        &f.recording,
        &chunk(&f, &f.reg, 1, 0, 4800),
    )
    .unwrap();
    complete_capture_sources(&f.s, &begin_source_commit(&f.s), &f.recording).unwrap();
    assert!(
        append_capture_source_chunk(
            &f.s,
            &begin_source_commit(&f.s),
            &f.project,
            &f.recording,
            &chunk(&f, &f.reg, 2, 4800, 9600)
        )
        .is_err(),
        "closed quarantined source must reject NEW PCM after worker drain"
    );
}

#[test]
fn exact_single_frame_tail_is_preserved_even_when_ms_projection_is_zero() {
    let f = fixture();
    reg(&f);
    let c = chunk(&f, &f.reg, 1, 0, 1);
    append_capture_source_chunk(
        &f.s,
        &begin_source_commit(&f.s),
        &f.project,
        &f.recording,
        &c,
    )
    .unwrap();
    complete_capture_sources(&f.s, &begin_source_commit(&f.s), &f.recording).unwrap();
    require_capture_source_access(&f.s, &f.recording).unwrap();
}
#[test]
fn rewritten_audio_ledger_is_not_trusted_under_old_binding() {
    let f = fixture();
    reg(&f);
    let c = chunk(&f, &f.reg, 1, 0, 4800);
    append_capture_source_chunk(
        &f.s,
        &begin_source_commit(&f.s),
        &f.project,
        &f.recording,
        &c,
    )
    .unwrap();
    complete_capture_sources(&f.s, &begin_source_commit(&f.s), &f.recording).unwrap();
    let mut audio = super::get(&f.s, "audio_chunks", &c.id).unwrap();
    audio["checksum"] = json!("changed-by-legacy-repair");
    ga::commit_rows(&f.s, vec![ga::upsert("audio_chunks", audio)]).unwrap();
    assert!(require_capture_source_access(&f.s, &f.recording).is_err());
}
#[test]
fn deleting_both_chunk_ledgers_does_not_clear_completed_source_evidence() {
    let f = fixture();
    reg(&f);
    let c = chunk(&f, &f.reg, 1, 0, 4800);
    append_capture_source_chunk(
        &f.s,
        &begin_source_commit(&f.s),
        &f.project,
        &f.recording,
        &c,
    )
    .unwrap();
    complete_capture_sources(&f.s, &begin_source_commit(&f.s), &f.recording).unwrap();
    ga::commit_rows(
        &f.s,
        vec![
            RelationalRowMutation {
                table: "capture_source_chunks".into(),
                kind: RelationalMutationKind::Delete,
                values: json!({}),
                key: Some(json!({"id":c.id})),
            },
            RelationalRowMutation {
                table: "audio_chunks".into(),
                kind: RelationalMutationKind::Delete,
                values: json!({}),
                key: Some(json!({"id":c.id})),
            },
        ],
    )
    .unwrap();
    assert!(require_capture_source_access(&f.s, &f.recording).is_err());
}

#[test]
fn completed_custody_and_late_quarantine_survive_actual_storage_reopen() {
    let f = fixture();
    reg(&f);
    let c = chunk(&f, &f.reg, 1, 0, 4800);
    append_capture_source_chunk(
        &f.s,
        &begin_source_commit(&f.s),
        &f.project,
        &f.recording,
        &c,
    )
    .unwrap();
    complete_capture_sources(&f.s, &begin_source_commit(&f.s), &f.recording).unwrap();
    let Fixture {
        _temp: t,
        s,
        recording,
        reg,
        ..
    } = f;
    drop(s);
    let open = || {
        Storage::open(OpenOptions {
            path: t.path().join("db").display().to_string(),
            page_cache_mb: Some(16),
            read_only: Some(false),
            vector_dim: Some(4),
            retention: None,
        })
        .unwrap()
    };
    let s = open();
    ga::install(&s).unwrap();
    require_capture_source_access(&s, &recording).unwrap();
    quarantine_capture_source(&s, &begin_source_commit(&s), &reg.id, "late-contamination").unwrap();
    drop(s);
    let s = open();
    ga::install(&s).unwrap();
    assert!(require_capture_source_access(&s, &recording).is_err());
    assert_eq!(
        super::get(&s, "capture_sources", &reg.id).unwrap()["state"],
        "quarantined"
    );
    assert_eq!(row(&s, "capture_source_chunks", &c.id)["id"], c.id);
}
