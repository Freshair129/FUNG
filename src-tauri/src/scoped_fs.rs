//! Handle-relative filesystem operations shared by routed audio custody.
use std::path::Path;

pub(crate) fn is_reparse(metadata: &std::fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        metadata.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        metadata.file_type().is_symlink()
    }
}

/// Pin each directory object while creating/publishing by path. Windows
/// share mode permits reads/writes inside a directory, but denies renaming or
/// deleting that directory until these handles are dropped.
#[cfg(windows)]
pub(crate) fn pin_directory(path: &Path) -> Result<std::fs::File, String> {
    use std::os::windows::fs::OpenOptionsExt;
    use windows_sys::Win32::Storage::FileSystem::{
        FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT, FILE_SHARE_READ, FILE_SHARE_WRITE,
    };
    let file = std::fs::OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)
        .map_err(|e| format!("source export directory pin failed: {e}"))?;
    let meta = file.metadata().map_err(|e| e.to_string())?;
    if is_reparse(&meta) || !meta.is_dir() {
        return Err("source export directory is a reparse point".into());
    }
    let actual =
        crate::desktop_playback::final_path_from_handle(&file, path).map_err(|e| e.code)?;
    if !crate::desktop_playback::normalized_path_inside(&actual, path)
        || !crate::desktop_playback::normalized_path_inside(path, &actual)
    {
        return Err("source export directory identity changed".into());
    }
    Ok(file)
}

#[cfg(not(windows))]
pub(crate) fn pin_directory(_: &Path) -> Result<std::fs::File, String> {
    Err("source export directory pinning requires Windows".into())
}

#[cfg(windows)]
pub(crate) fn open_relative(
    parent: &std::fs::File,
    name: &std::ffi::OsStr,
    directory: bool,
    disposition: u32,
    write: bool,
) -> Result<std::fs::File, String> {
    use std::os::windows::{
        ffi::OsStrExt,
        io::{AsRawHandle, FromRawHandle},
    };
    #[repr(C)]
    struct UnicodeString {
        len: u16,
        max_len: u16,
        buffer: *mut u16,
    }
    #[repr(C)]
    struct ObjectAttributes {
        length: u32,
        root: *mut std::ffi::c_void,
        name: *mut UnicodeString,
        attributes: u32,
        security: *mut std::ffi::c_void,
        qos: *mut std::ffi::c_void,
    }
    #[repr(C)]
    struct IoStatus {
        status: isize,
        information: usize,
    }
    #[link(name = "ntdll")]
    unsafe extern "system" {
        fn NtCreateFile(
            handle: *mut *mut std::ffi::c_void,
            access: u32,
            attrs: *mut ObjectAttributes,
            ios: *mut IoStatus,
            allocation: *mut i64,
            attributes: u32,
            share: u32,
            disposition: u32,
            options: u32,
            ea: *mut std::ffi::c_void,
            ea_len: u32,
        ) -> i32;
    }
    let mut wide: Vec<u16> = name.encode_wide().collect();
    if wide.is_empty()
        || wide
            .iter()
            .any(|c| *c == 0 || *c == b'\\' as u16 || *c == b'/' as u16 || *c == b':' as u16)
        || wide == [b'.' as u16]
        || wide == [b'.' as u16, b'.' as u16]
    {
        return Err("invalid source export component".into());
    }
    let bytes = u16::try_from(wide.len().checked_mul(2).ok_or("component too long")?)
        .map_err(|_| "component too long")?;
    let mut name = UnicodeString {
        len: bytes,
        max_len: bytes,
        buffer: wide.as_mut_ptr(),
    };
    const OBJ_DONT_REPARSE: u32 = 0x1000;
    const FILE_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
    const DIRECTORY_SYNCHRONOUS: u32 = 0x21;
    const FILE_SYNCHRONOUS_NON_DIRECTORY: u32 = 0x60;
    const DIRECTORY_READ_ACCESS: u32 = 0x8010_0081;
    const FILE_READ_ACCESS: u32 = 0x8010_0080;
    const FILE_READ_WRITE_ACCESS: u32 = 0xC010_0080;
    let mut attrs = ObjectAttributes {
        length: std::mem::size_of::<ObjectAttributes>() as u32,
        root: parent.as_raw_handle(),
        name: &mut name,
        attributes: OBJ_DONT_REPARSE,
        security: std::ptr::null_mut(),
        qos: std::ptr::null_mut(),
    };
    let mut ios = IoStatus {
        status: 0,
        information: 0,
    };
    let mut handle = std::ptr::null_mut();
    let access = if directory {
        DIRECTORY_READ_ACCESS
    } else if write {
        FILE_READ_WRITE_ACCESS
    } else {
        FILE_READ_ACCESS
    };
    let options = FILE_OPEN_REPARSE_POINT
        | if directory {
            DIRECTORY_SYNCHRONOUS
        } else {
            FILE_SYNCHRONOUS_NON_DIRECTORY
        };
    let status = unsafe {
        NtCreateFile(
            &mut handle,
            access,
            &mut attrs,
            &mut ios,
            std::ptr::null_mut(),
            if directory { 0x10 } else { 0x80 },
            if directory { 3 } else { 1 },
            disposition,
            options,
            std::ptr::null_mut(),
            0,
        )
    };
    if status < 0 || handle.is_null() {
        return Err(format!(
            "source export relative open failed: NTSTATUS {:#010x}",
            status as u32
        ));
    }
    let file = unsafe { std::fs::File::from_raw_handle(handle) };
    let meta = file.metadata().map_err(|e| e.to_string())?;
    if is_reparse(&meta) || meta.is_dir() != directory {
        return Err("source export relative object is invalid".into());
    }
    Ok(file)
}

#[cfg(not(windows))]
pub(crate) fn open_relative(
    _: &std::fs::File,
    _: &std::ffi::OsStr,
    _: bool,
    _: u32,
    _: bool,
) -> Result<std::fs::File, String> {
    Err("source export directory pinning requires Windows".into())
}

pub(crate) fn pin_ancestors(path: &Path) -> Result<Vec<std::fs::File>, String> {
    let mut components = path.ancestors().collect::<Vec<_>>();
    components.reverse();
    let root = components.first().ok_or("source export root missing")?;
    let mut handles = vec![pin_directory(root)?];
    for component in components.iter().skip(1) {
        let name = component
            .file_name()
            .ok_or("source export component missing")?;
        let file = open_relative(
            handles.last().ok_or("missing parent handle")?,
            name,
            true,
            1,
            false,
        )?;
        let actual = crate::desktop_playback::final_path_from_handle(&file, component)
            .map_err(|e| e.code)?;
        if !crate::desktop_playback::normalized_path_inside(&actual, component)
            || !crate::desktop_playback::normalized_path_inside(component, &actual)
        {
            return Err("source export directory identity changed".into());
        }
        handles.push(file);
    }
    Ok(handles)
}

#[cfg(all(test, windows))]
mod windows_tests {
    use super::{pin_ancestors, pin_directory};
    use std::ffi::OsString;
    use std::os::windows::ffi::{OsStrExt, OsStringExt};
    use std::path::{Path, PathBuf};
    use windows_sys::Win32::Storage::FileSystem::GetShortPathNameW;

    fn short_path(path: &Path) -> PathBuf {
        let input = path
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect::<Vec<_>>();
        let required = unsafe { GetShortPathNameW(input.as_ptr(), std::ptr::null_mut(), 0) };
        assert_ne!(
            required, 0,
            "GetShortPathNameW must query the existing fixture"
        );

        let mut output = vec![0u16; required as usize];
        let length =
            unsafe { GetShortPathNameW(input.as_ptr(), output.as_mut_ptr(), output.len() as u32) };
        assert_ne!(length, 0, "GetShortPathNameW must return the fixture path");
        assert!((length as usize) < output.len());
        output.truncate(length as usize);
        PathBuf::from(OsString::from_wide(&output))
    }

    #[test]
    fn pin_directory_and_ancestors_accept_8dot3_paths() {
        let temporary = tempfile::tempdir().unwrap();
        let long_path = temporary
            .path()
            .join("fung short path identity fixture with spaces");
        std::fs::create_dir(&long_path).unwrap();

        let short = short_path(&long_path);
        if short
            .to_string_lossy()
            .eq_ignore_ascii_case(&long_path.to_string_lossy())
        {
            println!(
                "skipping 8.3 identity assertions: this volume returned no short-name alias for a long fixture path"
            );
            return;
        }

        let directory = pin_directory(&short).expect("pin_directory must accept the 8.3 path");
        drop(directory);
        let ancestors = pin_ancestors(&short).expect("pin_ancestors must accept the 8.3 path");
        drop(ancestors);
    }
}
