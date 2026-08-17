//! UAC elevation — request administrator privileges.

use crate::error::{CoreError, Result};
use std::os::windows::ffi::OsStrExt;
use std::path::Path;
use tracing::{debug, info};

/// Check if the current process is running with administrator privileges.
pub fn is_admin() -> bool {
    use windows::Win32::Foundation::*;
    use windows::Win32::Security::*;

    unsafe {
        let mut admin_sid: PSID = PSID::default();
        let admin_group = SID_IDENTIFIER_AUTHORITY {
            Value: [0, 0, 0, 0, 0, 5], // SECURITY_NT_AUTHORITY
        };

        let allocated = AllocateAndInitializeSid(
            &admin_group as *const SID_IDENTIFIER_AUTHORITY,
            2,
            0x00000020, // SECURITY_BUILTIN_DOMAIN_RID
            0x00000220, // DOMAIN_ALIAS_RID_ADMINS
            0, 0, 0, 0, 0, 0,
            &mut admin_sid as *mut PSID,
        );

        if allocated.is_err() {
            return false;
        }

        let mut is_member: BOOL = BOOL(0);
        let result = CheckTokenMembership(
            None,
            admin_sid,
            &mut is_member,
        );

        let _ = FreeSid(admin_sid);

        result.is_ok() && is_member.as_bool()
    }
}

/// Re-launch the current executable with administrator privileges via UAC elevation.
///
/// Returns `Ok(true)` if elevation was requested (caller should exit),
/// `Ok(false)` if already elevated.
pub fn elevate_if_needed(args: &[String]) -> Result<bool> {
    if is_admin() {
        debug!("Already running as administrator");
        return Ok(false);
    }

    let exe_path = std::env::current_exe()
        .map_err(|e| CoreError::Other(format!("Failed to get exe path: {}", e)))?;

    info!("Requesting elevation: {}", exe_path.display());

    let result = shell_execute_elevated(&exe_path, args)?;

    if result {
        Ok(true)
    } else {
        Err(CoreError::ElevationRequired)
    }
}

/// Execute a program elevated via ShellExecuteExW with "runas".
fn shell_execute_elevated(exe_path: &Path, args: &[String]) -> Result<bool> {
    use windows::core::*;
    use windows::Win32::Foundation::*;
    use windows::Win32::UI::Shell::*;
    use windows::Win32::UI::WindowsAndMessaging::*;

    let file_wide: Vec<u16> = exe_path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    let args_str = args.join(" ");
    let args_wide: Vec<u16> = args_str
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();

    let dir = exe_path
        .parent()
        .map(|p| {
            p.as_os_str()
                .encode_wide()
                .chain(std::iter::once(0))
                .collect::<Vec<u16>>()
        })
        .unwrap_or_else(|| vec![0u16]);

    let op: Vec<u16> = "runas\0".encode_utf16().collect();

    let mut sei = SHELLEXECUTEINFOW {
        cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
        lpVerb: PCWSTR(op.as_ptr()),
        lpFile: PCWSTR(file_wide.as_ptr()),
        lpParameters: PCWSTR(args_wide.as_ptr()),
        lpDirectory: PCWSTR(dir.as_ptr()),
        nShow: SW_SHOWNORMAL.0,
        fMask: SEE_MASK_NOCLOSEPROCESS,
        ..Default::default()
    };

    unsafe {
        match ShellExecuteExW(&mut sei) {
            Ok(()) => {
                let _ = CloseHandle(sei.hProcess);
                Ok(true)
            }
            Err(e) => {
                debug!("ShellExecuteExW failed: {}", e);
                Ok(false)
            }
        }
    }
}
