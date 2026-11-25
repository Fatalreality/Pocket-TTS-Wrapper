//! Storage for the Hugging Face access token.
//!
//! The Hugging Face tooling reads `~/.cache/huggingface/token`, so the token has to live in that
//! file in plain text. This module makes sure the file is readable by the signed-in user alone,
//! and that the token is only written once those permissions are in place.

use std::io::{self, Write};
use std::path::Path;

/// Writes `token` to `path` and leaves the file readable only by the current user.
///
/// The token is written to a sibling file that is locked down *before* the secret goes into it,
/// and only then renamed over `path`. The existing token is therefore never emptied or replaced
/// by a half-finished write: if creating, restricting or writing the new file fails, the file
/// that was already there keeps its token and its permissions.
pub fn write_token(path: &Path, token: &str) -> io::Result<()> {
    let temp = sibling_temp(path)?;
    // Left over by a crash, and otherwise in the way of `create_new` below.
    let _ = std::fs::remove_file(&temp);
    let written = write_atomically(&temp, path, token);
    if written.is_err() {
        let _ = std::fs::remove_file(&temp);
    }
    written
}

fn write_atomically(temp: &Path, path: &Path, token: &str) -> io::Result<()> {
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(temp)?;
    restrict(temp)?;
    file.write_all(token.as_bytes())?;
    file.sync_all()?;
    drop(file);
    // Same folder, so the rename replaces the token file in one step.
    std::fs::rename(temp, path)
}

/// The one sibling file a write can use, so a failed attempt is easy to clean up.
fn sibling_temp(path: &Path) -> io::Result<std::path::PathBuf> {
    let name = path
        .file_name()
        .ok_or_else(|| io::Error::other("the token file has no name"))?;
    Ok(path.with_file_name(format!(
        "{}.tmp-{}",
        name.to_string_lossy(),
        std::process::id()
    )))
}

/// Reduces `path` to the current user, for a token file that already exists.
pub fn restrict(path: &Path) -> io::Result<()> {
    only_current_user(path)
}

/// Replaces the object's DACL with a single entry for the current user, so no inherited group,
/// `Everyone` or other account keeps read access. Windows only; elsewhere the file mode is 0600.
#[cfg(target_os = "windows")]
fn only_current_user(path: &Path) -> io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows::core::{PCWSTR, PWSTR};
    use windows::Win32::Foundation::{CloseHandle, LocalFree, GENERIC_ALL, HANDLE, HLOCAL, NO_ERROR};
    use windows::Win32::Security::Authorization::{
        SetEntriesInAclW, SetNamedSecurityInfoW, EXPLICIT_ACCESS_W, NO_MULTIPLE_TRUSTEE,
        SE_FILE_OBJECT, SET_ACCESS, TRUSTEE_IS_SID, TRUSTEE_IS_USER, TRUSTEE_W,
    };
    use windows::Win32::Security::{
        GetTokenInformation, TokenUser, ACL, DACL_SECURITY_INFORMATION, NO_INHERITANCE,
        OBJECT_SECURITY_INFORMATION, PROTECTED_DACL_SECURITY_INFORMATION, TOKEN_QUERY, TOKEN_USER,
    };
    use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

    unsafe {
        let mut token = HANDLE::default();
        OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token)
            .map_err(|_| io::Error::other("cannot open the current user's access token"))?;

        // Two calls: the first reports how much room the user information needs.
        let mut needed = 0u32;
        let _ = GetTokenInformation(token, TokenUser, None, 0, &mut needed);
        if needed == 0 {
            let _ = CloseHandle(token);
            return Err(io::Error::other("cannot read the current user's security identifier"));
        }
        let mut buffer = vec![0u8; needed as usize];
        let read = GetTokenInformation(
            token,
            TokenUser,
            Some(buffer.as_mut_ptr() as *mut core::ffi::c_void),
            needed,
            &mut needed,
        );
        if read.is_err() {
            let _ = CloseHandle(token);
            return Err(io::Error::other("cannot read the current user's security identifier"));
        }
        // SAFETY: the call above filled `buffer` with a TOKEN_USER when it succeeded.
        let sid = (*(buffer.as_ptr() as *const TOKEN_USER)).User.Sid;

        let entry = EXPLICIT_ACCESS_W {
            grfAccessPermissions: GENERIC_ALL.0,
            grfAccessMode: SET_ACCESS,
            grfInheritance: NO_INHERITANCE,
            Trustee: TRUSTEE_W {
                pMultipleTrustee: std::ptr::null_mut(),
                MultipleTrusteeOperation: NO_MULTIPLE_TRUSTEE,
                TrusteeForm: TRUSTEE_IS_SID,
                TrusteeType: TRUSTEE_IS_USER,
                ptstrName: PWSTR(sid.0 as *mut u16),
            },
        };

        let mut acl: *mut ACL = std::ptr::null_mut();
        let built = SetEntriesInAclW(Some(&[entry]), None, &mut acl);
        if built != NO_ERROR {
            let _ = CloseHandle(token);
            return Err(io::Error::from_raw_os_error(built.0 as i32));
        }

        let mut wide: Vec<u16> = path.as_os_str().encode_wide().collect();
        wide.push(0);
        // PROTECTED_DACL_SECURITY_INFORMATION stops the folder's permissions being inherited,
        // which is what keeps other accounts out.
        let info = OBJECT_SECURITY_INFORMATION(
            DACL_SECURITY_INFORMATION.0 | PROTECTED_DACL_SECURITY_INFORMATION.0,
        );
        let applied = SetNamedSecurityInfoW(
            PCWSTR(wide.as_ptr()),
            SE_FILE_OBJECT,
            info,
            None,
            None,
            Some(acl),
            None,
        );

        let _ = LocalFree(Some(HLOCAL(acl as *mut core::ffi::c_void)));
        let _ = CloseHandle(token);
        if applied != NO_ERROR {
            return Err(io::Error::from_raw_os_error(applied.0 as i32));
        }
        Ok(())
    }
}

/// `icacls` and the DACL API are Windows-only; elsewhere the file mode does the same job.
#[cfg(not(target_os = "windows"))]
fn only_current_user(path: &Path) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
}

#[cfg(all(test, target_os = "windows"))]
mod tests {
    use super::*;
    use std::process::Command;

    /// Reads the file back through `icacls`, which reports exactly the accounts that can open it.
    #[test]
    fn the_token_file_belongs_to_the_current_user_alone() {
        let dir = std::env::temp_dir().join(format!("pocket-tts-token-acl-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("token");

        write_token(&path, "hf_0123456789abcdef0123456789abcdef0123").unwrap();
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "hf_0123456789abcdef0123456789abcdef0123"
        );

        let root = std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".into());
        let output = Command::new(format!("{root}\\System32\\icacls.exe"))
            .arg(&path)
            .output()
            .unwrap();
        let report = String::from_utf8_lossy(&output.stdout);
        assert!(output.status.success(), "icacls failed:\n{report}");

        // Every access-control entry ends in `:(rights)`; only the current user should have one.
        let entries = report.lines().filter(|line| line.contains(":(")).count();
        assert_eq!(entries, 1, "expected just the current user, got:\n{report}");

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A write that cannot finish must leave the token that is already saved in place.
    #[test]
    fn a_failed_write_keeps_the_existing_token() {
        let dir = std::env::temp_dir().join(format!("pocket-tts-token-keep-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("token");
        write_token(&path, "hf_old_token_that_must_survive").unwrap();

        // Occupy the sibling file a write needs, so the new token cannot be staged.
        let blocker = sibling_temp(&path).unwrap();
        std::fs::create_dir_all(&blocker).unwrap();
        assert!(write_token(&path, "hf_new_token").is_err());

        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "hf_old_token_that_must_survive"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }
}
