//! Windows-only PDF parser process boundary. The child runs in an AppContainer
//! with zero capabilities and a kill-on-close Job Object; source PDF bytes are
//! delivered only through its bounded stdin pipe.

use super::{
    read_bounded_and_drain, MAX_PARSER_OUTPUT_BYTES, MAX_PARSER_STDERR_BYTES, PARSER_TIMEOUT,
};
use std::{
    ffi::{c_void, OsStr, OsString},
    fs,
    io::Write,
    os::windows::{
        ffi::{OsStrExt, OsStringExt},
        io::{FromRawHandle, RawHandle},
    },
    path::{Path, PathBuf},
    ptr::{null, null_mut},
    thread,
    time::Instant,
};
use windows_sys::Win32::{
    Foundation::{CloseHandle, LocalFree, HANDLE, HLOCAL, WAIT_OBJECT_0, WAIT_TIMEOUT},
    Security::{
        Authorization::{
            ConvertSidToStringSidW, GetNamedSecurityInfoW, SetEntriesInAclW, SetNamedSecurityInfoW,
            EXPLICIT_ACCESS_W, GRANT_ACCESS, NO_MULTIPLE_TRUSTEE, SE_FILE_OBJECT, TRUSTEE_IS_SID,
            TRUSTEE_IS_UNKNOWN, TRUSTEE_W,
        },
        Isolation::{
            CreateAppContainerProfile, DeriveAppContainerSidFromAppContainerName,
            GetAppContainerFolderPath,
        },
    },
    Security::{FreeSid, DACL_SECURITY_INFORMATION, SECURITY_ATTRIBUTES, SECURITY_CAPABILITIES},
    Storage::FileSystem::{FILE_GENERIC_EXECUTE, FILE_GENERIC_READ, FILE_GENERIC_WRITE},
    System::{
        Com::CoTaskMemFree,
        JobObjects::{
            AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
            SetInformationJobObject, TerminateJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
            JOB_OBJECT_LIMIT_ACTIVE_PROCESS, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
            JOB_OBJECT_LIMIT_PROCESS_MEMORY, JOB_OBJECT_LIMIT_PROCESS_TIME,
        },
        Pipes::CreatePipe,
        Threading::{
            CreateProcessW, DeleteProcThreadAttributeList, GetExitCodeProcess,
            InitializeProcThreadAttributeList, ResumeThread, TerminateProcess,
            UpdateProcThreadAttribute, WaitForSingleObject, CREATE_NO_WINDOW, CREATE_SUSPENDED,
            CREATE_UNICODE_ENVIRONMENT, EXTENDED_STARTUPINFO_PRESENT, PROCESS_INFORMATION,
            PROC_THREAD_ATTRIBUTE_HANDLE_LIST, PROC_THREAD_ATTRIBUTE_SECURITY_CAPABILITIES,
            STARTF_USESTDHANDLES, STARTUPINFOEXW,
        },
    },
};

const APP_CONTAINER_NAME: &str = "dev.fung.local.knowledge-parser.v1";
const MAX_INPUT_BYTES: usize = 25 * 1024 * 1024;
const MAX_PARSER_MEMORY: usize = 512 * 1024 * 1024;
const MAX_PARSER_CPU_100NS: i64 = 55 * 10_000_000;
const PARSER_EXIT_CODE: u32 = 0xE000_1001;

#[cfg(test)]
fn log_sandbox_probe_failure(stage: &str) {
    let error = unsafe { windows_sys::Win32::Foundation::GetLastError() };
    eprintln!("AppContainer parser probe failed at {stage} (Win32 error {error})");
}

fn sandbox_handle(handle: HANDLE, _stage: &str) -> Result<Handle, String> {
    Handle::new(handle).inspect_err(|_| {
        #[cfg(test)]
        log_sandbox_probe_failure(_stage);
    })
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeManifest {
    contract_version: u32,
    python_version: String,
    pypdf_version: String,
    pypdf_license: String,
}

struct Handle(HANDLE);

impl Handle {
    fn new(handle: HANDLE) -> Result<Self, String> {
        if handle.is_null() || handle as isize == -1 {
            Err("PDF_PARSER_SANDBOX_UNAVAILABLE".to_string())
        } else {
            Ok(Self(handle))
        }
    }

    fn raw(&self) -> HANDLE {
        self.0
    }

    fn into_file(mut self) -> fs::File {
        let raw = self.0 as RawHandle;
        self.0 = null_mut();
        unsafe { fs::File::from_raw_handle(raw) }
    }
}

impl Drop for Handle {
    fn drop(&mut self) {
        if !self.0.is_null() && self.0 as isize != -1 {
            unsafe { CloseHandle(self.0) };
        }
    }
}

struct LocalMemory(*mut c_void);

impl Drop for LocalMemory {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { LocalFree(self.0 as HLOCAL) };
        }
    }
}

struct CoTaskMemory(*mut c_void);

impl Drop for CoTaskMemory {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { CoTaskMemFree(self.0) };
        }
    }
}

struct AppContainerSid(*mut c_void);

impl Drop for AppContainerSid {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { FreeSid(self.0) };
        }
    }
}

struct AttributeList(Vec<usize>);

impl AttributeList {
    fn create() -> Result<Self, String> {
        let mut bytes = 0usize;
        unsafe {
            InitializeProcThreadAttributeList(null_mut(), 2, 0, &mut bytes);
        }
        if bytes == 0 {
            return Err("PDF_PARSER_SANDBOX_UNAVAILABLE".to_string());
        }
        let mut storage = vec![0usize; bytes.div_ceil(std::mem::size_of::<usize>())];
        let list = storage.as_mut_ptr().cast();
        if unsafe { InitializeProcThreadAttributeList(list, 2, 0, &mut bytes) } == 0 {
            return Err("PDF_PARSER_SANDBOX_UNAVAILABLE".to_string());
        }
        Ok(Self(storage))
    }

    fn as_mut_ptr(
        &mut self,
    ) -> windows_sys::Win32::System::Threading::LPPROC_THREAD_ATTRIBUTE_LIST {
        self.0.as_mut_ptr().cast()
    }
}

impl Drop for AttributeList {
    fn drop(&mut self) {
        unsafe { DeleteProcThreadAttributeList(self.0.as_mut_ptr().cast()) };
    }
}

/// Returns `(process_succeeded, stdout, stdout_overflow)`.
pub(super) fn run(
    bundled_runtime: &Path,
    selected_pdf: &[u8],
) -> Result<(bool, Vec<u8>, bool), String> {
    if selected_pdf.is_empty() || selected_pdf.len() > MAX_INPUT_BYTES {
        return Err("selected document exceeds the 25 MiB import bound".to_string());
    }
    let bundled_runtime = bundled_runtime
        .canonicalize()
        .map_err(|_| "PDF_PARSER_SANDBOX_UNAVAILABLE".to_string())?;
    validate_runtime(&bundled_runtime)?;

    let sid = app_container_sid()?;
    let stage = stage_runtime(&bundled_runtime, sid.0)?;
    let result = run_child(&stage.0, sid.0, selected_pdf);
    drop(stage);
    result
}

fn validate_runtime(root: &Path) -> Result<(), String> {
    let manifest_bytes = fs::read(root.join("parser-runtime.manifest.json"))
        .map_err(|_| "PDF_PARSER_SANDBOX_UNAVAILABLE".to_string())?;
    let manifest: RuntimeManifest = serde_json::from_slice(&manifest_bytes)
        .map_err(|_| "PDF_PARSER_SANDBOX_UNAVAILABLE".to_string())?;
    if manifest.contract_version != 1
        || manifest.python_version != "3.11.9"
        || manifest.pypdf_version != "6.10.0"
        || manifest.pypdf_license != "BSD-3-Clause"
        || !root.join("python.exe").is_file()
        || !root.join("python311.dll").is_file()
        || !root.join("python311.zip").is_file()
        || !root.join("python311._pth").is_file()
        || !root.join("extract_knowledge.py").is_file()
        || !root.join("Lib/site-packages/pypdf").is_dir()
    {
        return Err("PDF_PARSER_SANDBOX_UNAVAILABLE".to_string());
    }
    Ok(())
}

fn app_container_sid() -> Result<AppContainerSid, String> {
    let name = wide(APP_CONTAINER_NAME);
    let mut sid = null_mut();
    let status = unsafe {
        CreateAppContainerProfile(
            name.as_ptr(),
            name.as_ptr(),
            name.as_ptr(),
            null(),
            0,
            &mut sid,
        )
    };
    if status < 0 {
        if status as u32 != 0x8007_00B7 {
            #[cfg(test)]
            eprintln!(
                "CreateAppContainerProfile failed: HRESULT 0x{:08X}",
                status as u32
            );
            return Err("PDF_PARSER_SANDBOX_UNAVAILABLE".to_string());
        }
        let status = unsafe { DeriveAppContainerSidFromAppContainerName(name.as_ptr(), &mut sid) };
        if status < 0 {
            return Err("PDF_PARSER_SANDBOX_UNAVAILABLE".to_string());
        }
    }
    if sid.is_null() {
        return Err("PDF_PARSER_SANDBOX_UNAVAILABLE".to_string());
    }
    Ok(AppContainerSid(sid))
}

struct StagedRuntime(PathBuf);

impl Drop for StagedRuntime {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn stage_runtime(source: &Path, sid: *mut c_void) -> Result<StagedRuntime, String> {
    let parent = app_container_local_data_path(sid).inspect_err(|_error| {
        #[cfg(test)]
        eprintln!("AppContainer LocalAppData lookup failed: {_error}");
    })?;
    fs::create_dir_all(&parent).map_err(|_error| {
        #[cfg(test)]
        eprintln!("AppContainer LocalAppData preparation failed: {_error}");
        "PDF_PARSER_SANDBOX_UNAVAILABLE".to_string()
    })?;
    let destination = parent.join(format!(
        "FUNG-Knowledge-Parser-{}-{}",
        std::process::id(),
        uuid::Uuid::new_v4()
    ));
    fs::create_dir(&destination).map_err(|_error| {
        #[cfg(test)]
        eprintln!("AppContainer runtime directory creation failed: {_error}");
        "PDF_PARSER_SANDBOX_UNAVAILABLE".to_string()
    })?;
    let stage = StagedRuntime(destination.clone());
    copy_tree(source, &destination).inspect_err(|_error| {
        #[cfg(test)]
        eprintln!("AppContainer runtime staging copy failed: {_error}");
    })?;
    let temporary = destination.join("Temp");
    fs::create_dir(&temporary).map_err(|_error| {
        #[cfg(test)]
        eprintln!("AppContainer parser temp directory creation failed: {_error}");
        "PDF_PARSER_SANDBOX_UNAVAILABLE".to_string()
    })?;
    let local_app_data = destination.join("LocalAppData");
    fs::create_dir(&local_app_data).map_err(|_error| {
        #[cfg(test)]
        eprintln!("AppContainer LocalAppData directory creation failed: {_error}");
        "PDF_PARSER_SANDBOX_UNAVAILABLE".to_string()
    })?;
    grant_runtime_access(&destination, sid).inspect_err(|_error| {
        #[cfg(test)]
        eprintln!("AppContainer runtime ACL setup failed: {_error}");
    })?;
    grant_path_access(
        &temporary,
        sid,
        FILE_GENERIC_READ | FILE_GENERIC_WRITE | FILE_GENERIC_EXECUTE,
    )?;
    grant_path_access(
        &local_app_data,
        sid,
        FILE_GENERIC_READ | FILE_GENERIC_WRITE | FILE_GENERIC_EXECUTE,
    )?;
    Ok(stage)
}

fn app_container_local_data_path(sid: *mut c_void) -> Result<PathBuf, String> {
    let mut sid_string = null_mut();
    if unsafe { ConvertSidToStringSidW(sid, &mut sid_string) } == 0 || sid_string.is_null() {
        return Err("PDF_PARSER_SANDBOX_UNAVAILABLE".to_string());
    }
    let sid_string_memory = LocalMemory(sid_string.cast());
    let mut folder_path = null_mut();
    let status = unsafe { GetAppContainerFolderPath(sid_string, &mut folder_path) };
    if status < 0 || folder_path.is_null() {
        #[cfg(test)]
        eprintln!(
            "GetAppContainerFolderPath failed: HRESULT 0x{:08X}; SID {}",
            status as u32,
            OsString::from_wide(unsafe {
                let length = (0..128usize)
                    .find(|index| *sid_string.add(*index) == 0)
                    .unwrap_or(0);
                std::slice::from_raw_parts(sid_string, length)
            })
            .to_string_lossy()
        );
        return Err("PDF_PARSER_SANDBOX_UNAVAILABLE".to_string());
    }
    let folder_path_memory = CoTaskMemory(folder_path.cast());
    let path_length = unsafe {
        (0..32_768usize)
            .find(|index| *folder_path.add(*index) == 0)
            .ok_or_else(|| "PDF_PARSER_SANDBOX_UNAVAILABLE".to_string())?
    };
    let path = PathBuf::from(OsString::from_wide(unsafe {
        std::slice::from_raw_parts(folder_path, path_length)
    }));
    drop(folder_path_memory);
    drop(sid_string_memory);
    if !path.is_absolute() {
        return Err("PDF_PARSER_SANDBOX_UNAVAILABLE".to_string());
    }
    Ok(path)
}

fn copy_tree(source: &Path, destination: &Path) -> Result<(), String> {
    for entry in fs::read_dir(source).map_err(|_| "PDF_PARSER_SANDBOX_UNAVAILABLE".to_string())? {
        let entry = entry.map_err(|_| "PDF_PARSER_SANDBOX_UNAVAILABLE".to_string())?;
        let metadata = fs::symlink_metadata(entry.path())
            .map_err(|_| "PDF_PARSER_SANDBOX_UNAVAILABLE".to_string())?;
        if metadata.file_type().is_symlink() {
            return Err("PDF_PARSER_SANDBOX_UNAVAILABLE".to_string());
        }
        let target = destination.join(entry.file_name());
        if metadata.is_dir() {
            fs::create_dir(&target).map_err(|_| "PDF_PARSER_SANDBOX_UNAVAILABLE".to_string())?;
            copy_tree(&entry.path(), &target)?;
        } else if metadata.is_file() {
            fs::copy(entry.path(), target)
                .map_err(|_| "PDF_PARSER_SANDBOX_UNAVAILABLE".to_string())?;
        } else {
            return Err("PDF_PARSER_SANDBOX_UNAVAILABLE".to_string());
        }
    }
    Ok(())
}

fn grant_runtime_access(root: &Path, sid: *mut c_void) -> Result<(), String> {
    grant_path_access(root, sid, FILE_GENERIC_READ | FILE_GENERIC_EXECUTE)?;
    for entry in fs::read_dir(root).map_err(|_| "PDF_PARSER_SANDBOX_UNAVAILABLE".to_string())? {
        let entry = entry.map_err(|_| "PDF_PARSER_SANDBOX_UNAVAILABLE".to_string())?;
        let metadata = fs::symlink_metadata(entry.path())
            .map_err(|_| "PDF_PARSER_SANDBOX_UNAVAILABLE".to_string())?;
        if metadata.is_dir() {
            grant_runtime_access(&entry.path(), sid)?;
        } else if metadata.is_file() {
            grant_path_access(&entry.path(), sid, FILE_GENERIC_READ | FILE_GENERIC_EXECUTE)?;
        } else {
            return Err("PDF_PARSER_SANDBOX_UNAVAILABLE".to_string());
        }
    }
    Ok(())
}

fn grant_path_access(path: &Path, sid: *mut c_void, permissions: u32) -> Result<(), String> {
    let path = wide_os(path.as_os_str());
    let mut old_dacl = null_mut();
    let mut descriptor = null_mut();
    let query_status = unsafe {
        GetNamedSecurityInfoW(
            path.as_ptr(),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION,
            null_mut(),
            null_mut(),
            &mut old_dacl,
            null_mut(),
            &mut descriptor,
        )
    };
    if query_status != 0 {
        return Err("PDF_PARSER_SANDBOX_UNAVAILABLE".to_string());
    }
    let descriptor_memory = LocalMemory(descriptor);
    let trustee = TRUSTEE_W {
        pMultipleTrustee: null_mut(),
        MultipleTrusteeOperation: NO_MULTIPLE_TRUSTEE,
        TrusteeForm: TRUSTEE_IS_SID,
        TrusteeType: TRUSTEE_IS_UNKNOWN,
        ptstrName: sid.cast(),
    };
    let access = EXPLICIT_ACCESS_W {
        grfAccessPermissions: permissions,
        grfAccessMode: GRANT_ACCESS,
        grfInheritance: 0,
        Trustee: trustee,
    };
    let mut new_dacl = null_mut();
    let acl_status = unsafe { SetEntriesInAclW(1, &access, old_dacl, &mut new_dacl) };
    if acl_status != 0 || new_dacl.is_null() {
        drop(descriptor_memory);
        return Err("PDF_PARSER_SANDBOX_UNAVAILABLE".to_string());
    }
    let new_dacl_memory = LocalMemory(new_dacl.cast());
    let set_status = unsafe {
        SetNamedSecurityInfoW(
            path.as_ptr(),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION,
            null_mut(),
            null_mut(),
            new_dacl,
            null_mut(),
        )
    };
    drop(new_dacl_memory);
    drop(descriptor_memory);
    if set_status != 0 {
        return Err("PDF_PARSER_SANDBOX_UNAVAILABLE".to_string());
    }
    Ok(())
}

fn run_child(
    runtime: &Path,
    app_container_sid: *mut c_void,
    selected_pdf: &[u8],
) -> Result<(bool, Vec<u8>, bool), String> {
    let python = runtime.join("python.exe");
    let script = runtime.join("extract_knowledge.py");
    let mut stdin_read = null_mut();
    let mut stdin_write = null_mut();
    let mut stdout_read = null_mut();
    let mut stdout_write = null_mut();
    let mut stderr_read = null_mut();
    let mut stderr_write = null_mut();
    let pipe_security = SECURITY_ATTRIBUTES {
        nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: null_mut(),
        bInheritHandle: 1,
    };
    unsafe {
        if CreatePipe(&mut stdin_read, &mut stdin_write, &pipe_security, 0) == 0
            || CreatePipe(&mut stdout_read, &mut stdout_write, &pipe_security, 0) == 0
            || CreatePipe(&mut stderr_read, &mut stderr_write, &pipe_security, 0) == 0
        {
            for handle in [
                stdin_read,
                stdin_write,
                stdout_read,
                stdout_write,
                stderr_read,
                stderr_write,
            ] {
                if !handle.is_null() {
                    CloseHandle(handle);
                }
            }
            #[cfg(test)]
            log_sandbox_probe_failure("CreatePipe");
            return Err("PDF_PARSER_SANDBOX_UNAVAILABLE".to_string());
        }
    }
    let stdin_read = sandbox_handle(stdin_read, "stdin pipe handle")?;
    let stdin_write = sandbox_handle(stdin_write, "stdin writer handle")?;
    let stdout_read = sandbox_handle(stdout_read, "stdout pipe handle")?;
    let stdout_write = sandbox_handle(stdout_write, "stdout writer handle")?;
    let stderr_read = sandbox_handle(stderr_read, "stderr pipe handle")?;
    let stderr_write = sandbox_handle(stderr_write, "stderr writer handle")?;

    let job = sandbox_handle(
        unsafe { CreateJobObjectW(null(), null()) },
        "CreateJobObjectW",
    )?;
    let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
    limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE
        | JOB_OBJECT_LIMIT_PROCESS_MEMORY
        | JOB_OBJECT_LIMIT_PROCESS_TIME
        | JOB_OBJECT_LIMIT_ACTIVE_PROCESS;
    limits.BasicLimitInformation.PerProcessUserTimeLimit = MAX_PARSER_CPU_100NS;
    limits.BasicLimitInformation.ActiveProcessLimit = 1;
    limits.ProcessMemoryLimit = MAX_PARSER_MEMORY;
    if unsafe {
        SetInformationJobObject(
            job.raw(),
            JobObjectExtendedLimitInformation,
            (&limits as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
            std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
        )
    } == 0
    {
        #[cfg(test)]
        log_sandbox_probe_failure("SetInformationJobObject");
        return Err("PDF_PARSER_SANDBOX_UNAVAILABLE".to_string());
    }

    for handle in [stdin_write.raw(), stdout_read.raw(), stderr_read.raw()] {
        if unsafe { windows_sys::Win32::Foundation::SetHandleInformation(handle, 1, 0) } == 0 {
            #[cfg(test)]
            log_sandbox_probe_failure("SetHandleInformation");
            return Err("PDF_PARSER_SANDBOX_UNAVAILABLE".to_string());
        }
    }

    let mut attributes = AttributeList::create().inspect_err(|_| {
        #[cfg(test)]
        log_sandbox_probe_failure("InitializeProcThreadAttributeList");
    })?;
    let capabilities = SECURITY_CAPABILITIES {
        AppContainerSid: app_container_sid,
        Capabilities: null_mut(),
        CapabilityCount: 0,
        Reserved: 0,
    };
    let child_handles = [stdin_read.raw(), stdout_write.raw(), stderr_write.raw()];
    let list = attributes.as_mut_ptr();
    let capabilities_ok = unsafe {
        UpdateProcThreadAttribute(
            list,
            0,
            PROC_THREAD_ATTRIBUTE_SECURITY_CAPABILITIES as usize,
            (&capabilities as *const SECURITY_CAPABILITIES).cast(),
            std::mem::size_of::<SECURITY_CAPABILITIES>(),
            null_mut(),
            null(),
        )
    };
    let handles_ok = unsafe {
        UpdateProcThreadAttribute(
            list,
            0,
            PROC_THREAD_ATTRIBUTE_HANDLE_LIST as usize,
            child_handles.as_ptr().cast(),
            std::mem::size_of_val(&child_handles),
            null_mut(),
            null(),
        )
    };
    if capabilities_ok == 0 || handles_ok == 0 {
        #[cfg(test)]
        log_sandbox_probe_failure("UpdateProcThreadAttribute");
        return Err("PDF_PARSER_SANDBOX_UNAVAILABLE".to_string());
    }

    let mut startup = STARTUPINFOEXW::default();
    startup.StartupInfo.cb = std::mem::size_of::<STARTUPINFOEXW>() as u32;
    startup.StartupInfo.dwFlags = STARTF_USESTDHANDLES;
    startup.StartupInfo.hStdInput = stdin_read.raw();
    startup.StartupInfo.hStdOutput = stdout_write.raw();
    startup.StartupInfo.hStdError = stderr_write.raw();
    startup.lpAttributeList = list;
    let mut command = command_line(&python, &script, runtime);
    let application = wide_os(python.as_os_str());
    let current_directory = wide_os(runtime.as_os_str());
    let environment = parser_environment(runtime).inspect_err(|_| {
        #[cfg(test)]
        log_sandbox_probe_failure("parser_environment");
    })?;
    let mut process_info = PROCESS_INFORMATION::default();
    let create_result = unsafe {
        CreateProcessW(
            application.as_ptr(),
            command.as_mut_ptr(),
            null(),
            null(),
            1,
            EXTENDED_STARTUPINFO_PRESENT
                | CREATE_SUSPENDED
                | CREATE_NO_WINDOW
                | CREATE_UNICODE_ENVIRONMENT,
            environment.as_ptr().cast(),
            current_directory.as_ptr(),
            (&startup.StartupInfo as *const windows_sys::Win32::System::Threading::STARTUPINFOW)
                .cast(),
            &mut process_info,
        )
    };
    if create_result == 0 {
        #[cfg(test)]
        log_sandbox_probe_failure("CreateProcessW");
        return Err("PDF_PARSER_SANDBOX_UNAVAILABLE".to_string());
    }
    let process = sandbox_handle(process_info.hProcess, "process handle")?;
    let thread_handle = sandbox_handle(process_info.hThread, "thread handle")?;
    if unsafe { AssignProcessToJobObject(job.raw(), process.raw()) } == 0 {
        unsafe { TerminateProcess(process.raw(), PARSER_EXIT_CODE) };
        #[cfg(test)]
        log_sandbox_probe_failure("AssignProcessToJobObject");
        return Err("PDF_PARSER_SANDBOX_UNAVAILABLE".to_string());
    }
    drop(stdin_read);
    drop(stdout_write);
    drop(stderr_write);
    if unsafe { ResumeThread(thread_handle.raw()) } == u32::MAX {
        unsafe { TerminateJobObject(job.raw(), PARSER_EXIT_CODE) };
        #[cfg(test)]
        log_sandbox_probe_failure("ResumeThread");
        return Err("PDF_PARSER_SANDBOX_UNAVAILABLE".to_string());
    }

    let stdout_thread = {
        let stdout = stdout_read.into_file();
        thread::spawn(move || read_bounded_and_drain(stdout, MAX_PARSER_OUTPUT_BYTES))
    };
    let stderr_thread = {
        let stderr = stderr_read.into_file();
        thread::spawn(move || read_bounded_and_drain(stderr, MAX_PARSER_STDERR_BYTES))
    };
    let input = zeroize::Zeroizing::new(selected_pdf.to_vec());
    let stdin_thread = {
        let mut stdin = stdin_write.into_file();
        thread::spawn(move || stdin.write_all(&input))
    };

    let deadline = Instant::now() + PARSER_TIMEOUT;
    loop {
        match unsafe { WaitForSingleObject(process.raw(), 100) } {
            WAIT_OBJECT_0 => break,
            WAIT_TIMEOUT if Instant::now() < deadline => {}
            WAIT_TIMEOUT => {
                unsafe { TerminateJobObject(job.raw(), PARSER_EXIT_CODE) };
                let _ = unsafe { WaitForSingleObject(process.raw(), 5_000) };
                let _ = stdin_thread.join();
                let _ = stdout_thread.join();
                let _ = stderr_thread.join();
                return Err("local document parser exceeded the 60 second bound".to_string());
            }
            _ => {
                unsafe { TerminateJobObject(job.raw(), PARSER_EXIT_CODE) };
                let _ = stdin_thread.join();
                let _ = stdout_thread.join();
                let _ = stderr_thread.join();
                return Err("local document parser status is unavailable".to_string());
            }
        }
    }
    stdin_thread
        .join()
        .map_err(|_| "local document parser input thread failed".to_string())?
        .map_err(|_| "local document parser input pipe failed".to_string())?;
    let (output, overflow) = stdout_thread
        .join()
        .map_err(|_| "local document parser output thread failed".to_string())??;
    let _ = stderr_thread.join();
    let mut exit_code = PARSER_EXIT_CODE;
    if unsafe { GetExitCodeProcess(process.raw(), &mut exit_code) } == 0 {
        return Err("local document parser status is unavailable".to_string());
    }
    Ok((exit_code == 0, output, overflow))
}

fn parser_environment(runtime: &Path) -> Result<Vec<u16>, String> {
    let system_root = std::env::var_os("SystemRoot")
        .or_else(|| std::env::var_os("WINDIR"))
        .ok_or_else(|| "PDF_PARSER_SANDBOX_UNAVAILABLE".to_string())?;
    let temporary = runtime.join("Temp");
    let local_app_data = runtime.join("LocalAppData");
    let mut values = vec![
        ("LOCALAPPDATA", local_app_data.as_os_str().to_os_string()),
        ("SystemRoot", system_root.clone()),
        ("TEMP", temporary.as_os_str().to_os_string()),
        ("TMP", temporary.as_os_str().to_os_string()),
        ("WINDIR", system_root),
    ];
    values.sort_by(|left, right| {
        left.0
            .to_ascii_lowercase()
            .cmp(&right.0.to_ascii_lowercase())
    });
    let mut block = Vec::new();
    for (name, value) in values {
        block.extend(OsStr::new(name).encode_wide());
        block.push('=' as u16);
        block.extend(value.encode_wide());
        block.push(0);
    }
    block.push(0);
    Ok(block)
}

fn command_line(python: &Path, script: &Path, runtime: &Path) -> Vec<u16> {
    let args = [
        python.as_os_str(),
        OsStr::new("-I"),
        OsStr::new("-B"),
        script.as_os_str(),
        OsStr::new("--root"),
        runtime.as_os_str(),
        OsStr::new("--input-stdin"),
        OsStr::new("--format"),
        OsStr::new("pdf"),
    ];
    let text = args
        .iter()
        .map(|argument| quote_windows_argument(argument))
        .collect::<Vec<_>>()
        .join(" ");
    wide(&text)
}

fn quote_windows_argument(argument: &OsStr) -> String {
    let value = argument.to_string_lossy();
    if !value.is_empty()
        && !value
            .chars()
            .any(|character| character.is_whitespace() || character == '"')
    {
        return value.into_owned();
    }
    let mut result = String::from("\"");
    let mut backslashes = 0usize;
    for character in value.chars() {
        match character {
            '\\' => backslashes += 1,
            '"' => {
                result.push_str(&"\\".repeat(backslashes * 2 + 1));
                result.push('"');
                backslashes = 0;
            }
            _ => {
                result.push_str(&"\\".repeat(backslashes));
                result.push(character);
                backslashes = 0;
            }
        }
    }
    result.push_str(&"\\".repeat(backslashes * 2));
    result.push('"');
    result
}

fn wide(value: &str) -> Vec<u16> {
    OsStr::new(value).encode_wide().chain(Some(0)).collect()
}

fn wide_os(value: &OsStr) -> Vec<u16> {
    value.encode_wide().chain(Some(0)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;

    fn copy_runtime(source: &Path, destination: &Path) {
        fs::create_dir_all(destination).unwrap();
        for entry in fs::read_dir(source).unwrap() {
            let entry = entry.unwrap();
            let source_path = entry.path();
            let destination_path = destination.join(entry.file_name());
            let metadata = fs::symlink_metadata(&source_path).unwrap();
            if metadata.is_dir() {
                copy_runtime(&source_path, &destination_path);
            } else {
                fs::copy(source_path, destination_path).unwrap();
            }
        }
    }

    #[test]
    fn appcontainer_denies_user_file_and_loopback_access() {
        let runtime = std::env::current_dir()
            .unwrap()
            .join(".knowledge-parser-runtime");
        let runtime = if runtime.is_dir() {
            runtime
        } else {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.knowledge-parser-runtime")
        };
        if !runtime.join("parser-runtime.manifest.json").is_file() {
            eprintln!("skipping AppContainer probe: isolated parser runtime is not staged");
            return;
        }

        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        listener.set_nonblocking(true).unwrap();
        let port = listener.local_addr().unwrap().port();
        let temp = tempfile::tempdir().unwrap();
        let user_file = temp.path().join("host-user-only-probe.txt");
        fs::write(&user_file, b"host-only-fixture").unwrap();
        fs::File::open(&user_file).expect("the host process must be able to open its user fixture");
        let test_runtime = temp.path().join("parser-runtime");
        copy_runtime(&runtime, &test_runtime);
        let user_file = serde_json::to_string(&user_file.to_string_lossy().to_string()).unwrap();
        let script = format!(
            r#"import json, socket, sys
sys.stdin.buffer.read()
try:
    open({user_file}, 'rb').read(1)
    file_access = 'allowed'
except OSError:
    file_access = 'blocked'
try:
    socket.create_connection(('127.0.0.1', {port}), timeout=1).close()
    network_access = 'allowed'
except OSError:
    network_access = 'blocked'
print(json.dumps({{'file': file_access, 'network': network_access}}))
"#
        );
        fs::write(test_runtime.join("extract_knowledge.py"), script).unwrap();

        let test_runtime = test_runtime.canonicalize().unwrap();
        validate_runtime(&test_runtime).expect("staged parser runtime must validate");
        let sid = app_container_sid().expect("AppContainer identity must be available");
        let stage = stage_runtime(&test_runtime, sid.0)
            .expect("isolated parser runtime must stage with sandbox access");
        let (succeeded, output, overflow) = run_child(&stage.0, sid.0, b"pipe-only-fixture")
            .expect("AppContainer parser child must start and return a bounded result");
        assert!(succeeded, "probe child should complete normally");
        assert!(
            !overflow,
            "probe result should remain within the output bound"
        );
        let result: serde_json::Value = serde_json::from_slice(&output).unwrap();
        assert_eq!(result["file"], "blocked");
        assert_eq!(result["network"], "blocked");
        assert!(
            matches!(listener.accept(), Err(error) if error.kind() == std::io::ErrorKind::WouldBlock)
        );
    }
}
