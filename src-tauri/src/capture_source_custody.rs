//! Trusted in-process source custody for routed capture. No IPC or capture executor is registered.

use genesis_block_native::*;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::PathBuf;
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct CaptureSourceRegistration {
    pub id: String,
    pub epoch: i64,
    pub kind: String,
    pub endpoint_id: Option<String>,
    pub label: String,
    pub sample_rate: i64,
    pub clock_origin_qpc_hns: String,
    pub metadata: Value,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct CaptureSourceChunk {
    pub id: String,
    pub source_id: String,
    pub epoch: i64,
    pub sequence: i64,
    pub path: PathBuf,
    pub start_frame: i64,
    pub end_frame: i64,
    pub start_ms: i64,
    pub end_ms: i64,
    pub checksum: String,
}
#[derive(Clone, Debug)]
pub struct SourceCommitAttempt {
    id: String,
    frontier: u64,
    at: String,
}

use crate::genesis_adapter as ga;
use sha2::{Digest, Sha256};
use std::{
    collections::HashSet,
    io::{Read, Seek, SeekFrom},
    path::Path,
};
const MAX_MS: i64 = 24 * 60 * 60 * 1000;
const MAX_CHUNK_BYTES: u64 = 64 * 1024 * 1024;
/// Caller retains this attempt through uncertain commit reconciliation. Capture before reads.
pub fn begin_source_commit(s: &Storage, id: &str, at: &str) -> Result<SourceCommitAttempt, String> {
    if uuid::Uuid::parse_str(id).is_err() || at.is_empty() || at.len() > 64 {
        return Err("invalid custody attempt".into());
    }
    Ok(SourceCommitAttempt {
        id: id.into(),
        frontier: s.txn_frontier(),
        at: at.into(),
    })
}
fn commit(
    s: &Storage,
    a: &SourceCommitAttempt,
    m: Vec<RelationalRowMutation>,
) -> Result<(), String> {
    s.commit_transaction(GenesisTransaction {
        transaction_id: a.id.clone(),
        expected_frontier: Some(a.frontier),
        relational: vec![RelationalMutationGroup {
            namespace: ga::NAMESPACE.into(),
            mutations: m,
        }],
        graph: BatchInput {
            nodes: vec![],
            edges: vec![],
        },
        vectors: vec![],
    })
    .map(|_| ())
    .map_err(|e| e.to_string())
}
fn columns(t: &str) -> Result<Vec<String>, String> {
    ga::schema()
        .tables
        .into_iter()
        .find(|x| x.name == t)
        .map(|x| x.columns.into_iter().map(|c| c.name).collect())
        .ok_or_else(|| "unknown custody table".into())
}
fn rows(s: &Storage, t: &str, k: &str, v: &str) -> Result<Vec<Value>, String> {
    let cs = columns(t)?;
    let mut result = vec![];
    let mut offset = 0;
    let mut read_bytes = 0usize;
    loop {
        let page = s
            .query_relational(RelationalQuery {
                namespace: ga::NAMESPACE.into(),
                table: t.into(),
                columns: cs.iter().map(|c| format!("{t}.{c}")).collect(),
                joins: vec![],
                filters: vec![RelationalFilter::equal(&format!("{t}.{k}"), json!(v))],
                limit: Some(1000),
                offset: Some(offset),
            })
            .map_err(|e| e.to_string())?;
        let more = page.len() == 1000;
        read_bytes = read_bytes
            .checked_add(serde_json::to_vec(&page).map_err(|e| e.to_string())?.len())
            .ok_or("custody read overflow")?;
        if read_bytes > 64 * 1024 * 1024 {
            return Err("custody read byte budget exceeded".into());
        }
        result.extend(page.into_iter().map(|v| {
            Value::Object(
                v.as_object()
                    .expect("Genesis row object")
                    .iter()
                    .map(|(k, v)| {
                        (
                            k.strip_prefix(&format!("{t}.")).unwrap_or(k).to_owned(),
                            v.clone(),
                        )
                    })
                    .collect(),
            )
        }));
        if result.len() > 200000 {
            return Err("custody read bound exceeded".into());
        }
        if !more {
            return Ok(result);
        }
        offset += 1000;
    }
}
fn get(s: &Storage, t: &str, id: &str) -> Result<Value, String> {
    let mut r = rows(s, t, "id", id)?;
    if r.len() != 1 {
        return Err(format!("missing unique {t}"));
    }
    Ok(r.remove(0))
}
fn text<'a>(v: &'a Value, k: &str) -> Result<&'a str, String> {
    v[k].as_str().ok_or_else(|| format!("missing {k}"))
}
fn int(v: &Value, k: &str) -> Result<i64, String> {
    v[k].as_i64().ok_or_else(|| format!("missing {k}"))
}
fn uuid(s: &str) -> Result<(), String> {
    if uuid::Uuid::parse_str(s)
        .map(|u| u.to_string() == s)
        .unwrap_or(false)
    {
        Ok(())
    } else {
        Err("noncanonical source/recording/chunk UUID".into())
    }
}
fn scope(s: &Storage, p: &str, r: &str) -> Result<(Value, Value), String> {
    let project = get(s, "projects", p)?;
    let recording = get(s, "recordings", r)?;
    if text(&recording, "project_id")? != p {
        return Err("cross-project recording".into());
    }
    Ok((project, recording))
}
fn registration(v: &Value) -> Result<CaptureSourceRegistration, String> {
    serde_json::from_value(v["registration_json"].clone()).map_err(|e| e.to_string())
}
fn valid_endpoint_identity(id: &str) -> bool {
    !id.trim().is_empty() && id.len() <= 4096 && !id.contains('\0')
}
/// Reads the persisted endpoint binding for a live routed recording.
/// This is an authoritative Storage read only; it grants no native execution authority.
pub(crate) fn load_active_endpoint_registration(
    s: &Storage,
    p: &str,
    r: &str,
    source: &str,
    epoch: i64,
) -> Result<CaptureSourceRegistration, String> {
    if epoch <= 0 {
        return Err("active source epoch must be positive".into());
    }
    uuid(p)?;
    uuid(r)?;
    uuid(source)?;

    for _ in 0..3 {
        let frontier = s.txn_frontier();
        let result = (|| {
            let (_, recording) = scope(s, p, r)?;
            if text(&recording, "status")? != "recording"
                || text(&recording, "source")? != "routed_endpoint"
            {
                return Err("active endpoint source requires routed recording".into());
            }

            let row = get(s, "capture_sources", source)?;
            if text(&row, "id")? != source
                || text(&row, "project_id")? != p
                || text(&row, "recording_id")? != r
                || int(&row, "capture_epoch")? != epoch
            {
                return Err("active source ownership/epoch mismatch".into());
            }
            if !matches!(text(&row, "state")?, "registered" | "capturing")
                || !row["closed_at"].is_null()
                || !row["quarantine_json"].is_null()
            {
                return Err("active source is closed, quarantined, or not live".into());
            }

            let registration = registration(&row)?;
            let registration_json =
                serde_json::to_value(&registration).map_err(|e| e.to_string())?;
            if registration_json != row["registration_json"]
                || registration.id != source
                || registration.epoch != epoch
                || registration.kind != "endpoint"
                || registration
                    .endpoint_id
                    .as_deref()
                    .filter(|id| valid_endpoint_identity(id))
                    .is_none()
                || !(8000..=192000).contains(&registration.sample_rate)
                || registration
                    .clock_origin_qpc_hns
                    .parse::<u64>()
                    .ok()
                    .filter(|origin| *origin > 0)
                    .is_none()
                || registration.label.is_empty()
                || registration.label.len() > 256
                || serde_json::to_vec(&registration.metadata)
                    .map_err(|e| e.to_string())?
                    .len()
                    > 16384
            {
                return Err("invalid or mismatched active endpoint registration".into());
            }
            uuid(&registration.id)?;
            Ok(registration)
        })();

        if s.txn_frontier() != frontier {
            continue;
        }
        return result;
    }

    Err("custody changed during all three active registration reads".into())
}
/// Native caller supplies already admitted identities; this API grants no Windows authority.
pub fn register_capture_sources(
    s: &Storage,
    a: &SourceCommitAttempt,
    p: &str,
    r: &str,
    regs: &[CaptureSourceRegistration],
) -> Result<(), String> {
    uuid(p)?;
    uuid(r)?;
    let (_, mut rec) = scope(s, p, r)?;
    if regs.is_empty() || regs.len() > 3 {
        return Err("require one to three explicit sources".into());
    }
    // Source frame positions share one meeting origin. Otherwise independent
    // starts would be silently superimposed by the global duration/checkpoint.
    let origin = &regs[0].clock_origin_qpc_hns;
    if regs.iter().any(|reg| &reg.clock_origin_qpc_hns != origin) {
        return Err("capture sources require one common QPC origin".into());
    }
    let mut ids = HashSet::new();
    let mut endpoints = HashSet::new();
    let mut mics = 0;
    for x in regs {
        uuid(&x.id)?;
        if !ids.insert(&x.id)
            || x.epoch <= 0
            || x.label.is_empty()
            || x.label.len() > 256
            || !(8000..=192000).contains(&x.sample_rate)
            || x.clock_origin_qpc_hns
                .parse::<u64>()
                .ok()
                .filter(|v| *v > 0)
                .is_none()
            || serde_json::to_vec(&x.metadata)
                .map_err(|e| e.to_string())?
                .len()
                > 16384
        {
            return Err("invalid source registration".into());
        }
        match x.kind.as_str() {
            "microphone" => {
                mics += 1;
                if mics > 1 || x.endpoint_id.is_some() {
                    return Err("invalid microphone registration".into());
                }
            }
            "endpoint" => {
                let id = x
                    .endpoint_id
                    .as_deref()
                    .filter(|id| valid_endpoint_identity(id))
                    .ok_or("missing endpoint identity")?;
                if !endpoints.insert(id) || endpoints.len() > 2 {
                    return Err("duplicate or too many endpoints".into());
                }
            }
            _ => return Err("unsupported source kind".into()),
        }
    }
    let previous = rows(s, "capture_sources", "recording_id", r)?;
    if !previous.is_empty() {
        if previous.len() == regs.len()
            && regs.iter().all(|reg| {
                previous.iter().any(|v| {
                    v["id"] == reg.id
                        && v["registration_attempt"] == a.id
                        && registration(v).ok().as_ref() == Some(reg)
                })
            })
        {
            return Ok(());
        }
        return Err("source set already registered; never reset/reassign".into());
    }
    if text(&rec, "status")? != "recording"
        || text(&rec, "source")? == "routed_endpoint"
        || !rows(s, "audio_chunks", "recording_id", r)?.is_empty()
    {
        return Err("source registration requires empty active recording".into());
    }
    let mut mutations = vec![];
    for x in regs {
        if !rows(s, "capture_sources", "id", &x.id)?.is_empty() {
            return Err("source UUID already belongs to another recording/epoch".into());
        }
        mutations.push(ga::upsert("capture_sources",json!({"id":x.id,"project_id":p,"recording_id":r,"capture_epoch":x.epoch,"registration_json":x,"registration_attempt":a.id,"state":"registered","closed_at":null,"last_sequence":0,"last_end_frame":0,"last_end_ms":0,"checkpoint_revision":0,"quarantine_json":null,"created_at":a.at,"updated_at":a.at})));
    }
    rec["source"] = json!("routed_endpoint");
    rec["updated_at"] = json!(a.at);
    mutations.push(ga::upsert("recordings", rec));
    commit(s, a, mutations)
}
fn verify_file(
    project: &Value,
    r: &str,
    reg: &CaptureSourceRegistration,
    c: &CaptureSourceChunk,
) -> Result<i64, String> {
    if !c.path.is_absolute()
        || c.path.to_string_lossy().len() > 4096
        || c.path.to_string_lossy().starts_with("\\\\")
    {
        return Err("local absolute PCM path required".into());
    }
    let project_root = Path::new(text(project, "storage_path")?)
        .canonicalize()
        .map_err(|e| e.to_string())?;
    let expected = project_root
        .join("live")
        .join(r)
        .join("sources")
        .join(&reg.id)
        .join(format!("epoch-{}", reg.epoch));
    let actual = c.path.canonicalize().map_err(|e| e.to_string())?;
    if !actual.starts_with(&expected)
        || expected.canonicalize().map_err(|e| e.to_string())? != expected
    {
        return Err("PCM outside authoritative source directory".into());
    }
    let before = std::fs::metadata(&actual).map_err(|e| e.to_string())?;
    if !before.is_file() || before.len() == 0 || before.len() > MAX_CHUNK_BYTES {
        return Err("PCM byte bound exceeded".into());
    }
    let mut f = std::fs::File::open(&actual).map_err(|e| e.to_string())?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 65536];
    let mut total = 0u64;
    loop {
        let n = f.read(&mut buffer).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        total += n as u64;
        if total > MAX_CHUNK_BYTES {
            return Err("PCM grew beyond byte bound".into());
        }
        hasher.update(&buffer[..n]);
    }
    if format!("{:x}", hasher.finalize()) != c.checksum {
        return Err("PCM checksum mismatch".into());
    }
    f.seek(SeekFrom::Start(0)).map_err(|e| e.to_string())?;
    let mut wav = hound::WavReader::new(f).map_err(|e| e.to_string())?;
    let spec = wav.spec();
    if spec.channels != 1
        || spec.bits_per_sample != 16
        || spec.sample_format != hound::SampleFormat::Int
        || i64::from(spec.sample_rate) != reg.sample_rate
    {
        return Err("unsupported source PCM header".into());
    }
    let mut frames = 0;
    for sample in wav.samples::<i16>() {
        sample.map_err(|e| e.to_string())?;
        frames += 1;
    }
    if frames != c.end_frame - c.start_frame {
        return Err("PCM frame provenance mismatch".into());
    }
    let after = std::fs::metadata(&actual).map_err(|e| e.to_string())?;
    if before.len() != after.len()
        || before.modified().ok() != after.modified().ok()
        || total != after.len()
    {
        return Err("PCM changed during validation".into());
    }
    Ok(total as i64)
}
/// False=new durable append_capture_source_chunk; true=exact same attempt already durable. No enrichment here.
pub fn append_capture_source_chunk(
    s: &Storage,
    a: &SourceCommitAttempt,
    p: &str,
    r: &str,
    c: &CaptureSourceChunk,
) -> Result<bool, String> {
    uuid(&c.id)?;
    uuid(&c.source_id)?;
    let (project, mut rec) = scope(s, p, r)?;
    let mut source = get(s, "capture_sources", &c.source_id)?;
    let reg = registration(&source)?;
    if text(&rec, "source")? != "routed_endpoint"
        || text(&source, "project_id")? != p
        || text(&source, "recording_id")? != r
        || int(&source, "capture_epoch")? != c.epoch
        || reg.id != c.source_id
        || reg.epoch != c.epoch
    {
        return Err("source scope/epoch mismatch".into());
    }
    if c.sequence <= 0
        || c.start_frame < 0
        || c.end_frame <= c.start_frame
        || c.start_ms < 0
        || c.end_ms < c.start_ms
        || c.end_ms > MAX_MS
        || c.end_frame > MAX_MS * reg.sample_rate / 1000
        || c.start_ms != c.start_frame * 1000 / reg.sample_rate
        || c.end_ms != c.end_frame * 1000 / reg.sample_rate
    {
        return Err("invalid bounded source interval".into());
    }
    let bytes = verify_file(&project, r, &reg, c)?;
    let hash = format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&(p, r, c, bytes)).map_err(|e| e.to_string())?)
    );
    let previous = rows(s, "capture_source_chunks", "id", &c.id)?;
    if let Some(old) = previous.first() {
        if old["payload_hash"] == hash && old["attempt_id"] == a.id {
            return Ok(true);
        }
        return Err("conflicting source chunk retry".into());
    }
    if !source["closed_at"].is_null()
        || !["recording", "partial"].contains(&text(&rec, "status")?)
        || !["registered", "capturing", "quarantined"].contains(&text(&source, "state")?)
    {
        return Err("source/recording no longer accepts PCM".into());
    }
    if c.sequence
        != int(&source, "last_sequence")?
            .checked_add(1)
            .ok_or("source sequence overflow")?
        || c.start_frame < int(&source, "last_end_frame")?
        || (text(&source, "state")? != "quarantined"
            && c.start_frame != int(&source, "last_end_frame")?)
    {
        return Err("source sequence or unquarantined gap".into());
    }
    if !rows(s, "audio_chunks", "id", &c.id)?.is_empty() {
        return Err("chunk ID already owned".into());
    }
    let mut checkpoint = get(s, "mobile_recording_checkpoints", r)?;
    let sequence = int(&checkpoint, "segment_count")?
        .checked_add(1)
        .ok_or("sequence overflow")?;
    let duration = int(&rec, "duration_ms")?.max(c.end_ms);
    checkpoint["segment_count"] = json!(sequence);
    checkpoint["safe_offset_ms"] = json!(int(&checkpoint, "safe_offset_ms")?.max(c.end_ms));
    checkpoint["last_checksum"] = json!(c.checksum);
    checkpoint["updated_at"] = json!(a.at);
    rec["duration_ms"] = json!(duration);
    rec["updated_at"] = json!(a.at);
    source["last_sequence"] = json!(c.sequence);
    source["last_end_frame"] = json!(c.end_frame);
    source["last_end_ms"] = json!(c.end_ms);
    source["checkpoint_revision"] = json!(int(&source, "checkpoint_revision")?
        .checked_add(1)
        .ok_or("source revision overflow")?);
    source["updated_at"] = json!(a.at);
    if text(&source, "state")? != "quarantined" {
        source["state"] = json!("capturing")
    }
    commit(
        s,
        a,
        vec![
            ga::upsert(
                "audio_chunks",
                json!({"id":c.id,"recording_id":r,"sequence_no":sequence,"file_path":c.path,"start_ms":c.start_ms,"end_ms":c.end_ms,"byte_size":bytes,"checksum":c.checksum,"created_at":a.at}),
            ),
            ga::upsert(
                "capture_source_chunks",
                json!({"id":c.id,"source_id":c.source_id,"recording_id":r,"capture_epoch":c.epoch,"source_sequence":c.sequence,"global_sequence":sequence,"chunk_json":c,"payload_hash":hash,"byte_size":bytes,"attempt_id":a.id,"created_at":a.at}),
            ),
            ga::upsert("capture_sources", source),
            ga::upsert("mobile_recording_checkpoints", checkpoint),
            ga::upsert("recordings", rec),
        ],
    )?;
    Ok(false)
}
pub fn quarantine_capture_source(
    s: &Storage,
    a: &SourceCommitAttempt,
    id: &str,
    reason: &str,
) -> Result<(), String> {
    if reason.is_empty() || reason.len() > 1024 {
        return Err("bounded quarantine_capture_source reason required".into());
    }
    let mut source = get(s, "capture_sources", id)?;
    if text(&source, "state")? == "quarantined" {
        return if source["quarantine_json"]["reason"] == reason {
            Ok(())
        } else {
            Err("quarantine_capture_source first cause immutable".into())
        };
    }
    let mut rec = get(s, "recordings", text(&source, "recording_id")?)?;
    source["state"] = json!("quarantined");
    source["quarantine_json"] = json!({"reason":reason,"attemptId":a.id,"at":a.at});
    source["updated_at"] = json!(a.at);
    rec["status"] = json!("partial");
    rec["updated_at"] = json!(a.at);
    commit(
        s,
        a,
        vec![
            ga::upsert("capture_sources", source.clone()),
            ga::upsert("recordings", rec),
            ga::upsert(
                "audit_events",
                json!({"id":a.id,"project_id":source["project_id"],"event_type":"capture_source_quarantined","actor":"native-capture","payload_json":{"sourceId":id,"recordingId":source["recording_id"],"epoch":source["capture_epoch"],"reason":reason},"created_at":a.at}),
            ),
        ],
    )
}
pub fn complete_capture_sources(
    s: &Storage,
    a: &SourceCommitAttempt,
    r: &str,
) -> Result<(), String> {
    let mut rec = get(s, "recordings", r)?;
    let mut sources = rows(s, "capture_sources", "recording_id", r)?;
    if sources.is_empty() || sources.len() > 3 || text(&rec, "source")? != "routed_endpoint" {
        return Err("missing routed sources".into());
    }
    let mut partial = false;
    for source in &mut sources {
        source["closed_at"] = json!(a.at);
        if text(source, "state")? == "quarantined" {
            partial = true;
        } else if int(source, "last_sequence")? == 0 {
            return Err("source captured no PCM".into());
        } else {
            source["state"] = json!("completed");
            source["updated_at"] = json!(a.at);
        }
    }
    rec["status"] = json!(if partial { "partial" } else { "completed" });
    rec["updated_at"] = json!(a.at);
    let mut mutations = sources
        .into_iter()
        .map(|v| ga::upsert("capture_sources", v))
        .collect::<Vec<_>>();
    mutations.push(ga::upsert("recordings", rec));
    commit(s, a, mutations)
}
/// Aggregate guard is intentionally conservative until transcript lineage is source-aware.
/// Legacy unbound recordings retain legacy behavior; a routed marker never falls back.
/// Returns the validated storage frontier, NOT a reusable permission token.
/// Consumers must fence publication with that frontier and install quarantine_capture_source cancellation.
pub fn require_capture_source_access(s: &Storage, r: &str) -> Result<u64, String> {
    let frontier = s.txn_frontier();
    let rec = get(s, "recordings", r)?;
    let sources = rows(s, "capture_sources", "recording_id", r)?;
    let source_marker = text(&rec, "source")?;
    if source_marker == "routed_endpoint_pending" {
        return Err("pending routed source registration cannot use legacy access".into());
    }
    if source_marker != "routed_endpoint" {
        return if sources.is_empty() && s.txn_frontier() == frontier {
            Ok(frontier)
        } else {
            Err("source marker mismatch or concurrent mutation".into())
        };
    }
    if text(&rec, "status")? != "completed"
        || sources.is_empty()
        || sources.len() > 3
        || sources.iter().any(|v| {
            v["state"] != "completed"
                || v["closed_at"].is_null()
                || v["last_sequence"].as_i64().unwrap_or(0) == 0
        })
    {
        return Err("routed source not completed/trusted".into());
    }
    let chunks = rows(s, "audio_chunks", "recording_id", r)?;
    let bindings = rows(s, "capture_source_chunks", "recording_id", r)?;
    if chunks.is_empty() || chunks.len() != bindings.len() {
        return Err("routed chunk missing authoritative binding".into());
    }
    let audio = chunks
        .iter()
        .map(|v| Ok((text(v, "id")?, v)))
        .collect::<Result<std::collections::HashMap<_, _>, String>>()?;
    let mut expected_global = Vec::with_capacity(bindings.len());
    let mut duration = 0;
    for source in &sources {
        if source["project_id"] != rec["project_id"] || source["recording_id"] != r {
            return Err("source scope changed".into());
        }
        let reg = registration(source)?;
        if source["id"] != reg.id || source["capture_epoch"] != reg.epoch {
            return Err("registration ownership changed".into());
        }
        let mut owned = bindings
            .iter()
            .filter(|b| b["source_id"] == source["id"])
            .collect::<Vec<_>>();
        owned.sort_by_key(|b| b["source_sequence"].as_i64().unwrap_or(-1));
        if owned.len() as i64 != int(source, "last_sequence")? {
            return Err("source checkpoint/binding count mismatch".into());
        }
        let mut end = 0;
        let mut end_ms = 0;
        for (i, b) in owned.iter().enumerate() {
            let c: CaptureSourceChunk =
                serde_json::from_value(b["chunk_json"].clone()).map_err(|e| e.to_string())?;
            if c.id != text(b, "id")?
                || c.source_id != reg.id
                || c.epoch != reg.epoch
                || b["capture_epoch"] != reg.epoch
                || c.sequence != i as i64 + 1
                || b["source_sequence"] != c.sequence
                || c.start_frame != end
                || c.end_frame <= c.start_frame
            {
                return Err("source provenance/sequence changed".into());
            }
            let row = audio.get(c.id.as_str()).ok_or("binding missing audio")?;
            let bytes = int(b, "byte_size")?;
            let hash = format!(
                "{:x}",
                Sha256::digest(
                    serde_json::to_vec(&(text(&rec, "project_id")?, r, &c, bytes))
                        .map_err(|e| e.to_string())?
                )
            );
            if b["payload_hash"] != hash
                || row["recording_id"] != r
                || row["file_path"] != json!(c.path)
                || row["checksum"] != c.checksum
                || row["start_ms"] != c.start_ms
                || row["end_ms"] != c.end_ms
                || row["byte_size"] != bytes
                || row["sequence_no"] != b["global_sequence"]
            {
                return Err("audio ledger diverged from source authority".into());
            }
            expected_global.push(int(b, "global_sequence")?);
            end = c.end_frame;
            end_ms = c.end_ms;
        }
        if end != int(source, "last_end_frame")? || end_ms != int(source, "last_end_ms")? {
            return Err("source final checkpoint changed".into());
        }
        duration = duration.max(end_ms);
    }
    expected_global.sort_unstable();
    if expected_global.len() != bindings.len()
        || expected_global
            .iter()
            .enumerate()
            .any(|(i, v)| *v != i as i64 + 1)
    {
        return Err("foreign source or global sequence gap".into());
    }
    let cp = get(s, "mobile_recording_checkpoints", r)?;
    if int(&cp, "segment_count")? != bindings.len() as i64
        || int(&cp, "safe_offset_ms")? != duration
        || int(&rec, "duration_ms")? != duration
    {
        return Err("global checkpoint diverged".into());
    }
    if s.txn_frontier() != frontier {
        return Err("custody changed during access validation".into());
    }
    Ok(frontier)
}
#[cfg(test)]
#[path = "capture_source_custody_tests.rs"]
mod tests;
