//! System architecture detection for 32/64-bit install decisions.
//!
//! Provides:
//! - OS architecture detection (x86, x64, ARM64)
//! - WoW64 detection (32-bit process on 64-bit OS)
//! - Program Files path resolution
//! - Registry redirection awareness
//! - Install mode helpers (32-bit vs 64-bit install)

use std::path::PathBuf;

/// Detected system architecture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SystemArch {
    /// 32-bit x86
    X86,
    /// 64-bit x86-64 (AMD64)
    X64,
    /// 64-bit ARM (AArch64)
    Arm64,
    /// Unknown architecture
    Unknown,
}

impl std::fmt::Display for SystemArch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SystemArch::X86 => write!(f, "x86"),
            SystemArch::X64 => write!(f, "x64"),
            SystemArch::Arm64 => write!(f, "arm64"),
            SystemArch::Unknown => write!(f, "unknown"),
        }
    }
}

/// Information about the current system environment.
#[derive(Debug, Clone)]
pub struct SystemInfo {
    /// The native OS architecture
    pub os_arch: SystemArch,
    /// Whether this process is running under WoW64
    pub is_wow64: bool,
    /// Whether this process is 64-bit
    pub is_64bit_process: bool,
    /// Whether the OS supports 64-bit installation
    pub supports_64bit: bool,
    /// Program Files directory (native)
    pub program_files: PathBuf,
    /// Program Files (x86) directory (if applicable)
    pub program_files_x86: Option<PathBuf>,
    /// Common Files directory (native)
    pub common_files: PathBuf,
    /// Common Files (x86) directory (if applicable)
    pub common_files_x86: Option<PathBuf>,
    /// System32 directory (native)
    pub system_dir: PathBuf,
    /// SysWOW64 directory (if applicable)
    pub sys_wow64: Option<PathBuf>,
    /// Windows directory
    pub windows_dir: PathBuf,
}

/// Detect the current system information.
pub fn detect_system_info() -> SystemInfo {
    let os_arch = detect_os_arch();
    let is_wow64 = detect_wow64();
    let is_64bit_process = cfg!(target_pointer_width = "64");
    let supports_64bit = os_arch == SystemArch::X64 || os_arch == SystemArch::Arm64;

    // Resolve system directories via environment variables
    let program_files = get_env_path("ProgramFiles", r"C:\Program Files");
    let program_files_x86 = std::env::var("ProgramFiles(x86)")
        .ok()
        .map(PathBuf::from)
        .or_else(|| {
            // If running WoW64, ProgramFiles is actually the x86 directory
            if is_wow64 {
                Some(PathBuf::from(r"C:\Program Files (x86)"))
            } else {
                None
            }
        });

    let common_files = get_env_path("CommonProgramFiles", r"C:\Program Files\Common Files");
    let common_files_x86 = std::env::var("CommonProgramFiles(x86)")
        .ok()
        .map(PathBuf::from);

    let system_dir = get_env_path("SystemRoot", r"C:\Windows")
        .join(if is_wow64 { "SysWOW64" } else { "System32" });
    let sys_wow64 = if !is_wow64 && supports_64bit {
        let wow = get_env_path("SystemRoot", r"C:\Windows").join("SysWOW64");
        if wow.exists() { Some(wow) } else { None }
    } else {
        None
    };

    let windows_dir = get_env_path("SystemRoot", r"C:\Windows");

    SystemInfo {
        os_arch,
        is_wow64,
        is_64bit_process,
        supports_64bit,
        program_files,
        program_files_x86,
        common_files,
        common_files_x86,
        system_dir,
        sys_wow64,
        windows_dir,
    }
}

/// Detect the native OS architecture.
fn detect_os_arch() -> SystemArch {
    // Check PROCESSOR_ARCHITECTURE environment variable
    if let Ok(arch) = std::env::var("PROCESSOR_ARCHITECTURE") {
        match arch.to_lowercase().as_str() {
            "amd64" => return SystemArch::X64,
            "x86" => {
                // Could be native x86 or WoW64 — check PROCESSOR_ARCHITEW6432
                if let Ok(w6432) = std::env::var("PROCESSOR_ARCHITEW6432") {
                    return match w6432.to_lowercase().as_str() {
                        "amd64" => SystemArch::X64,
                        "arm64" => SystemArch::Arm64,
                        _ => SystemArch::X86,
                    };
                }
                return SystemArch::X86;
            }
            "arm64" => return SystemArch::Arm64,
            _ => {}
        }
    }

    // Fallback: compile-time detection
    if cfg!(target_arch = "x86_64") {
        SystemArch::X64
    } else if cfg!(target_arch = "aarch64") {
        SystemArch::Arm64
    } else if cfg!(target_arch = "x86") {
        SystemArch::X86
    } else {
        SystemArch::Unknown
    }
}

/// Detect if the current process is running under WoW64.
fn detect_wow64() -> bool {
    // A 32-bit process on a 64-bit OS has PROCESSOR_ARCHITEW6432 set
    if std::env::var("PROCESSOR_ARCHITEW6432").is_ok() {
        return true;
    }

    // A 64-bit process is never WoW64
    if cfg!(target_pointer_width = "64") {
        return false;
    }

    // For 32-bit processes, use IsWow64Process via Windows API
    #[cfg(target_os = "windows")]
    {
        // SAFETY: GetProcAddress returns a valid function pointer for IsWow64Process
        // from kernel32.dll (always loaded). The transmute target signature matches
        // the actual API exactly. GetCurrentProcess returns a pseudo-handle (no close needed).
        unsafe {
            use windows::Win32::System::Threading::GetCurrentProcess;
            use windows::Win32::System::LibraryLoader::GetModuleHandleW;

            // Dynamically load IsWow64Process for compatibility
            let kernel32 = GetModuleHandleW(windows::core::w!("kernel32.dll")).ok();
            if let Some(module) = kernel32 {
                // Use GetProcAddress to find IsWow64Process
                let func_name = windows::core::s!("IsWow64Process");
                if let Some(func) = windows::Win32::System::LibraryLoader::GetProcAddress(
                    module,
                    func_name,
                ) {
                    let is_wow64_fn: unsafe extern "system" fn(
                        windows::Win32::Foundation::HANDLE,
                        *mut windows::Win32::Foundation::BOOL,
                    ) -> windows::Win32::Foundation::BOOL =
                        std::mem::transmute(func);

                    let mut result = windows::Win32::Foundation::BOOL::from(false);
                    let process = GetCurrentProcess();
                    if is_wow64_fn(process, &mut result).as_bool() {
                        return result.as_bool();
                    }
                }
            }
        }
    }

    false
}

/// Get an environment variable as a PathBuf, with a fallback.
fn get_env_path(var: &str, fallback: &str) -> PathBuf {
    std::env::var(var)
        .ok()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(fallback))
}

/// Check if the system is 64-bit.
pub fn is_64bit_os() -> bool {
    let info = detect_system_info();
    info.supports_64bit
}

/// Check if the manifest's target arch matches the current system.
///
/// Returns true if the system can run the specified architecture.
/// - x86 binaries run on all systems
/// - x64 binaries require x64 or arm64 OS
/// - arm64 binaries require arm64 OS
pub fn is_arch_compatible(target_arch: &str) -> bool {
    let info = detect_system_info();
    match target_arch.to_lowercase().as_str() {
        "x86" | "x86_32" | "win32" | "32bit" => true, // x86 runs everywhere
        "x64" | "x86_64" | "amd64" | "64bit" => info.os_arch == SystemArch::X64,
        "arm64" | "aarch64" => info.os_arch == SystemArch::Arm64,
        _ => false,
    }
}

/// Get the appropriate Program Files directory for the install mode.
///
/// - `install_64bit = true`: Returns the native Program Files (e.g., `C:\Program Files`)
/// - `install_64bit = false`: Returns Program Files (x86) (e.g., `C:\Program Files (x86)`)
pub fn program_files_dir(install_64bit: bool) -> PathBuf {
    let info = detect_system_info();
    if install_64bit && info.supports_64bit {
        info.program_files.clone()
    } else {
        info.program_files_x86
            .clone()
            .unwrap_or_else(|| info.program_files.clone())
    }
}

/// Get the appropriate Common Files directory for the install mode.
pub fn common_files_dir(install_64bit: bool) -> PathBuf {
    let info = detect_system_info();
    if install_64bit && info.supports_64bit {
        info.common_files.clone()
    } else {
        info.common_files_x86
            .clone()
            .unwrap_or_else(|| info.common_files.clone())
    }
}

/// Determine the default install mode based on system and target arch.
///
/// Returns true if the installer should use 64-bit mode.
pub fn default_install_mode(target_arch: &str) -> bool {
    let info = detect_system_info();
    match target_arch.to_lowercase().as_str() {
        "x64" | "x86_64" | "amd64" => info.supports_64bit,
        "arm64" | "aarch64" => info.os_arch == SystemArch::Arm64,
        _ => false,
    }
}

/// Resolve an `{autopf}` or `{autocf}` variable to the correct directory.
pub fn resolve_auto_path(variable: &str, install_64bit: bool) -> PathBuf {
    match variable {
        "{autopf}" => program_files_dir(install_64bit),
        "{autocf}" => common_files_dir(install_64bit),
        "{autodesktop}" => {
            // Use SHGetFolderPath or environment
            std::env::var("USERPROFILE")
                .map(|p| PathBuf::from(p).join("Desktop"))
                .unwrap_or_else(|_| PathBuf::from(r"C:\Users\Public\Desktop"))
        }
        "{windows}" | "{win}" => detect_system_info().windows_dir.clone(),
        "{sys}" | "{system}" => detect_system_info().system_dir.clone(),
        "{tmp}" | "{temp}" => std::env::temp_dir(),
        _ => PathBuf::from(variable),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_system_arch_display() {
        assert_eq!(SystemArch::X64.to_string(), "x64");
        assert_eq!(SystemArch::X86.to_string(), "x86");
        assert_eq!(SystemArch::Arm64.to_string(), "arm64");
    }

    #[test]
    fn test_detect_system_info() {
        let info = detect_system_info();
        // On any modern Windows, we should detect something
        assert!(info.os_arch == SystemArch::X64 || info.os_arch == SystemArch::X86 || info.os_arch == SystemArch::Arm64);
        assert!(info.program_files.to_string_lossy().len() > 0);
    }

    #[test]
    fn test_arch_compatibility_x86() {
        // x86 should be compatible with everything
        assert!(is_arch_compatible("x86"));
        assert!(is_arch_compatible("win32"));
        assert!(is_arch_compatible("32bit"));
    }

    #[test]
    fn test_arch_compatibility_unknown() {
        assert!(!is_arch_compatible("sparc"));
        assert!(!is_arch_compatible("mips"));
    }

    #[test]
    fn test_resolve_auto_path() {
        let pf = resolve_auto_path("{autopf}", true);
        assert!(pf.to_string_lossy().contains("Program Files"));

        let tmp = resolve_auto_path("{tmp}", true);
        assert!(tmp.to_string_lossy().len() > 0);
    }

    #[test]
    fn test_program_files_dir() {
        let dir_64 = program_files_dir(true);
        let dir_32 = program_files_dir(false);
        // Both should be valid paths
        assert!(dir_64.to_string_lossy().len() > 0);
        assert!(dir_32.to_string_lossy().len() > 0);
        // On a 64-bit system, they should differ
        if is_64bit_os() {
            assert_ne!(dir_64, dir_32);
        }
    }
}
