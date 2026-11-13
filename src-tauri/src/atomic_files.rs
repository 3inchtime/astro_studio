//! Publish a complete sibling temporary file without replacing an existing name.
//! macOS and Windows use exclusive rename, including on FAT/exFAT volumes.
use std::{io, path::Path};

pub(crate) fn publish_no_replace(source: &Path, destination: &Path) -> io::Result<()> {
    #[cfg(target_os = "macos")]
    {
        use std::{ffi::CString, os::unix::ffi::OsStrExt};
        let source = CString::new(source.as_os_str().as_bytes())
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "NUL in source path"))?;
        let destination = CString::new(destination.as_os_str().as_bytes())
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "NUL in destination path"))?;
        // RENAME_EXCL atomically rejects any existing destination, including symlinks.
        let result =
            unsafe { libc::renamex_np(source.as_ptr(), destination.as_ptr(), libc::RENAME_EXCL) };
        if result == 0 {
            Ok(())
        } else {
            Err(io::Error::last_os_error())
        }
    }
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::Storage::FileSystem::{MoveFileExW, MOVEFILE_WRITE_THROUGH};
        let source: Vec<u16> = source.as_os_str().encode_wide().collect();
        let destination: Vec<u16> = destination.as_os_str().encode_wide().collect();
        if source.contains(&0) || destination.contains(&0) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "NUL in file path",
            ));
        }
        let source: Vec<u16> = source.into_iter().chain(Some(0)).collect();
        let destination: Vec<u16> = destination.into_iter().chain(Some(0)).collect();
        // Deliberately omit MOVEFILE_REPLACE_EXISTING and MOVEFILE_COPY_ALLOWED.
        let result = unsafe {
            MoveFileExW(
                source.as_ptr(),
                destination.as_ptr(),
                MOVEFILE_WRITE_THROUGH,
            )
        };
        if result != 0 {
            Ok(())
        } else {
            Err(io::Error::last_os_error())
        }
    }
    #[cfg(target_os = "linux")]
    {
        use std::{ffi::CString, os::unix::ffi::OsStrExt};
        let source_c = CString::new(source.as_os_str().as_bytes())
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "NUL in source path"))?;
        let destination_c = CString::new(destination.as_os_str().as_bytes())
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "NUL in destination path"))?;
        let result = unsafe {
            libc::syscall(
                libc::SYS_renameat2,
                libc::AT_FDCWD,
                source_c.as_ptr(),
                libc::AT_FDCWD,
                destination_c.as_ptr(),
                libc::RENAME_NOREPLACE,
            )
        };
        if result == 0 {
            return Ok(());
        }
        let error = io::Error::last_os_error();
        if !matches!(
            error.raw_os_error(),
            Some(libc::ENOSYS) | Some(libc::EINVAL)
        ) {
            return Err(error);
        }
        // Older kernels/filesystems may lack renameat2. Hard-link creation still
        // publishes atomically without replacement; unsupported filesystems fail safely.
        std::fs::hard_link(source, destination)?;
        std::fs::remove_file(source)
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        // Safe fallback: never substitute a check-then-rename or truncate-copy.
        std::fs::hard_link(source, destination)?;
        std::fs::remove_file(source)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    #[test]
    fn publishes_complete_file_and_never_replaces_existing_destination() {
        let root = std::env::temp_dir().join(format!("astro-publish-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let source = root.join("source.tmp");
        let target = root.join("image.svg");
        fs::write(&source, b"original").unwrap();
        publish_no_replace(&source, &target).unwrap();
        assert!(!source.exists());
        assert_eq!(fs::read(&target).unwrap(), b"original");
        fs::write(&source, b"replacement").unwrap();
        assert_eq!(
            publish_no_replace(&source, &target).unwrap_err().kind(),
            std::io::ErrorKind::AlreadyExists
        );
        assert_eq!(fs::read(&target).unwrap(), b"original");
        assert_eq!(fs::read(&source).unwrap(), b"replacement");
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn concurrent_publishers_have_exactly_one_winner() {
        let root = std::env::temp_dir().join(format!("astro-publish-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let target = root.join("result");
        let tasks: Vec<_> = (0..8)
            .map(|index| {
                let source = root.join(format!("{index}.tmp"));
                fs::write(&source, format!("payload-{index}")).unwrap();
                let target = target.clone();
                std::thread::spawn(move || publish_no_replace(&source, &target))
            })
            .collect();
        let mut successes = 0;
        for task in tasks {
            match task.join().unwrap() {
                Ok(()) => successes += 1,
                Err(error) => assert_eq!(error.kind(), std::io::ErrorKind::AlreadyExists),
            }
        }
        assert_eq!(successes, 1);
        assert!(fs::read_to_string(target).unwrap().starts_with("payload-"));
        fs::remove_dir_all(root).unwrap();
    }
}
