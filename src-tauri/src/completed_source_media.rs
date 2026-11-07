//! Private completed routed-source reader. No IPC or publication authority.
//!
//! The CallMD §7 bound is 20,000 descriptors / 8 MiB of player metadata. It is
//! not a promise that SDK queries, the existing aggregate custody guard, and
//! all allocator overhead together peak below 8 MiB. We count projected
//! authoritative metadata before retaining descriptors, read small deterministic
//! pages, and preserve the aggregate guard unchanged. No new query principal,
//! schema or storage authority is introduced. A manifest frontier is never a
//! lease: the caller must separately fence publication and cancel on quarantine.

use crate::{capture_source_custody as custody, desktop_playback, genesis_adapter as ga};
use genesis_block_native::Storage;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::{BufReader, Read, Seek, SeekFrom, Write},
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};

const MAX_DESCRIPTORS: usize = 20_000;
const MAX_METADATA_BYTES: usize = 8 * 1024 * 1024;
const READ_BYTES: usize = 64 * 1024;
const MAX_CHUNK_BYTES: u64 = 64 * 1024 * 1024;
const DEADLINE: Duration = Duration::from_secs(10);
const PAGE_ROWS: u32 = 64;

#[derive(Debug)]
pub(crate) struct Manifest {
    pub(crate) frontier: u64,
    pub(crate) metadata_bytes: usize,
    pub(crate) project_id: String,
    pub(crate) recording_id: String,
    storage_path: String,
    pub(crate) sources: Vec<CompletedSource>,
}

#[derive(Debug)]
pub(crate) struct CompletedSource {
    pub(crate) id: String,
    pub(crate) epoch: i64,
    pub(crate) label: String,
    pub(crate) kind: String,
    pub(crate) sample_rate: u32,
    pub(crate) chunks: Vec<CompletedChunk>,
}

#[derive(Debug)]
pub(crate) struct CompletedChunk {
    pub(crate) chunk: custody::CaptureSourceChunk,
    pub(crate) byte_size: u64,
}

#[derive(Default)]
struct MetadataBudget(usize);
impl Write for MetadataBudget {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0 = self
            .0
            .checked_add(bytes.len())
            .filter(|n| *n <= MAX_METADATA_BYTES)
            .ok_or_else(|| std::io::Error::other("RESOURCE_LIMIT"))?;
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
impl MetadataBudget {
    fn charge(&mut self, value: &Value) -> Result<(), String> {
        serde_json::to_writer(self, value).map_err(|_| "RESOURCE_LIMIT".into())
    }
}

fn text<'a>(row: &'a Value, key: &str) -> Result<&'a str, String> {
    row[key]
        .as_str()
        .ok_or_else(|| "PLAYBACK_SOURCE_INVALID".into())
}

fn integer(row: &Value, key: &str) -> Result<i64, String> {
    row[key]
        .as_i64()
        .ok_or_else(|| "PLAYBACK_SOURCE_INVALID".into())
}

fn check_deadline(deadline: Instant) -> Result<(), String> {
    if Instant::now() >= deadline {
        Err("PLAYBACK_OPEN_TIMEOUT".into())
    } else {
        Ok(())
    }
}

fn check_open(cancel: &AtomicBool, deadline: Instant) -> Result<(), String> {
    if cancel.load(Ordering::Acquire) {
        return Err("PLAYBACK_CANCELLED".into());
    }
    check_deadline(deadline)
}

fn check_frontier(storage: &Storage, frontier: u64) -> Result<(), String> {
    if storage.txn_frontier() != frontier {
        Err("PLAYBACK_SOURCE_CHANGED".into())
    } else {
        Ok(())
    }
}

fn one(storage: &Storage, table: &str, columns: &[&str], id: &str) -> Result<Value, String> {
    let mut rows = ga::query(
        storage,
        table,
        columns,
        vec![ga::eq(table, "id", json!(id))],
        1,
    )?;
    rows.pop().ok_or_else(|| "PLAYBACK_SOURCE_MISSING".into())
}

fn canonical_uuid(value: &str) -> bool {
    uuid::Uuid::parse_str(value)
        .map(|id| id.to_string() == value)
        .unwrap_or(false)
}

pub(crate) fn completed_manifest(
    storage: &Storage,
    project: &str,
    recording: &str,
) -> Result<Manifest, String> {
    let deadline = Instant::now() + DEADLINE;
    for id in [project, recording] {
        if !canonical_uuid(id) {
            return Err("PLAYBACK_SOURCE_INVALID".into());
        }
    }
    let frontier = custody::require_capture_source_access(storage, recording)?;
    check_deadline(deadline)?;
    let mut budget = MetadataBudget::default();
    let row = one(
        storage,
        "recordings",
        &["project_id", "source", "status"],
        recording,
    )?;
    budget.charge(&row)?;
    if text(&row, "recordings.project_id")? != project
        || text(&row, "recordings.source")? != "routed_endpoint"
        || text(&row, "recordings.status")? != "completed"
    {
        return Err("PLAYBACK_SOURCE_INVALID".into());
    }
    let row = one(storage, "projects", &["storage_path"], project)?;
    budget.charge(&row)?;
    let storage_path = text(&row, "projects.storage_path")?.to_owned();
    let rows = ga::query_page(
        storage,
        "capture_sources",
        &[
            "id",
            "project_id",
            "recording_id",
            "capture_epoch",
            "registration_json",
        ],
        vec![ga::eq("capture_sources", "recording_id", json!(recording))],
        4,
        0,
    )?;
    if rows.is_empty() || rows.len() > 3 {
        return Err("PLAYBACK_SOURCE_INVALID".into());
    }
    let mut sources = Vec::new();
    for mut row in rows {
        budget.charge(&row)?;
        let reg: custody::CaptureSourceRegistration =
            serde_json::from_value(row["capture_sources.registration_json"].take())
                .map_err(|_| "PLAYBACK_SOURCE_INVALID")?;
        if text(&row, "capture_sources.project_id")? != project
            || text(&row, "capture_sources.recording_id")? != recording
            || text(&row, "capture_sources.id")? != reg.id
            || integer(&row, "capture_sources.capture_epoch")? != reg.epoch
            || !canonical_uuid(&reg.id)
            || reg.epoch <= 0
            || !(8000..=192000).contains(&reg.sample_rate)
        {
            return Err("PLAYBACK_SOURCE_INVALID".into());
        }
        sources.push(CompletedSource {
            id: reg.id,
            epoch: reg.epoch,
            label: reg.label,
            kind: reg.kind,
            sample_rate: reg.sample_rate as u32,
            chunks: Vec::new(),
        });
    }
    let mut offset = 0;
    let mut count = 0usize;
    loop {
        check_deadline(deadline)?;
        check_frontier(storage, frontier)?;
        let page = ga::query_page(
            storage,
            "capture_source_chunks",
            &[
                "id",
                "source_id",
                "capture_epoch",
                "source_sequence",
                "chunk_json",
                "byte_size",
            ],
            vec![ga::eq(
                "capture_source_chunks",
                "recording_id",
                json!(recording),
            )],
            PAGE_ROWS,
            offset,
        )?;
        let more = page.len() == PAGE_ROWS as usize;
        for mut row in page {
            count += 1;
            if count > MAX_DESCRIPTORS {
                return Err("RESOURCE_LIMIT".into());
            }
            budget.charge(&row)?;
            let chunk: custody::CaptureSourceChunk =
                serde_json::from_value(row["capture_source_chunks.chunk_json"].take())
                    .map_err(|_| "PLAYBACK_SOURCE_INVALID")?;
            let bytes = integer(&row, "capture_source_chunks.byte_size")?;
            if text(&row, "capture_source_chunks.id")? != chunk.id
                || !canonical_uuid(&chunk.id)
                || text(&row, "capture_source_chunks.source_id")? != chunk.source_id
                || integer(&row, "capture_source_chunks.capture_epoch")? != chunk.epoch
                || integer(&row, "capture_source_chunks.source_sequence")? != chunk.sequence
                || bytes <= 0
                || bytes as u64 > MAX_CHUNK_BYTES
            {
                return Err("PLAYBACK_SOURCE_INVALID".into());
            }
            let source = sources
                .iter_mut()
                .find(|s| s.id == chunk.source_id && s.epoch == chunk.epoch)
                .ok_or("PLAYBACK_SOURCE_INVALID")?;
            source.chunks.push(CompletedChunk {
                chunk,
                byte_size: bytes as u64,
            });
        }
        if !more {
            break;
        }
        offset += PAGE_ROWS;
    }
    for source in &mut sources {
        if source.chunks.is_empty() {
            return Err("PLAYBACK_SOURCE_MISSING".into());
        }
        source.chunks.sort_unstable_by_key(|c| c.chunk.sequence);
        let mut end = 0;
        for (index, descriptor) in source.chunks.iter().enumerate() {
            let c = &descriptor.chunk;
            if c.sequence != index as i64 + 1 || c.start_frame != end || c.end_frame <= end {
                return Err("PLAYBACK_SOURCE_INVALID".into());
            }
            end = c.end_frame;
        }
    }
    // Serialized size alone does not account for Vec capacity or heap-backed
    // String/PathBuf storage. Charge retained capacities and struct storage too.
    let mut retained = std::mem::size_of::<Manifest>()
        .checked_add(
            sources
                .capacity()
                .saturating_mul(std::mem::size_of::<CompletedSource>()),
        )
        .and_then(|n| n.checked_add(project.len()))
        .and_then(|n| n.checked_add(recording.len()))
        .and_then(|n| n.checked_add(storage_path.capacity()))
        .ok_or("RESOURCE_LIMIT")?;
    for source in &sources {
        retained = retained
            .checked_add(source.id.capacity())
            .and_then(|n| n.checked_add(source.label.capacity()))
            .and_then(|n| n.checked_add(source.kind.capacity()))
            .and_then(|n| {
                n.checked_add(
                    source
                        .chunks
                        .capacity()
                        .saturating_mul(std::mem::size_of::<CompletedChunk>()),
                )
            })
            .ok_or("RESOURCE_LIMIT")?;
        for descriptor in &source.chunks {
            let c = &descriptor.chunk;
            retained = retained
                .checked_add(c.id.capacity())
                .and_then(|n| n.checked_add(c.source_id.capacity()))
                .and_then(|n| n.checked_add(c.path.capacity()))
                .and_then(|n| n.checked_add(c.checksum.capacity()))
                .ok_or("RESOURCE_LIMIT")?;
        }
    }
    if retained > MAX_METADATA_BYTES {
        return Err("RESOURCE_LIMIT".into());
    }
    check_deadline(deadline)?;
    check_frontier(storage, frontier)?;
    Ok(Manifest {
        frontier,
        metadata_bytes: retained,
        project_id: project.to_owned(),
        recording_id: recording.to_owned(),
        storage_path,
        sources,
    })
}

#[cfg(windows)]
pub(crate) fn open_read_locked(path: &Path) -> Result<File, String> {
    use std::os::windows::fs::OpenOptionsExt;
    use windows_sys::Win32::Storage::FileSystem::FILE_SHARE_READ;
    std::fs::OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_READ)
        .open(path)
        .map_err(|_| "PLAYBACK_IO_FAILED".into())
}

#[cfg(not(windows))]
pub(crate) fn open_read_locked(_: &Path) -> Result<File, String> {
    // Routed native capture is Windows-only. Do not silently promise Windows
    // sharing/immutability semantics on a platform with advisory file locks.
    Err("PLAYBACK_SOURCE_UNSUPPORTED".into())
}

pub(crate) fn open_verified_chunk(
    storage: &Storage,
    data_root: &Path,
    manifest: &Manifest,
    source_id: &str,
    epoch: i64,
    sequence: i64,
    cancel: &AtomicBool,
) -> Result<hound::WavReader<BufReader<File>>, String> {
    let deadline = Instant::now() + DEADLINE;
    check_open(cancel, deadline)?;
    if custody::require_capture_source_access(storage, &manifest.recording_id)? != manifest.frontier
    {
        return Err("PLAYBACK_SOURCE_CHANGED".into());
    }
    let source = manifest
        .sources
        .iter()
        .find(|s| s.id == source_id && s.epoch == epoch)
        .ok_or("PLAYBACK_SOURCE_INVALID")?;
    let descriptor = source
        .chunks
        .iter()
        .find(|c| c.chunk.sequence == sequence)
        .ok_or("PLAYBACK_SOURCE_MISSING")?;
    let c = &descriptor.chunk;
    let root = desktop_playback::project_custody_root(data_root, &manifest.storage_path)
        .map_err(|e| e.code)?;
    let expected = root
        .join("live")
        .join(&manifest.recording_id)
        .join("sources")
        .join(&source.id)
        .join(format!("epoch-{}", source.epoch));
    let raw_path = c.path.to_str().ok_or("PLAYBACK_PATH_DENIED")?;
    let candidate = desktop_playback::resolve_ledger_path(&root, raw_path).map_err(|e| e.code)?;
    check_open(cancel, deadline)?;
    let mut file = open_read_locked(&candidate)?;
    let actual = desktop_playback::final_path_from_handle(&file, &candidate).map_err(|e| e.code)?;
    if !desktop_playback::normalized_path_inside(&actual, &root)
        || !desktop_playback::normalized_path_inside(&actual, &expected)
    {
        return Err("PLAYBACK_PATH_DENIED".into());
    }
    let metadata = file.metadata().map_err(|_| "PLAYBACK_IO_FAILED")?;
    if !metadata.is_file() || metadata.len() != descriptor.byte_size {
        return Err("PLAYBACK_SOURCE_INVALID".into());
    }
    let mut digest = Sha256::new();
    let mut buffer = [0u8; READ_BYTES];
    let mut bytes = 0u64;
    loop {
        check_open(cancel, deadline)?;
        check_frontier(storage, manifest.frontier)?;
        let n = file.read(&mut buffer).map_err(|_| "PLAYBACK_IO_FAILED")?;
        if n == 0 {
            break;
        }
        bytes += n as u64;
        if bytes > descriptor.byte_size {
            return Err("PLAYBACK_SOURCE_INVALID".into());
        }
        digest.update(&buffer[..n]);
    }
    if bytes != descriptor.byte_size || format!("{:x}", digest.finalize()) != c.checksum {
        return Err("PLAYBACK_SOURCE_INVALID".into());
    }
    file.seek(SeekFrom::Start(0))
        .map_err(|_| "PLAYBACK_IO_FAILED")?;
    let mut reader = hound::WavReader::new(BufReader::with_capacity(READ_BYTES, file))
        .map_err(|_| "PLAYBACK_FORMAT_UNSUPPORTED")?;
    let spec = reader.spec();
    if spec.channels != 1
        || spec.bits_per_sample != 16
        || spec.sample_format != hound::SampleFormat::Int
        || spec.sample_rate != source.sample_rate
    {
        return Err("PLAYBACK_FORMAT_UNSUPPORTED".into());
    }
    let frames = c
        .end_frame
        .checked_sub(c.start_frame)
        .filter(|v| *v > 0)
        .ok_or("PLAYBACK_SOURCE_INVALID")? as u64;
    if u64::from(reader.duration()) != frames {
        return Err("PLAYBACK_SOURCE_INVALID".into());
    }
    let mut seen = 0u64;
    for sample in reader.samples::<i16>() {
        sample.map_err(|_| "PLAYBACK_SOURCE_INVALID")?;
        seen += 1;
        if seen.is_multiple_of(4096) {
            check_open(cancel, deadline)?;
        }
    }
    if seen != frames {
        return Err("PLAYBACK_SOURCE_INVALID".into());
    }
    reader.seek(0).map_err(|_| "PLAYBACK_IO_FAILED")?;
    check_open(cancel, deadline)?;
    check_frontier(storage, manifest.frontier)?;
    Ok(reader)
}

#[cfg(all(test, windows))]
pub(crate) mod tests {
    use super::*;
    use crate::{capture_source_custody as custody, genesis_adapter as ga};
    use genesis_block_native::OpenOptions;
    use serde_json::json;
    use sha2::{Digest, Sha256};
    use std::{fs, path::PathBuf, sync::Arc};
    use uuid::Uuid;

    pub(crate) struct Fixture {
        pub(crate) storage: Arc<Storage>,
        pub(crate) temp: tempfile::TempDir,
        pub(crate) project: String,
        pub(crate) recording: String,
        pub(crate) regs: Vec<custody::CaptureSourceRegistration>,
        pub(crate) paths: Vec<PathBuf>,
    }
    fn attempt(s: &Storage) -> custody::SourceCommitAttempt {
        custody::begin_source_commit(s, &Uuid::new_v4().to_string(), "2026-10-29T00:00:00Z")
            .unwrap()
    }
    fn stored_row(s: &Storage, table: &str, id: &str) -> serde_json::Value {
        let names = ga::schema()
            .tables
            .into_iter()
            .find(|t| t.name == table)
            .unwrap()
            .columns
            .into_iter()
            .map(|c| c.name)
            .collect::<Vec<_>>();
        let columns = names.iter().map(String::as_str).collect::<Vec<_>>();
        let mut values =
            ga::query(s, table, &columns, vec![ga::eq(table, "id", json!(id))], 1).unwrap();
        let value = values.pop().unwrap();
        serde_json::Value::Object(
            value
                .as_object()
                .unwrap()
                .iter()
                .map(|(k, v)| {
                    (
                        k.strip_prefix(&format!("{table}.")).unwrap().to_string(),
                        v.clone(),
                    )
                })
                .collect(),
        )
    }

    fn rewrite_chunk_path(f: &Fixture, source: usize, new_path: &Path) {
        let id = ga::query(
            &f.storage,
            "capture_source_chunks",
            &["id"],
            vec![ga::eq(
                "capture_source_chunks",
                "source_id",
                json!(f.regs[source].id),
            )],
            1,
        )
        .unwrap()[0]["capture_source_chunks.id"]
            .as_str()
            .unwrap()
            .to_string();
        let mut binding = stored_row(&f.storage, "capture_source_chunks", &id);
        let mut audio = stored_row(&f.storage, "audio_chunks", &id);
        binding["chunk_json"]["path"] = json!(new_path);
        let chunk: custody::CaptureSourceChunk =
            serde_json::from_value(binding["chunk_json"].clone()).unwrap();
        binding["payload_hash"] = json!(format!(
            "{:x}",
            Sha256::digest(
                serde_json::to_vec(&(
                    &f.project,
                    &f.recording,
                    &chunk,
                    binding["byte_size"].as_i64().unwrap()
                ))
                .unwrap()
            )
        ));
        audio["file_path"] = json!(new_path);
        ga::commit_rows(
            &f.storage,
            vec![
                ga::upsert("capture_source_chunks", binding),
                ga::upsert("audio_chunks", audio),
            ],
        )
        .unwrap();
    }
    impl Fixture {
        pub(crate) fn new(complete: bool) -> Self {
            let temp = tempfile::tempdir().unwrap();
            let root = temp.path().join("projects").join("fixture");
            fs::create_dir_all(&root).unwrap();
            let storage = Arc::new(
                Storage::open(OpenOptions {
                    path: temp.path().join("db").display().to_string(),
                    page_cache_mb: Some(16),
                    read_only: Some(false),
                    vector_dim: Some(4),
                    retention: None,
                })
                .unwrap(),
            );
            ga::install(&storage).unwrap();
            let project = Uuid::new_v4().to_string();
            let recording = Uuid::new_v4().to_string();
            ga::commit_rows(&storage,vec![
                ga::upsert("projects",json!({"id":project,"name":"test","storage_path":root,"created_at":"2026-10-29T00:00:00Z","updated_at":"2026-10-29T00:00:00Z"})),
                ga::upsert("recordings",json!({"id":recording,"project_id":project,"source":"microphone","canonical_audio_path":"unused","status":"recording","duration_ms":0,"created_at":"2026-10-29T00:00:00Z","updated_at":"2026-10-29T00:00:00Z"})),
                ga::upsert("mobile_recording_checkpoints",json!({"id":recording,"recording_id":recording,"safe_offset_ms":0,"segment_count":0,"last_checksum":null,"updated_at":"t"}))
            ]).unwrap();
            let regs = (0..2)
                .map(|n| custody::CaptureSourceRegistration {
                    id: Uuid::new_v4().to_string(),
                    epoch: 1,
                    kind: "endpoint".into(),
                    endpoint_id: Some(format!("endpoint-{n}")),
                    label: format!("source-{n}"),
                    sample_rate: 48000,
                    clock_origin_qpc_hns: "1234".into(),
                    metadata: json!({"policyVersion":1}),
                })
                .collect::<Vec<_>>();
            custody::register_capture_sources(
                &storage,
                &attempt(&storage),
                &project,
                &recording,
                &regs,
            )
            .unwrap();
            let mut paths = Vec::new();
            for (n, reg) in regs.iter().enumerate() {
                let dir = root
                    .join("live")
                    .join(&recording)
                    .join("sources")
                    .join(&reg.id)
                    .join("epoch-1");
                fs::create_dir_all(&dir).unwrap();
                let path = dir.join("chunk-1.wav");
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
                w.write_sample((n as i16 + 1) * 111).unwrap();
                w.finalize().unwrap();
                let checksum = format!("{:x}", Sha256::digest(fs::read(&path).unwrap()));
                custody::append_capture_source_chunk(
                    &storage,
                    &attempt(&storage),
                    &project,
                    &recording,
                    &custody::CaptureSourceChunk {
                        id: Uuid::new_v4().to_string(),
                        source_id: reg.id.clone(),
                        epoch: 1,
                        sequence: 1,
                        path: path.clone(),
                        start_frame: 0,
                        end_frame: 1,
                        start_ms: 0,
                        end_ms: 0,
                        checksum,
                    },
                )
                .unwrap();
                paths.push(path);
            }
            if complete {
                custody::complete_capture_sources(&storage, &attempt(&storage), &recording)
                    .unwrap();
            }
            Self {
                temp,
                storage,
                project,
                recording,
                regs,
                paths,
            }
        }
        fn manifest(&self) -> Manifest {
            completed_manifest(&self.storage, &self.project, &self.recording).unwrap()
        }
        fn open(
            &self,
            m: &Manifest,
            index: usize,
            epoch: i64,
        ) -> Result<hound::WavReader<std::io::BufReader<std::fs::File>>, String> {
            open_verified_chunk(
                &self.storage,
                self.temp.path(),
                m,
                &self.regs[index].id,
                epoch,
                1,
                &AtomicBool::new(false),
            )
        }
    }
    #[test]
    fn distinct_same_named_sources_preserve_one_frame_tail() {
        let f = Fixture::new(true);
        let m = f.manifest();
        for i in 0..2 {
            let mut r = f.open(&m, i, 1).unwrap();
            assert_eq!(
                r.samples::<i16>().collect::<Result<Vec<_>, _>>().unwrap(),
                vec![(i as i16 + 1) * 111]
            );
        }
    }
    #[test]
    fn wrong_project_recording_source_and_epoch_fail_closed() {
        let f = Fixture::new(true);
        assert!(completed_manifest(&f.storage, &Uuid::new_v4().to_string(), &f.recording).is_err());
        assert!(completed_manifest(&f.storage, &f.project, &Uuid::new_v4().to_string()).is_err());
        let m = f.manifest();
        assert!(f.open(&m, 0, 2).is_err());
        assert!(open_verified_chunk(
            &f.storage,
            f.temp.path(),
            &m,
            &Uuid::new_v4().to_string(),
            1,
            1,
            &AtomicBool::new(false)
        )
        .is_err());
    }
    #[test]
    fn active_quarantined_and_tampered_chunks_are_denied() {
        let f = Fixture::new(false);
        assert!(completed_manifest(&f.storage, &f.project, &f.recording).is_err());
        custody::complete_capture_sources(&f.storage, &attempt(&f.storage), &f.recording).unwrap();
        let m = f.manifest();
        let mut bytes = fs::read(&f.paths[0]).unwrap();
        let n = bytes.len();
        bytes[n - 1] ^= 1;
        fs::write(&f.paths[0], bytes).unwrap();
        assert!(f.open(&m, 0, 1).is_err());
        custody::quarantine_capture_source(&f.storage, &attempt(&f.storage), &f.regs[1].id, "test")
            .unwrap();
        assert!(f.open(&m, 0, 1).is_err());
        assert!(completed_manifest(&f.storage, &f.project, &f.recording).is_err());
    }

    #[test]
    fn cancelled_and_truncated_media_are_denied() {
        let f = Fixture::new(true);
        let m = f.manifest();
        assert!(open_verified_chunk(
            &f.storage,
            f.temp.path(),
            &m,
            &f.regs[0].id,
            1,
            1,
            &AtomicBool::new(true)
        )
        .is_err());
        let bytes = fs::read(&f.paths[0]).unwrap();
        fs::write(&f.paths[0], &bytes[..bytes.len() - 1]).unwrap();
        assert!(f.open(&m, 0, 1).is_err());
    }

    #[test]
    fn unrelated_commit_invalidates_a_prepared_manifest() {
        let f = Fixture::new(true);
        let m = f.manifest();
        ga::commit_rows(&f.storage, vec![ga::upsert("projects", json!({"id":Uuid::new_v4().to_string(),"name":"other","storage_path":f.temp.path(),"created_at":"t","updated_at":"t"}))]).unwrap();
        assert!(f.open(&m, 0, 1).is_err());
        let fresh = f.manifest();
        assert!(f.open(&fresh, 0, 1).is_ok());
    }

    #[test]
    fn authoritative_path_outside_source_directory_is_denied() {
        let f = Fixture::new(true);
        let foreign = f.temp.path().join("foreign.wav");
        fs::copy(&f.paths[0], &foreign).unwrap();
        rewrite_chunk_path(&f, 0, &foreign);
        assert!(custody::require_capture_source_access(&f.storage, &f.recording).is_ok());
        let m = f.manifest();
        assert!(f.open(&m, 0, 1).is_err());
    }

    #[test]
    fn pending_source_marker_is_denied() {
        let f = Fixture::new(true);
        let mut row = stored_row(&f.storage, "recordings", &f.recording);
        row["source"] = json!("routed_endpoint_pending");
        ga::commit_rows(&f.storage, vec![ga::upsert("recordings", row)]).unwrap();
        assert!(completed_manifest(&f.storage, &f.project, &f.recording).is_err());
    }

    #[test]
    fn a_custody_row_with_a_path_like_source_identity_is_denied() {
        let f = Fixture::new(true);
        let old_id = &f.regs[0].id;
        let forged_id = f.temp.path().join("foreign-source").display().to_string();
        let mut source = stored_row(&f.storage, "capture_sources", old_id);
        source["id"] = json!(forged_id);
        source["registration_json"]["id"] = json!(forged_id);
        let id = ga::query(
            &f.storage,
            "capture_source_chunks",
            &["id"],
            vec![ga::eq("capture_source_chunks", "source_id", json!(old_id))],
            1,
        )
        .unwrap()[0]["capture_source_chunks.id"]
            .as_str()
            .unwrap()
            .to_string();
        let mut binding = stored_row(&f.storage, "capture_source_chunks", &id);
        binding["source_id"] = json!(forged_id);
        binding["chunk_json"]["source_id"] = json!(forged_id);
        let chunk: custody::CaptureSourceChunk =
            serde_json::from_value(binding["chunk_json"].clone()).unwrap();
        binding["payload_hash"] = json!(format!(
            "{:x}",
            Sha256::digest(
                serde_json::to_vec(&(
                    &f.project,
                    &f.recording,
                    &chunk,
                    binding["byte_size"].as_i64().unwrap()
                ))
                .unwrap()
            )
        ));
        ga::commit_rows(
            &f.storage,
            vec![
                ga::upsert("capture_sources", source),
                ga::upsert("capture_source_chunks", binding),
                ga::delete("capture_sources", old_id),
            ],
        )
        .unwrap();
        assert!(custody::require_capture_source_access(&f.storage, &f.recording).is_ok());
        assert!(completed_manifest(&f.storage, &f.project, &f.recording).is_err());
    }

    #[test]
    fn metadata_budget_rejects_oversized_authoritative_registration() {
        let f = Fixture::new(true);
        let mut row = stored_row(&f.storage, "capture_sources", &f.regs[0].id);
        row["registration_json"]["metadata"] = json!({"oversized":"x".repeat(8 * 1024 * 1024)});
        ga::commit_rows(&f.storage, vec![ga::upsert("capture_sources", row)]).unwrap();
        assert!(custody::require_capture_source_access(&f.storage, &f.recording).is_ok());
        assert_eq!(
            completed_manifest(&f.storage, &f.project, &f.recording).unwrap_err(),
            "RESOURCE_LIMIT"
        );
    }

    #[cfg(windows)]
    #[test]
    fn verified_handle_prevents_write_and_replacement_until_closed() {
        use std::io::Write;
        let f = Fixture::new(true);
        let m = f.manifest();
        let reader = f.open(&m, 0, 1).unwrap();
        assert!(fs::OpenOptions::new()
            .write(true)
            .open(&f.paths[0])
            .is_err());
        assert!(fs::rename(&f.paths[0], f.paths[0].with_extension("moved")).is_err());
        drop(reader);
        let mut writable = fs::OpenOptions::new()
            .append(true)
            .open(&f.paths[0])
            .unwrap();
        writable.write_all(&[0]).unwrap();
        drop(writable);
        assert!(f.open(&m, 0, 1).is_err());
    }

    #[cfg(windows)]
    #[test]
    fn an_existing_writer_prevents_verification() {
        let f = Fixture::new(true);
        let m = f.manifest();
        let writable = fs::OpenOptions::new()
            .write(true)
            .open(&f.paths[0])
            .unwrap();
        assert!(f.open(&m, 0, 1).is_err());
        drop(writable);
        assert!(f.open(&m, 0, 1).is_ok());
    }

    #[test]
    fn a_separate_process_cannot_write_or_replace_an_open_source() {
        use std::process::{Command, Stdio};
        fn probe(path: &Path, locked: bool) {
            let mut child = Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "completed_source_media::tests::source_share_child_probe",
                    "--ignored",
                    "--test-threads=1",
                ])
                .env("FUNG_SOURCE_SHARE_CHILD_PATH", path)
                .env(
                    "FUNG_SOURCE_SHARE_CHILD_LOCKED",
                    if locked { "1" } else { "0" },
                )
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap();
            let deadline = Instant::now() + Duration::from_secs(10);
            loop {
                if child.try_wait().unwrap().is_some() {
                    break;
                }
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    panic!("source file-sharing child exceeded deadline");
                }
                std::thread::sleep(Duration::from_millis(20));
            }
            let output = child.wait_with_output().unwrap();
            assert!(
                output.status.success(),
                "child failed: {} {}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }
        let f = Fixture::new(true);
        let m = f.manifest();
        let reader = f.open(&m, 0, 1).unwrap();
        probe(&f.paths[0], true);
        drop(reader);
        probe(&f.paths[0], false);
        assert!(f.open(&m, 0, 1).is_ok());
    }

    #[test]
    #[ignore = "spawned only by the bounded cross-process file-sharing fixture"]
    fn source_share_child_probe() {
        let path =
            PathBuf::from(std::env::var_os("FUNG_SOURCE_SHARE_CHILD_PATH").expect("fixture path"));
        let locked = std::env::var("FUNG_SOURCE_SHARE_CHILD_LOCKED").unwrap() == "1";
        let write = fs::OpenOptions::new().write(true).open(&path);
        assert_eq!(write.is_err(), locked, "ordinary writable open");
        drop(write);
        let moved = path.with_extension("sharing-probe");
        let rename = fs::rename(&path, &moved);
        if rename.is_ok() {
            fs::rename(&moved, &path).unwrap();
        }
        assert_eq!(rename.is_err(), locked, "ordinary replacement/rename");
    }
}
