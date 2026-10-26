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

fn load_active(
    f: &Fixture,
    project: &str,
    recording: &str,
    source: &str,
    epoch: i64,
) -> Result<CaptureSourceRegistration, String> {
    super::load_active_endpoint_registration(&f.s, project, recording, source, epoch)
}

#[test]
fn active_registration_read_accepts_registered_and_checkpoint_advanced_sources() {
    let f = fixture();
    reg(&f);

    assert_eq!(
        load_active(&f, &f.project, &f.recording, &f.reg.id, f.reg.epoch).unwrap(),
        f.reg
    );

    append_capture_source_chunk(
        &f.s,
        &begin_source_commit(&f.s),
        &f.project,
        &f.recording,
        &chunk(&f, &f.reg, 1, 0, 4800),
    )
    .unwrap();

    assert_eq!(
        load_active(&f, &f.project, &f.recording, &f.reg.id, f.reg.epoch).unwrap(),
        f.reg,
        "ordinary checkpoint advancement must not invalidate the active binding"
    );
}

#[test]
fn active_registration_read_rejects_wrong_project_recording_source_and_epoch() {
    let f = fixture();
    reg(&f);

    let other_project = uuid::Uuid::new_v4().to_string();
    let other_recording = uuid::Uuid::new_v4().to_string();
    ga::commit_rows(
        &f.s,
        vec![
            ga::upsert(
                "projects",
                json!({"id":other_project,"name":"other","storage_path":f.root,"created_at":"t","updated_at":"t"}),
            ),
            ga::upsert(
                "recordings",
                json!({"id":other_recording,"project_id":f.project,"source":"routed_endpoint","canonical_audio_path":"unused","status":"recording","duration_ms":0,"created_at":"t","updated_at":"t"}),
            ),
        ],
    )
    .unwrap();

    assert!(load_active(&f, &other_project, &f.recording, &f.reg.id, f.reg.epoch).is_err());
    assert!(load_active(&f, &f.project, &other_recording, &f.reg.id, f.reg.epoch).is_err());
    assert!(load_active(
        &f,
        &f.project,
        &f.recording,
        &uuid::Uuid::new_v4().to_string(),
        f.reg.epoch
    )
    .is_err());
    assert!(load_active(&f, &f.project, &f.recording, &f.reg.id, f.reg.epoch + 1).is_err());
}

#[test]
fn active_registration_read_rejects_missing_quarantined_and_closed_sources() {
    let missing = fixture();
    assert!(load_active(
        &missing,
        &missing.project,
        &missing.recording,
        &missing.reg.id,
        missing.reg.epoch
    )
    .is_err());

    let quarantined = fixture();
    reg(&quarantined);
    quarantine_capture_source(
        &quarantined.s,
        &begin_source_commit(&quarantined.s),
        &quarantined.reg.id,
        "test-quarantine",
    )
    .unwrap();
    assert!(load_active(
        &quarantined,
        &quarantined.project,
        &quarantined.recording,
        &quarantined.reg.id,
        quarantined.reg.epoch
    )
    .is_err());

    let closed = fixture();
    reg(&closed);
    let mut source = super::get(&closed.s, "capture_sources", &closed.reg.id).unwrap();
    source["closed_at"] = json!("2026-10-27T00:00:00Z");
    ga::commit_rows(&closed.s, vec![ga::upsert("capture_sources", source)]).unwrap();
    assert!(load_active(
        &closed,
        &closed.project,
        &closed.recording,
        &closed.reg.id,
        closed.reg.epoch
    )
    .is_err());
}

#[test]
fn active_registration_read_rejects_completed_recording_and_source() {
    let f = fixture();
    reg(&f);
    append_capture_source_chunk(
        &f.s,
        &begin_source_commit(&f.s),
        &f.project,
        &f.recording,
        &chunk(&f, &f.reg, 1, 0, 4800),
    )
    .unwrap();
    complete_capture_sources(&f.s, &begin_source_commit(&f.s), &f.recording).unwrap();

    assert_eq!(
        super::get(&f.s, "recordings", &f.recording).unwrap()["status"],
        "completed"
    );
    assert_eq!(
        super::get(&f.s, "capture_sources", &f.reg.id).unwrap()["state"],
        "completed"
    );
    assert!(load_active(&f, &f.project, &f.recording, &f.reg.id, f.reg.epoch).is_err());
}

#[test]
fn active_registration_read_rejects_completed_or_unknown_source_while_recording() {
    let f = fixture();
    reg(&f);
    let original = super::get(&f.s, "capture_sources", &f.reg.id).unwrap();

    for state in ["completed", "unrecognized-state"] {
        let mut changed = original.clone();
        changed["state"] = json!(state);
        ga::commit_rows(&f.s, vec![ga::upsert("capture_sources", changed)]).unwrap();

        assert_eq!(
            super::get(&f.s, "recordings", &f.recording).unwrap()["status"],
            "recording",
            "the recording must stay active so rejection comes from the source state"
        );
        assert!(
            load_active(&f, &f.project, &f.recording, &f.reg.id, f.reg.epoch).is_err(),
            "source state must be rejected while the recording remains active: {state}"
        );
    }
}

#[test]
fn registration_rejects_blank_or_nul_endpoint_ids() {
    for endpoint_id in [" \t\r\n", "endpoint\0identity"] {
        let f = fixture();
        let mut invalid_registration = f.reg.clone();
        invalid_registration.endpoint_id = Some(endpoint_id.into());
        assert!(
            register_capture_sources(
                &f.s,
                &begin_source_commit(&f.s),
                &f.project,
                &f.recording,
                std::slice::from_ref(&invalid_registration),
            )
            .is_err(),
            "registration must reject invalid endpoint identity {endpoint_id:?}"
        );
    }
}

#[test]
fn active_registration_read_rejects_blank_or_nul_persisted_endpoint_ids() {
    let f = fixture();
    reg(&f);
    let original = super::get(&f.s, "capture_sources", &f.reg.id).unwrap();

    for endpoint_id in [" \t\r\n", "endpoint\0identity"] {
        let mut changed = original.clone();
        changed["registration_json"]["endpoint_id"] = json!(endpoint_id);
        ga::commit_rows(&f.s, vec![ga::upsert("capture_sources", changed)]).unwrap();
        assert!(
            load_active(&f, &f.project, &f.recording, &f.reg.id, f.reg.epoch).is_err(),
            "active read must reject persisted endpoint identity {endpoint_id:?}"
        );
    }
}

#[test]
fn active_registration_read_rejects_mutated_row_and_registration_bindings() {
    let f = fixture();
    reg(&f);

    let other_project = uuid::Uuid::new_v4().to_string();
    let other_recording = uuid::Uuid::new_v4().to_string();
    ga::commit_rows(
        &f.s,
        vec![
            ga::upsert(
                "projects",
                json!({"id":other_project,"name":"other","storage_path":f.root,"created_at":"t","updated_at":"t"}),
            ),
            ga::upsert(
                "recordings",
                json!({"id":other_recording,"project_id":f.project,"source":"routed_endpoint","canonical_audio_path":"unused","status":"recording","duration_ms":0,"created_at":"t","updated_at":"t"}),
            ),
        ],
    )
    .unwrap();

    let original = super::get(&f.s, "capture_sources", &f.reg.id).unwrap();
    let mut changed = original.clone();
    changed["project_id"] = json!(other_project);
    ga::commit_rows(&f.s, vec![ga::upsert("capture_sources", changed)]).unwrap();
    assert!(load_active(&f, &f.project, &f.recording, &f.reg.id, f.reg.epoch).is_err());

    ga::commit_rows(&f.s, vec![ga::upsert("capture_sources", original.clone())]).unwrap();
    let mut changed = original.clone();
    changed["recording_id"] = json!(other_recording);
    ga::commit_rows(&f.s, vec![ga::upsert("capture_sources", changed)]).unwrap();
    assert!(load_active(&f, &f.project, &f.recording, &f.reg.id, f.reg.epoch).is_err());

    ga::commit_rows(&f.s, vec![ga::upsert("capture_sources", original.clone())]).unwrap();
    let mut changed = original.clone();
    changed["capture_epoch"] = json!(f.reg.epoch + 1);
    ga::commit_rows(&f.s, vec![ga::upsert("capture_sources", changed)]).unwrap();
    assert!(load_active(&f, &f.project, &f.recording, &f.reg.id, f.reg.epoch).is_err());

    let invalid_registration_fields = [
        ("id", json!(uuid::Uuid::new_v4().to_string())),
        ("epoch", json!(0)),
        ("kind", json!("microphone")),
        ("endpoint_id", Value::Null),
        ("endpoint_id", json!("")),
        ("sample_rate", json!(7999)),
        ("sample_rate", json!(192001)),
        ("clock_origin_qpc_hns", json!("0")),
        ("clock_origin_qpc_hns", json!("not-a-number")),
    ];
    for (field, invalid_value) in invalid_registration_fields {
        let mut changed = original.clone();
        changed["registration_json"][field] = invalid_value;
        ga::commit_rows(&f.s, vec![ga::upsert("capture_sources", changed)]).unwrap();
        assert!(
            load_active(&f, &f.project, &f.recording, &f.reg.id, f.reg.epoch).is_err(),
            "invalid registration field must be rejected: {field}"
        );
    }
}

#[test]
fn pending_routed_recording_is_not_admitted_as_an_unbound_legacy_recording() {
    let f = fixture();
    assert!(require_capture_source_access(&f.s, &f.recording).is_ok());

    let mut recording = super::get(&f.s, "recordings", &f.recording).unwrap();
    recording["source"] = json!("routed_endpoint_pending");
    ga::commit_rows(&f.s, vec![ga::upsert("recordings", recording)]).unwrap();

    assert!(
        require_capture_source_access(&f.s, &f.recording).is_err(),
        "pending routed recording with no registered sources must fail closed"
    );
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
