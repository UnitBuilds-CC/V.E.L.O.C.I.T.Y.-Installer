//! Windows platform implementations.

use std::path::PathBuf;

/// Detect system architecture via environment variables and compile-time checks.
pub fn detect_arch() -> String {
    if let Ok(arch) = std::env::var("PROCESSOR_ARCHITECTURE") {
        match arch.to_lowercase().as_str() {
            "amd64" => return "x64".to_string(),
            "x86" => {
                if let Ok(w6432) = std::env::var("PROCESSOR_ARCHITEW6432") {
                    return match w6432.to_lowercase().as_str() {
                        "amd64" => "x64".to_string(),
                        "arm64" => "arm64".to_string(),
                        _ => "x86".to_string(),
                    };
                }
                return "x86".to_string();
            }
            "arm64" => return "arm64".to_string(),
            _ => {}
        }
    }
    if cfg!(target_arch = "x86_64") {
        "x64".to_string()
    } else if cfg!(target_arch = "aarch64") {
        "arm64".to_string()
    } else if cfg!(target_arch = "x86") {
        "x86".to_string()
    } else {
        "unknown".to_string()
    }
}

/// Check if the current process is running elevated (Administrator).
pub fn is_elevated() -> bool {
    use windows::Win32::Security::*;

    // SAFETY: Win32 SID API — AllocateAndInitializeSid allocates a SID that is
    // freed by FreeSid on all paths. CheckTokenMembership borrows the SID.
    unsafe {
        let mut admin_sid: PSID = PSID::default();
        let admin_group = SID_IDENTIFIER_AUTHORITY {
            Value: [0, 0, 0, 0, 0, 5],
        };

        let allocated = AllocateAndInitializeSid(
            &admin_group as *const SID_IDENTIFIER_AUTHORITY,
            2,
            0x00000020,
            0x00000220,
            0,
            0,
            0,
            0,
            0,
            0,
            &mut admin_sid as *mut PSID,
        );

        if allocated.is_err() {
            return false;
        }

        let mut is_member: windows::core::BOOL = windows::core::BOOL(0);
        let result = CheckTokenMembership(None, admin_sid, &mut is_member);
        let _ = FreeSid(admin_sid);

        result.is_ok() && is_member.as_bool()
    }
}

/// Default installation directory (e.g. `C:\Program Files\AppName`).
pub fn default_install_dir(app_name: &str) -> PathBuf {
    let pf = std::env::var("ProgramFiles").unwrap_or_else(|_| r"C:\Program Files".to_string());
    PathBuf::from(pf).join(app_name)
}

/// Default configuration directory.
pub fn default_config_dir() -> PathBuf {
    std::env::var("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|_| dirs::config_dir().unwrap_or_else(|| PathBuf::from(r"C:\ProgramData")))
}

/// Request a system reboot via shutdown.exe.
pub fn request_system_reboot() -> crate::error::Result<bool> {
    let output = std::process::Command::new("shutdown")
        .args(["/r", "/t", "0", "/d", "p:4:1"])
        .output()
        .map_err(|e| {
            crate::error::CoreError::other(
                "reboot",
                format!("Failed to run shutdown command: {}", e),
            )
        })?;

    if output.status.success() {
        Ok(true)
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(crate::error::CoreError::other(
            "reboot",
            format!("Failed to initiate reboot: {}", stderr),
        ))
    }
}

/// Check if a system reboot is pending (via Session Manager registry key).
pub fn is_reboot_pending() -> bool {
    use winreg::enums::HKEY_LOCAL_MACHINE;
    use winreg::RegKey;

    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    if let Ok(key) = hklm.open_subkey(r"SYSTEM\CurrentControlSet\Control\Session Manager") {
        if key.get_raw_value("PendingFileRenameOperations").is_ok() {
            return true;
        }
        if let Ok(flag) = key.get_value::<u32, _>("RebootRequired") {
            if flag != 0 {
                return true;
            }
        }
    }
    false
}

/// User's desktop directory.
pub fn desktop_dir() -> PathBuf {
    std::env::var("USERPROFILE")
        .map(|p| PathBuf::from(p).join("Desktop"))
        .unwrap_or_else(|_| PathBuf::from(r"C:\Users\Public\Desktop"))
}

/// Start Menu programs directory.
pub fn start_menu_dir() -> PathBuf {
    std::env::var("APPDATA")
        .map(|p| {
            PathBuf::from(p)
                .join("Microsoft")
                .join("Windows")
                .join("Start Menu")
                .join("Programs")
        })
        .unwrap_or_else(|_| PathBuf::from(r"C:\ProgramData\Microsoft\Windows\Start Menu\Programs"))
}
