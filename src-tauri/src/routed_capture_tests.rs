use super::*;
use genesis_block_native::OpenOptions as GenesisOpenOptions;

struct Fixture {
    _temp: tempfile::TempDir,
    storage: Arc<Storage>,
    project: String,
    root: PathBuf,
}
fn fixture() -> Fixture {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("project");
    fs::create_dir(&root).unwrap();
    let storage = Arc::new(
        Storage::open(GenesisOpenOptions {
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
    ga::commit_rows(&storage,vec![ga::upsert("projects",json!({"id":project,"name":"User project name","storage_path":root,"active_recording_id":null,"created_at":"t","updated_at":"t"}))]).unwrap();
    Fixture {
        _temp: temp,
        storage,
        project,
        root,
    }
}
fn plan() -> CaptureRoutePlan {
    CaptureRoutePlan {
        policy_version: 1,
        routes: vec![routing::CaptureRoute {
            source_id: Uuid::new_v4().to_string(),
            label: "source".into(),
            endpoint_id: "opaque-endpoint".into(),
            application: routing::ApplicationIdentity {
                pid: 1,
                parent_pid: 0,
                creation_filetime: "1".into(),
                image_path: "C:\\owned.exe".into(),
                image_volume_serial: 1,
                image_file_id: "file".into(),
            },
            allowed_children: vec![],
        }],
    }
}
#[test]
fn plan_rejects_nil_duplicate_and_noncanonical_source_uuids() {
    let mut p = plan();
    assert!(validate_plan(&p).is_ok());
    p.routes[0].source_id = Uuid::nil().to_string();
    assert!(validate_plan(&p).is_err());
    let mut p = plan();
    p.routes[0].source_id = "AAAAAAAA-AAAA-4AAA-8AAA-AAAAAAAAAAAA".into();
    assert!(validate_plan(&p).is_err());
    let mut p = plan();
    let mut second = p.routes[0].clone();
    second.endpoint_id = "other".into();
    p.routes.push(second);
    assert!(validate_plan(&p).is_err());
}
#[test]
fn invalid_storage_root_releases_capture_reservation_before_native_admission() {
    let f = fixture();
    set_project_path(&f, Path::new("relative-untrusted-root"));
    let guard = Arc::new(NativeCaptureGuard::default());
    assert!(start(f.storage.clone(), f.project.clone(), plan(), guard.clone()).is_err());
    assert!(!guard.capture_is_active());
    assert!(ga::query(&f.storage, "recordings", &["id"], vec![], 10)
        .unwrap()
        .is_empty());
}
fn set_project_path(f: &Fixture, path: &Path) {
    ga::commit_rows(&f.storage,vec![ga::upsert("projects",json!({"id":f.project,"name":"User project name","storage_path":path,"active_recording_id":null,"created_at":"t","updated_at":"t"}))]).unwrap();
}
#[test]
fn lazy_ledger_project_root_creates_exact_nested_directory_without_probe_residue() {
    let f = fixture();
    let root = f._temp.path().join("lazy").join("nested").join("project");
    set_project_path(&f, &root);
    assert!(!root.parent().unwrap().exists());
    assert_eq!(project_root(&f.storage, &f.project).unwrap(), root);
    assert!(root.is_dir());
    assert_eq!(
        fs::read_dir(&root).unwrap().count(),
        0,
        "write probe must be removed"
    );
    let rows = ga::query(
        &f.storage,
        "projects",
        &["storage_path"],
        vec![ga::eq("projects", "id", json!(f.project))],
        1,
    )
    .unwrap();
    assert_eq!(rows[0]["projects.storage_path"], json!(root));
    assert!(ga::query(&f.storage, "recordings", &["id"], vec![], 10)
        .unwrap()
        .is_empty());
}
#[test]
fn missing_parent_traversal_is_rejected_before_creating_any_component() {
    let f = fixture();
    let missing = f.root.join("must-not-be-created");
    let escaped = f.root.join("escaped");
    let invalid = missing.join("..").join("escaped");
    assert!(checked_create_dir(&invalid).is_err());
    assert!(!missing.exists());
    assert!(!escaped.exists());
    set_project_path(&f, &invalid);
    assert!(project_root(&f.storage, &f.project).is_err());
    assert!(!missing.exists());
    assert!(!escaped.exists());
}
#[cfg(windows)]
#[test]
fn nonordinary_windows_paths_are_rejected_before_any_component_is_created() {
    let f = fixture();
    for path in [
        r"relative\project",
        r"C:drive-relative",
        r"\\server\share\project",
        r"\\?\C:\project",
        r"\\.\C:\project",
    ] {
        assert!(ordinary_local_path(Path::new(path)).is_err(), "{path}");
    }
    let prefix = f.root.join("must-not-be-created");
    for alias in ["..", ".", "trailing.", "trailing ", "NUL", "stream:alias"] {
        let path = prefix.join(alias).join("child");
        assert!(checked_create_dir(&path).is_err(), "{}", path.display());
        assert!(!prefix.exists(), "invalid suffix caused prefix creation");
    }
    assert_eq!(fs::read_dir(&f.root).unwrap().count(), 0);
}
#[cfg(windows)]
fn test_junction(link: &Path, target: &Path) {
    use std::{
        ffi::c_void,
        os::windows::{fs::OpenOptionsExt, io::AsRawHandle},
    };
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn DeviceIoControl(
            device: *mut c_void,
            code: u32,
            input: *const c_void,
            input_size: u32,
            output: *mut c_void,
            output_size: u32,
            returned: *mut u32,
            overlapped: *mut c_void,
        ) -> i32;
    }
    fs::create_dir(link).unwrap();
    let substitute = format!(r"\??\{}", target.display())
        .encode_utf16()
        .collect::<Vec<_>>();
    let print = target.to_str().unwrap().encode_utf16().collect::<Vec<_>>();
    let mut names = substitute.clone();
    names.push(0);
    let print_offset = names.len() * 2;
    names.extend(print.iter().copied());
    names.push(0);
    let data_len = 8 + names.len() * 2;
    let mut buffer = Vec::new();
    buffer.extend(0xA0000003u32.to_le_bytes()); // IO_REPARSE_TAG_MOUNT_POINT
    buffer.extend(u16::try_from(data_len).unwrap().to_le_bytes());
    buffer.extend(0u16.to_le_bytes());
    buffer.extend(0u16.to_le_bytes());
    buffer.extend(u16::try_from(substitute.len() * 2).unwrap().to_le_bytes());
    buffer.extend(u16::try_from(print_offset).unwrap().to_le_bytes());
    buffer.extend(u16::try_from(print.len() * 2).unwrap().to_le_bytes());
    for value in names {
        buffer.extend(value.to_le_bytes());
    }
    let handle = OpenOptions::new()
        .write(true)
        .custom_flags(0x02000000 | 0x00200000)
        .open(link)
        .unwrap();
    let mut returned = 0;
    // Filesystem-only fixture, confined to an owned temporary directory. No
    // shell, administrator privilege, audio device, or external path is used.
    let success = unsafe {
        DeviceIoControl(
            handle.as_raw_handle(),
            0x000900A4,
            buffer.as_ptr().cast(),
            buffer.len() as u32,
            std::ptr::null_mut(),
            0,
            &mut returned,
            std::ptr::null_mut(),
        )
    };
    assert_ne!(
        success,
        0,
        "create owned test junction: {}",
        std::io::Error::last_os_error()
    );
}
#[cfg(windows)]
#[test]
fn lazy_ledger_root_under_reparse_ancestor_is_rejected_before_target_mutation() {
    let f = fixture();
    let outside = f._temp.path().join("outside-project");
    fs::create_dir(&outside).unwrap();
    let junction = f.root.join("junction");
    test_junction(&junction, &outside);
    let requested = junction.join("must-not-be-created").join("project");
    set_project_path(&f, &requested);
    assert!(project_root(&f.storage, &f.project).is_err());
    assert_eq!(fs::read_dir(&outside).unwrap().count(), 0);
    fs::remove_dir(&junction).unwrap();
}
#[test]
fn startup_rows_and_failed_finalization_preserve_project_and_source_marker() {
    let f = fixture();
    let recording = Uuid::new_v4().to_string();
    let job = Uuid::new_v4().to_string();
    assert_eq!(project_root(&f.storage, &f.project).unwrap(), f.root);
    start_rows(&f.storage, &f.project, &recording, &job, &f.root).unwrap();
    assert_eq!(
        ga::capture(&f.storage, &recording).unwrap().source,
        "routed_endpoint_pending"
    );
    recording_terminal(&f.storage, &recording, "partial").unwrap();
    let row = ga::capture(&f.storage, &recording).unwrap();
    assert_eq!(row.status, "partial");
    assert_eq!(row.source, "routed_endpoint_pending");
    assert_eq!(row.safe_offset_ms, 0);
    let project = ga::query(
        &f.storage,
        "projects",
        &["name"],
        vec![ga::eq("projects", "id", json!(f.project))],
        1,
    )
    .unwrap();
    assert_eq!(project[0]["projects.name"], "User project name");
    let jobs = ga::query(
        &f.storage,
        "jobs",
        &["provider_id", "input_refs_json"],
        vec![ga::eq("jobs", "id", json!(job))],
        1,
    )
    .unwrap();
    assert!(jobs[0]["jobs.provider_id"].is_null());
    assert_eq!(jobs[0]["jobs.input_refs_json"], json!([recording]));
}
#[test]
fn duplicate_endpoint_and_parent_alias_are_rejected_without_native_calls() {
    let mut p = plan();
    let mut second = p.routes[0].clone();
    second.source_id = Uuid::new_v4().to_string();
    p.routes.push(second);
    assert!(validate_plan(&p).is_err());
    let f = fixture();
    assert!(local_root(&f.root.join("..").join("project")).is_err());
}
#[test]
fn stop_joins_coordinator_before_releasing_exclusive_capture() {
    let guard = Arc::new(NativeCaptureGuard::default());
    NativeCaptureGuard::reserve_capture(guard.clone())
        .unwrap()
        .commit_active();
    let shared = Arc::new(Shared {
        status: Mutex::new(RoutedCaptureStatus {
            project_id: "p".into(),
            recording_id: "r".into(),
            phase: "recording".into(),
            duration_ms: 0,
            sources: vec![],
            first_failure: None,
            warnings: vec![],
        }),
        first: Mutex::new(state::FailureLedger::default()),
        armed: AtomicBool::new(true),
        cancel_requested: AtomicBool::new(false),
        stop: Arc::new(AtomicBool::new(false)),
        observer: Mutex::new(None),
    });
    let worker_shared = shared.clone();
    let worker_guard = guard.clone();
    let join = thread::spawn(move || {
        let _release = ReleaseGuard {
            guard: worker_guard,
            reservation: None,
        };
        while !worker_shared.stop.load(Ordering::SeqCst) {
            thread::yield_now();
        }
        lock(&worker_shared.status).phase = "partial".into();
    });
    let mut handle = RoutedCaptureHandle {
        shared,
        join: Some(join),
    };
    assert!(guard.capture_is_active());
    assert_eq!(handle.stop_and_join().phase, "partial");
    assert!(!guard.capture_is_active());
    // An old retained handle has no authority over a later capture.
    NativeCaptureGuard::reserve_capture(guard.clone())
        .unwrap()
        .commit_active();
    handle.request_stop();
    assert_eq!(
        guard.capture_phase(),
        crate::recording_review::CapturePhase::Active
    );
    drop(handle);
    assert_eq!(
        guard.capture_phase(),
        crate::recording_review::CapturePhase::Active
    );
    guard.release_capture();
}
#[test]
fn group_failure_is_monotonic_and_source_failure_remains_original() {
    let shared = Shared {
        status: Mutex::new(RoutedCaptureStatus {
            project_id: "p".into(),
            recording_id: "r".into(),
            phase: "recording".into(),
            duration_ms: 0,
            sources: vec![RoutedSourceStatus {
                source_id: "s".into(),
                epoch: 1,
                ready: false,
                native_terminated: false,
                drained: false,
                durable_counts_known: false,
                durable_frames: 0,
                durable_chunks: 0,
                accepted_packets: 0,
                private_orphans: vec![],
                first_failure: None,
            }],
            first_failure: None,
            warnings: vec![],
        }),
        first: Mutex::new(state::FailureLedger::default()),
        armed: AtomicBool::new(false),
        cancel_requested: AtomicBool::new(false),
        stop: Arc::new(AtomicBool::new(false)),
        observer: Mutex::new(None),
    };
    shared.fail(Some("s"), "route owner changed");
    shared.fail(Some("s"), "cleanup failed");
    assert_eq!(
        lock(&shared.status).first_failure.as_deref(),
        Some("route owner changed")
    );
    assert_eq!(
        lock(&shared.status).sources[0].first_failure.as_deref(),
        Some("route owner changed")
    );
    assert!(shared.stop.load(Ordering::SeqCst));
}

#[test]
fn private_pcm_spool_streams_exact_bytes_and_bounded_metadata_without_publication() {
    let temp = tempfile::tempdir().unwrap();
    let scope = Arc::new(
        sink::SourcePacketScope::new(Uuid::new_v4(), 1, "endpoint".into(), RATE, 10000000).unwrap(),
    );
    let samples = (0..9001)
        .map(|value| value as i16 - 4500)
        .collect::<Vec<_>>();
    let packet = EndpointPacket {
        scope,
        frames: sink::FrameSpan {
            start: 0,
            end: samples.len() as u64,
        },
        samples: samples.clone(),
        original_native_frames: sink::FrameSpan {
            start: 0,
            end: samples.len() as u64,
        },
        preroll_clipped_frames: 0,
        missing_before: None,
        known_idle_before: None,
        native_stamp: sink::PacketStamp {
            first_sample: 0,
            silence_before: None,
            native_index: 0,
            native_qpc_hns: 10000000,
            observed_qpc_hns: 10000000,
            native_flags: 0,
        },
    };
    let mut spool = PrivateSpool::new(temp.path()).unwrap();
    spool.append(&packet).unwrap();
    spool.sync().unwrap();
    let bytes = fs::read(&spool.pcm_path).unwrap();
    let actual = bytes
        .chunks_exact(2)
        .map(|bytes| i16::from_le_bytes([bytes[0], bytes[1]]))
        .collect::<Vec<_>>();
    assert_eq!(actual, samples);
    let metadata: serde_json::Value =
        serde_json::from_str(fs::read_to_string(&spool.metadata_path).unwrap().trim()).unwrap();
    assert_eq!(metadata["byteLength"], 18002);
    assert_eq!(metadata["privateOnly"], true);
    spool.frames = MAX_SOURCE_FRAMES;
    assert!(spool.append(&packet).is_err());
    spool.frames = 0;
    assert!(
        spool.append(&packet).is_err(),
        "private spool offsets never reopen after failure"
    );
}

#[test]
fn durable_source_quarantine_uses_runtime_first_cause_instead_of_later_cleanup() {
    let f = fixture();
    let recording = Uuid::new_v4().to_string();
    let job = Uuid::new_v4().to_string();
    start_rows(&f.storage, &f.project, &recording, &job, &f.root).unwrap();
    let reg = CaptureSourceRegistration {
        id: Uuid::new_v4().to_string(),
        epoch: 1,
        kind: "endpoint".into(),
        endpoint_id: Some("endpoint".into()),
        label: "source".into(),
        sample_rate: i64::from(RATE),
        clock_origin_qpc_hns: "1234".into(),
        metadata: json!({}),
    };
    custody::register_capture_sources(
        &f.storage,
        &attempt(&f.storage).unwrap(),
        &f.project,
        &recording,
        std::slice::from_ref(&reg),
    )
    .unwrap();
    let mut group =
        CaptureSourceWriterGroup::new(f.storage.clone(), f.project.clone(), recording).unwrap();
    let mut writer = group.spawn(SourceWriterOptions::new(reg.clone())).unwrap();
    let shared = Shared {
        status: Mutex::new(RoutedCaptureStatus {
            project_id: f.project.clone(),
            recording_id: "unused".into(),
            phase: "recording".into(),
            duration_ms: 0,
            sources: vec![],
            first_failure: None,
            warnings: vec![],
        }),
        first: Mutex::new(state::FailureLedger::default()),
        armed: AtomicBool::new(true),
        cancel_requested: AtomicBool::new(false),
        stop: Arc::new(AtomicBool::new(false)),
        observer: Mutex::new(None),
    };
    shared.fail(Some(&reg.id), "source route became foreign");
    poison(&mut writer, &shared, &reg.id, "later cleanup failed");
    let report = writer.finish();
    assert_eq!(
        report.first_failure.unwrap().detail,
        "source route became foreign"
    );
    assert!(report.quarantine_persisted);
    let rows = ga::query(
        &f.storage,
        "capture_sources",
        &["quarantine_json"],
        vec![ga::eq("capture_sources", "id", json!(reg.id))],
        1,
    )
    .unwrap();
    let reason = rows[0]["capture_sources.quarantine_json"]["reason"]
        .as_str()
        .unwrap();
    assert!(reason.contains("source route became foreign"));
    assert!(!reason.contains("later cleanup"));
    // A successful close transaction is not healthy completion evidence.
    let recording = ga::query(
        &f.storage,
        "capture_sources",
        &["recording_id"],
        vec![ga::eq("capture_sources", "id", json!(reg.id))],
        1,
    )
    .unwrap();
    let recording = recording[0]["capture_sources.recording_id"]
        .as_str()
        .unwrap();
    custody::complete_capture_sources(&f.storage, &attempt(&f.storage).unwrap(), recording)
        .unwrap();
    assert_eq!(
        ga::capture(&f.storage, recording).unwrap().status,
        "partial"
    );
    assert!(custody::require_capture_source_access(&f.storage, recording).is_err());
}

fn recovery_shared(source: &str) -> Arc<Shared> {
    Arc::new(Shared {
        status: Mutex::new(RoutedCaptureStatus {
            project_id: "p".into(),
            recording_id: "r".into(),
            phase: "recording".into(),
            duration_ms: 0,
            sources: vec![RoutedSourceStatus {
                source_id: source.into(),
                epoch: 1,
                ready: true,
                native_terminated: true,
                drained: true,
                durable_counts_known: true,
                durable_frames: 10,
                durable_chunks: 1,
                accepted_packets: 3,
                private_orphans: vec![],
                first_failure: None,
            }],
            first_failure: None,
            warnings: vec![],
        }),
        first: Mutex::new(state::FailureLedger::default()),
        armed: AtomicBool::new(true),
        cancel_requested: AtomicBool::new(false),
        stop: Arc::new(AtomicBool::new(true)),
        observer: Mutex::new(None),
    })
}
fn complete_report() -> SourceWriterReport {
    SourceWriterReport {
        source_id: "s".into(),
        epoch: 1,
        durable_chunks: 1,
        durable_frames: 10,
        first_failure: None,
        quarantine_persisted: false,
        drained: true,
        private_orphans: vec![PathBuf::from("private-tail.pcm")],
        durable_counts_known: true,
        persistence_error: None,
        quarantine_error: None,
    }
}
#[test]
fn final_guard_failure_retains_reports_counts_and_private_tail_paths() {
    let mut reports = vec![];
    let shared = recovery_shared("s");
    let guard = Arc::new(NativeCaptureGuard::default());
    let error = {
        let _lease = ReleaseGuard {
            guard: guard.clone(),
            reservation: Some(NativeCaptureGuard::reserve_capture(guard.clone()).unwrap()),
        };
        let mut workers = Workers {
            shared: shared.clone(),
            joins: vec![thread::spawn(complete_report)],
        };
        let error = join_then_snapshot(&mut workers.joins, &mut reports, &shared, 1, || {
            Err("final route changed".into())
        })
        .unwrap_err();
        assert!(workers.joins.is_empty());
        assert!(
            guard.capture_is_active(),
            "admission remains held through final failure handling"
        );
        error
    };
    assert!(
        !guard.capture_is_active(),
        "cleanup releases admission only after worker joins"
    );
    assert_eq!(error, "final route changed");
    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0].durable_frames, 10);
    assert_eq!(
        reports[0].private_orphans,
        [PathBuf::from("private-tail.pcm")]
    );
}

#[test]
fn final_snapshot_never_runs_when_any_native_termination_is_unproven() {
    let reports = vec![complete_report()];
    let shared = recovery_shared("s");
    lock(&shared.status).sources[0].native_terminated = false;
    let called = std::cell::Cell::new(false);
    assert!(
        final_snapshot(&reports, &lock(&shared.status).sources, 1, || {
            called.set(true);
            Ok(())
        })
        .is_err()
    );
    assert!(!called.get());
    lock(&shared.status).sources[0].native_terminated = true;
    assert!(final_snapshot(&[], &lock(&shared.status).sources, 1, || {
        called.set(true);
        Ok(())
    })
    .is_err());
    assert!(!called.get());
}
fn exercise_packet_panic(stage: &'static str, expect_complete: bool) {
    let f = fixture();
    let recording = Uuid::new_v4().to_string();
    let job = Uuid::new_v4().to_string();
    start_rows(&f.storage, &f.project, &recording, &job, &f.root).unwrap();
    let id = Uuid::new_v4();
    let origin = 10010000u64;
    let reg = CaptureSourceRegistration {
        id: id.to_string(),
        epoch: 1,
        kind: "endpoint".into(),
        endpoint_id: Some("endpoint".into()),
        label: "source".into(),
        sample_rate: i64::from(RATE),
        clock_origin_qpc_hns: origin.to_string(),
        metadata: json!({}),
    };
    custody::register_capture_sources(
        &f.storage,
        &attempt(&f.storage).unwrap(),
        &f.project,
        &recording,
        std::slice::from_ref(&reg),
    )
    .unwrap();
    let mut group =
        CaptureSourceWriterGroup::new(f.storage.clone(), f.project.clone(), recording).unwrap();
    let mut writer = group.spawn(SourceWriterOptions::new(reg.clone())).unwrap();
    let private = f.root.join("panic-evidence");
    fs::create_dir(&private).unwrap();
    let mut spool = PrivateSpool::new(&private).unwrap();
    let (mut producer, mut receiver) = sink::packet_channel(
        sink::SourcePacketScope::new(id, 1, "endpoint".into(), RATE, origin).unwrap(),
        QUEUE_CAPACITY,
    )
    .unwrap();
    producer.bind_native_clock(10000000, RATE).unwrap();
    producer.confirm_native_ready(10001000).unwrap();
    let stamp = |first: u64, observed: u64| sink::PacketStamp {
        first_sample: first,
        silence_before: None,
        native_index: first + 777,
        native_qpc_hns: 10000000 + first * 10000000 / u64::from(RATE),
        observed_qpc_hns: observed,
        native_flags: 0,
    };
    producer
        .push_native(stamp(0, origin - 1), vec![7; 48])
        .unwrap();
    for (index, value) in [11i16, 22, 33].into_iter().enumerate() {
        let first = 48 + index as u64 * 480;
        producer
            .push_native(
                stamp(first, origin + index as u64 * 100000 + 1000),
                vec![value; 480],
            )
            .unwrap();
    }
    producer.finish(Ok(()));
    let shared = recovery_shared(&reg.id);
    let (events_tx, events_rx) = mpsc::sync_channel(2);
    drop(events_tx);
    let native = NativeExit {
        events: events_rx,
        shared: shared.clone(),
    };
    if stage == "healthy_peer" {
        shared.fail(
            Some("unrelated-source-A"),
            "A endpoint contains foreign renderer",
        );
        PACKET_PANIC_STAGE.with(|inject| inject.set(None));
    } else {
        PACKET_PANIC_STAGE.with(|inject| inject.set(Some(stage)));
    }
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        drain_source(
            &reg,
            &mut writer,
            &mut receiver,
            &native,
            &mut spool,
            &shared,
        )
    }));
    assert!(
        result.is_ok(),
        "processing panic must be recovered without losing accepted PCM ownership"
    );
    let drained = result.unwrap();
    assert_eq!(drained.packets, 3);
    assert!(drained.native_terminated);
    assert!(drained.summary.is_some());
    let pcm_path = spool.pcm_path.clone();
    let metadata_path = spool.metadata_path.clone();
    let report = finish_source(&reg, writer, spool, drained, &shared);
    let bytes = fs::read(&pcm_path).unwrap();
    let pcm = bytes
        .chunks_exact(2)
        .map(|b| i16::from_le_bytes([b[0], b[1]]))
        .collect::<Vec<_>>();
    if expect_complete {
        assert_eq!(pcm, [vec![11; 480], vec![22; 480], vec![33; 480]].concat());
        assert_eq!(
            fs::read_to_string(&metadata_path).unwrap().lines().count(),
            3
        );
        assert!(report.durable_counts_known);
    } else {
        assert!(
            !report.durable_counts_known,
            "interrupted private persistence must be explicitly unknown"
        );
        assert!(report
            .persistence_error
            .as_deref()
            .unwrap()
            .contains("incomplete/unknown"));
        assert!(lock(&shared.status)
            .warnings
            .iter()
            .any(|warning| warning.contains("incomplete/unknown")));
        assert_ne!(pcm, [vec![11; 480], vec![22; 480], vec![33; 480]].concat());
    }
    if stage == "healthy_peer" {
        assert!(!report.quarantine_persisted);
        assert!(report.first_failure.is_none());
        assert_eq!(report.durable_frames, 1440);
        assert!(shared.source_failure(&reg.id).is_none());
    } else {
        assert!(report.quarantine_persisted);
        assert!(report.first_failure.is_some());
    }
    assert!(shared.failed());
    assert_eq!(lock(&shared.status).sources[0].accepted_packets, 3);
}
#[test]
fn packet_processing_panic_preserves_inflight_and_every_accepted_queued_packet() {
    exercise_packet_panic("before_private_spool", true);
}
#[test]
fn panic_after_durable_private_spool_does_not_duplicate_inflight_pcm() {
    exercise_packet_panic("after_private_spool", true);
}
#[test]
fn interrupted_private_spool_remains_incomplete_unknown_after_queue_drain() {
    exercise_packet_panic("during_private_spool", false);
}
#[test]
fn healthy_peer_drains_every_accepted_packet_authoritatively_after_another_source_fails() {
    exercise_packet_panic("healthy_peer", true);
}

#[test]
fn concurrent_delayed_failure_publication_always_exports_canonical_first_reason() {
    let shared = recovery_shared("A");
    {
        let mut status = lock(&shared.status);
        let mut b = status.sources[0].clone();
        b.source_id = "B".into();
        status.sources.push(b);
    }
    let (recorded_tx, recorded_rx) = mpsc::channel();
    let (published_tx, published_rx) = mpsc::channel();
    let a_shared = shared.clone();
    let a = thread::spawn(move || {
        let publication = a_shared.record_failure(Some("A"), "canonical A");
        recorded_tx.send(()).unwrap();
        published_rx.recv().unwrap();
        a_shared.publish_failure(publication);
        a_shared.cancel();
    });
    recorded_rx.recv().unwrap();
    let b_shared = shared.clone();
    let b = thread::spawn(move || {
        b_shared.fail(Some("B"), "later B");
        assert_eq!(
            lock(&b_shared.status).first_failure.as_deref(),
            Some("canonical A")
        );
        published_tx.send(()).unwrap();
    });
    a.join().unwrap();
    b.join().unwrap();
    assert_eq!(lock(&shared.first).get(), Some("canonical A"));
    assert_eq!(
        lock(&shared.status).first_failure.as_deref(),
        Some("canonical A")
    );
    assert_eq!(shared.source_failure("A").as_deref(), Some("canonical A"));
    assert_eq!(shared.source_failure("B").as_deref(), Some("later B"));
    assert_eq!(
        lock(&shared.status).sources[1].first_failure.as_deref(),
        Some("later B")
    );
    assert_eq!(shared.source_failure("healthy"), None);
    shared.fail(None, "explicit global uncertainty");
    assert_eq!(
        shared.source_failure("healthy").as_deref(),
        Some("explicit global uncertainty")
    );
    assert_eq!(
        lock(&shared.status).first_failure.as_deref(),
        Some("canonical A")
    );
}
fn completed_transport_fixture(reg: &CaptureSourceRegistration) -> SourceDrainState {
    let origin = reg.clock_origin_qpc_hns.parse::<u64>().unwrap();
    let (mut producer, mut receiver) = sink::packet_channel(
        sink::SourcePacketScope::new(
            Uuid::parse_str(&reg.id).unwrap(),
            1,
            reg.endpoint_id.clone().unwrap(),
            RATE,
            origin,
        )
        .unwrap(),
        2,
    )
    .unwrap();
    producer.bind_native_clock(origin - 10000, RATE).unwrap();
    producer.confirm_native_ready(origin - 9000).unwrap();
    producer
        .push_native(
            sink::PacketStamp {
                first_sample: 0,
                silence_before: None,
                native_index: 0,
                native_qpc_hns: origin - 10000,
                observed_qpc_hns: origin - 1,
                native_flags: 0,
            },
            vec![7; 48],
        )
        .unwrap();
    producer
        .push_native(
            sink::PacketStamp {
                first_sample: 48,
                silence_before: None,
                native_index: 48,
                native_qpc_hns: origin,
                observed_qpc_hns: origin + 1000,
                native_flags: 0,
            },
            vec![9],
        )
        .unwrap();
    producer.finish(Ok(()));
    assert!(matches!(
        receiver.recv_timeout(Duration::from_millis(100)).unwrap(),
        SinkRead::Packet(_)
    ));
    let SinkRead::Drained(summary) = receiver.recv_timeout(Duration::from_millis(100)).unwrap()
    else {
        panic!("pure transport fixture did not drain");
    };
    SourceDrainState {
        packets: 1,
        summary: Some(summary),
        native_terminated: true,
    }
}
fn exercise_source_custody_scope(global: bool) {
    use sha2::{Digest, Sha256};
    let f = fixture();
    let recording = Uuid::new_v4().to_string();
    let job = Uuid::new_v4().to_string();
    start_rows(&f.storage, &f.project, &recording, &job, &f.root).unwrap();
    let regs = (0..2)
        .map(|index| CaptureSourceRegistration {
            id: Uuid::new_v4().to_string(),
            epoch: 1,
            kind: "endpoint".into(),
            endpoint_id: Some(format!("endpoint-{index}")),
            label: format!("source-{index}"),
            sample_rate: i64::from(RATE),
            clock_origin_qpc_hns: "10010000".into(),
            metadata: json!({"fixture":"source-custody-only"}),
        })
        .collect::<Vec<_>>();
    custody::register_capture_sources(
        &f.storage,
        &attempt(&f.storage).unwrap(),
        &f.project,
        &recording,
        &regs,
    )
    .unwrap();
    let mut group =
        CaptureSourceWriterGroup::new(f.storage.clone(), f.project.clone(), recording.clone())
            .unwrap();
    let mut a = group
        .spawn(SourceWriterOptions::new(regs[0].clone()))
        .unwrap();
    let mut b = group
        .spawn(SourceWriterOptions::new(regs[1].clone()))
        .unwrap();
    a.try_push(SourcePcmPacket {
        source_id: regs[0].id.clone(),
        epoch: 1,
        first_frame: 0,
        pcm: vec![111; 96000],
    })
    .unwrap();
    b.try_push(SourcePcmPacket {
        source_id: regs[1].id.clone(),
        epoch: 1,
        first_frame: 0,
        pcm: vec![222; 96000],
    })
    .unwrap();
    let a_report = a.finish();
    assert_eq!(a_report.durable_frames, 96000);
    let deadline = Instant::now() + Duration::from_secs(5);
    let b_chunk = loop {
        let rows = ga::query(
            &f.storage,
            "capture_source_chunks",
            &["chunk_json"],
            vec![ga::eq(
                "capture_source_chunks",
                "source_id",
                json!(regs[1].id),
            )],
            1,
        )
        .unwrap();
        if let Some(row) = rows.first() {
            break serde_json::from_value::<custody::CaptureSourceChunk>(
                row["capture_source_chunks.chunk_json"].clone(),
            )
            .unwrap();
        }
        assert!(
            Instant::now() < deadline,
            "first valid B PCM chunk was not persisted"
        );
        thread::sleep(Duration::from_millis(5));
    };
    let before_hash = format!("{:x}", Sha256::digest(fs::read(&b_chunk.path).unwrap()));
    let shared = recovery_shared(&regs[0].id);
    {
        let mut status = lock(&shared.status);
        let mut peer = status.sources[0].clone();
        peer.source_id = regs[1].id.clone();
        status.sources.push(peer);
    }
    if global {
        shared.fail(None, "global provenance uncertainty");
    } else {
        shared.fail(Some(&regs[0].id), "A endpoint contains foreign renderer");
    }
    assert!(shared.stop.load(Ordering::SeqCst));
    if !global {
        assert!(shared.source_failure(&regs[1].id).is_none());
    }
    // This packet was already accepted from B before the group stop. B keeps
    // its own custody admission while A is quarantined.
    b.try_push(SourcePcmPacket {
        source_id: regs[1].id.clone(),
        epoch: 1,
        first_frame: 96000,
        pcm: vec![333; 480],
    })
    .unwrap();
    let private = f.root.join("healthy-peer-private");
    fs::create_dir(&private).unwrap();
    let b_report = finish_source(
        &regs[1],
        b,
        PrivateSpool::new(&private).unwrap(),
        completed_transport_fixture(&regs[1]),
        &shared,
    );
    if global {
        assert!(b_report.first_failure.is_some());
    } else {
        assert!(b_report.first_failure.is_none());
        assert_eq!(b_report.durable_frames, 96480);
    }
    let sources = regs.iter().map(|reg| reg.id.clone()).collect::<Vec<_>>();
    close_registered_sources(&f.storage, &f.project, &recording, &sources, &shared);
    let rows = ga::query(
        &f.storage,
        "capture_sources",
        &[
            "id",
            "state",
            "closed_at",
            "registration_json",
            "quarantine_json",
        ],
        vec![ga::eq("capture_sources", "recording_id", json!(recording))],
        10,
    )
    .unwrap();
    let a_row = rows
        .iter()
        .find(|row| row["capture_sources.id"] == regs[0].id)
        .unwrap();
    let b_row = rows
        .iter()
        .find(|row| row["capture_sources.id"] == regs[1].id)
        .unwrap();
    assert_eq!(a_row["capture_sources.state"], "quarantined");
    assert_eq!(
        b_row["capture_sources.state"],
        if global { "quarantined" } else { "completed" }
    );
    assert!(!a_row["capture_sources.closed_at"].is_null());
    assert!(!b_row["capture_sources.closed_at"].is_null());
    assert_eq!(b_row["capture_sources.registration_json"], json!(regs[1]));
    if !global {
        assert!(b_row["capture_sources.quarantine_json"].is_null());
        assert!(shared.global_failure().is_none());
    }
    assert_eq!(
        before_hash,
        format!("{:x}", Sha256::digest(fs::read(&b_chunk.path).unwrap()))
    );
    assert_eq!(
        ga::capture(&f.storage, &recording).unwrap().status,
        "partial"
    );
    assert!(custody::require_capture_source_access(&f.storage, &recording).is_err());
}
#[test]
fn one_source_failure_quarantines_only_its_epoch_and_completes_healthy_peer() {
    exercise_source_custody_scope(false);
}
#[test]
fn explicit_global_provenance_failure_quarantines_all_source_epochs() {
    exercise_source_custody_scope(true);
}

fn runtime_cancellation_race(
    code: routing::Rejection,
) -> (Arc<Shared>, String, Result<(), String>) {
    let p = plan();
    let b = p.routes[0].source_id.clone();
    let shared = recovery_shared(&b);
    shared.stop.store(false, Ordering::SeqCst);
    let a = Uuid::new_v4().to_string();
    let (started_tx, started_rx) = mpsc::channel();
    let (cancelled_tx, cancelled_rx) = mpsc::channel();
    let worker_shared = shared.clone();
    let worker_a = a.clone();
    let worker = thread::spawn(move || {
        started_rx.recv().unwrap();
        worker_shared.fail(Some(&worker_a), "A foreign renderer observed");
        cancelled_tx.send(()).unwrap();
    });
    let result = validate_runtime_decisions(
        &p,
        &shared,
        || true,
        |source| {
            assert_eq!(source, b);
            started_tx.send(()).unwrap();
            cancelled_rx.recv().unwrap();
            Err(routing::RouteError {
                code: code.clone(),
                message: "controlled observer decision".into(),
            })
        },
    );
    worker.join().unwrap();
    (shared, b, result)
}
#[test]
fn runtime_observer_revocation_caused_by_peer_cancellation_does_not_fault_healthy_peer() {
    let (shared, b, result) = runtime_cancellation_race(routing::Rejection::InventoryError);
    assert!(
        result.is_ok(),
        "known cancellation must discard induced observer revocation"
    );
    assert!(shared.source_failure(&b).is_none());
    assert_eq!(
        lock(&shared.status).first_failure.as_deref(),
        Some("A foreign renderer observed")
    );
    assert!(shared.stop.load(Ordering::SeqCst));
    assert!(shared.global_failure().is_none());
}
#[test]
fn runtime_independent_policy_rejection_is_retained_despite_peer_cancellation() {
    let (shared, b, result) = runtime_cancellation_race(routing::Rejection::UnknownSession);
    assert!(result.is_err());
    assert!(shared
        .source_failure(&b)
        .unwrap()
        .contains("controlled observer decision"));
    assert_eq!(
        lock(&shared.status).first_failure.as_deref(),
        Some("A foreign renderer observed")
    );
}

#[test]
fn cancellation_intent_blocks_observer_reads_before_native_stop_flag_is_set() {
    let p = plan();
    let shared = recovery_shared(&p.routes[0].source_id);
    shared.stop.store(false, Ordering::SeqCst);
    shared.cancel_requested.store(true, Ordering::SeqCst);
    assert!(validate_runtime_decisions(
        &p,
        &shared,
        || panic!("observer readiness read after cancellation intent"),
        |_| panic!("source read after cancellation intent")
    )
    .is_ok());
    assert!(!shared.stop.load(Ordering::SeqCst));
    assert!(!shared.failed());
}
#[test]
fn group_cancellation_during_readiness_does_not_create_global_uncertainty() {
    let p = plan();
    let shared = recovery_shared(&p.routes[0].source_id);
    shared.stop.store(false, Ordering::SeqCst);
    let a = Uuid::new_v4().to_string();
    assert!(validate_runtime_decisions(
        &p,
        &shared,
        || {
            shared.fail(Some(&a), "A native source failed");
            false
        },
        |_| panic!("revoked source read")
    )
    .is_ok());
    assert!(shared.global_failure().is_none());
    assert!(shared.source_failure(&p.routes[0].source_id).is_none());
}
fn exercise_post_runtime_finalization(reject_b: bool, global_error: bool) {
    let mut p = plan();
    let a = p.routes[0].source_id.clone();
    let mut b_route = p.routes[0].clone();
    b_route.source_id = Uuid::new_v4().to_string();
    b_route.endpoint_id = "second-endpoint".into();
    let b = b_route.source_id.clone();
    p.routes.push(b_route);
    let shared = recovery_shared(&a);
    {
        let mut status = lock(&shared.status);
        let mut peer = status.sources[0].clone();
        peer.source_id = b.clone();
        status.sources.push(peer);
    }
    shared.fail(Some(&a), "A writer-local failure");
    // These synthetic joins exercise coordinator control only, not native proof.
    let joined = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let mut joins = p
        .routes
        .iter()
        .map(|route| {
            let mut report = complete_report();
            report.source_id = route.source_id.clone();
            let joined = joined.clone();
            thread::spawn(move || {
                joined.fetch_add(1, Ordering::SeqCst);
                report
            })
        })
        .collect::<Vec<_>>();
    let mut reports = vec![];
    let mut checked = vec![];
    let execution = if global_error {
        Err(ExecutionFailure::Global(
            "late global clock uncertainty".into(),
        ))
    } else {
        Err(ExecutionFailure::Recorded)
    };
    let passed = drain_and_finalize(
        execution,
        &mut joins,
        &mut reports,
        &p,
        &shared,
        |selected| {
            assert_eq!(
                joined.load(Ordering::SeqCst),
                2,
                "final native checks precede no owned join"
            );
            assert_eq!(selected.routes.len(), 1);
            checked.push(selected.routes[0].source_id.clone());
            if reject_b {
                Err("B final foreign renderer".into())
            } else {
                Ok(())
            }
        },
    );
    assert!(joins.is_empty());
    assert_eq!(reports.len(), 2);
    assert_eq!(
        lock(&shared.status).first_failure.as_deref(),
        Some("A writer-local failure")
    );
    if global_error {
        assert!(checked.is_empty());
        assert_eq!(
            shared.source_failure(&b).as_deref(),
            Some("late global clock uncertainty")
        );
        assert_eq!(
            shared.global_failure().as_deref(),
            Some("late global clock uncertainty")
        );
    } else {
        assert_eq!(checked.as_slice(), std::slice::from_ref(&b));
        assert_eq!(passed, !reject_b);
        if reject_b {
            assert!(shared
                .source_failure(&b)
                .unwrap()
                .contains("B final foreign renderer"));
        } else {
            assert!(shared.source_failure(&b).is_none());
        }
        assert!(shared.global_failure().is_none());
    }
}
#[test]
fn runtime_error_still_requires_fresh_final_admission_for_healthy_peer_after_all_joins() {
    exercise_post_runtime_finalization(false, false);
}
#[test]
fn late_final_peer_rejection_is_scoped_and_not_masked_by_earlier_writer_failure() {
    exercise_post_runtime_finalization(true, false);
}
#[test]
fn later_global_execution_error_is_recorded_despite_prior_source_cause() {
    exercise_post_runtime_finalization(false, true);
}
