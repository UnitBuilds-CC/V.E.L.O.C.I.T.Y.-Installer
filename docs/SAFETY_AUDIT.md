# Unsafe Code Safety Audit

This document catalogs every `unsafe` block in the Velocity Installer codebase,
categorizes it by pattern, and documents the safety invariant that makes each usage sound.

## Summary

| Category | Count | Risk Level | Pattern |
|---|---|---|---|
| MessageBoxW calls | 11 | Low | UTF-16 encode → null-terminate → PCWSTR |
| COM initialization/usage | 3 | Medium | CoInitialize → CoCreateInstance → Release |
| Win32 Security (SID) | 1 | Medium | AllocateAndInitializeSid → CheckTokenMembership → FreeSid |
| Win32 Shell (ShellExecuteEx) | 1 | Medium | Struct init → call → close handle |
| WinHTTP networking | 2 | Medium | Session/connect/request lifecycle with cleanup |
| Named mutex | 3 | Medium | CreateMutex/OpenMutex → ReleaseMutex → CloseHandle |
| Environment broadcast | 1 | Low | SendMessageTimeoutW with static string |
| Known folder paths | 1 | Medium | SHGetKnownFolderPath → convert → CoTaskMemFree |
| Architecture detection | 1 | Medium | GetProcAddress → transmute → call |
| Win32 Window creation | 1 | High | CreateWindowExW + message loop + raw pointers |
| **Total** | **25** | | |

## Safety Invariants by Category

### 1. MessageBoxW Calls (11 blocks)

**Files:** `velocity-ui/src/classic.rs` (9), `velocity-ui/src/progress_dialog.rs` (2)

**Pattern:**
```rust
let text_w: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
unsafe { MessageBoxW(None, PCWSTR(text_w.as_ptr()), PCWSTR(title_w.as_ptr()), flags); }
```

**Safety invariant:**
- `Vec<u16>` owns its buffer; `.as_ptr()` is valid for the Vec's lifetime.
- `.chain(std::iter::once(0))` guarantees null-termination required by `PCWSTR`.
- The Vec outlives the `MessageBoxW` call (stack-allocated, not dropped until scope exit).
- `MessageBoxW` is a synchronous call — it returns before the Vec is dropped.
- No buffer mutation occurs during the call.

**Risk assessment:** Low. Well-understood Win32 API with deterministic behavior.

### 2. COM Initialization and IShellLink (3 blocks)

**Files:** `velocity-core/src/shortcuts.rs` (2), `velocity-ui/src/classic.rs` (1)

**Pattern:**
```rust
struct ComGuard;
impl ComGuard {
    fn new() -> Self { unsafe { let _ = CoInitialize(None).ok(); } ComGuard }
}
impl Drop for ComGuard { unsafe { CoUninitialize(); } }
```

**Safety invariant:**
- `ComGuard` uses RAII to guarantee `CoUninitialize()` is called on scope exit.
- `CoCreateInstance` returns COM smart pointers (`IShellLinkW`, `IPersistFile`) that manage their own refcount.
- COM interfaces are called through Rust's `windows` crate vtable dispatch, which validates the interface pointer.
- The `_com_guard` is declared before COM usage and dropped after, ensuring proper init/uninit ordering.

**Risk assessment:** Medium. COM lifetime is managed by Rust wrappers; `ComGuard` ensures apartment model correctness.

### 3. Win32 Security — SID Operations (1 block)

**File:** `velocity-core/src/elevation.rs` — `is_admin()`

**Pattern:**
```rust
unsafe {
    let mut admin_sid = PSID::default();
    AllocateAndInitializeSid(..., &mut admin_sid);
    CheckTokenMembership(None, admin_sid, &mut is_member);
    FreeSid(admin_sid);
}
```

**Safety invariant:**
- `AllocateAndInitializeSid` allocates a SID that must be freed with `FreeSid`.
- The SID is freed exactly once on all code paths (including early return on error).
- `CheckTokenMembership` does not take ownership of the SID — it borrows it.
- `PSID::default()` initializes to null, preventing double-free if allocation fails.

**Risk assessment:** Medium. Proper allocate/free pairing with no leak paths.

### 4. ShellExecuteExW (1 block)

**File:** `velocity-core/src/elevation.rs` — `shell_execute_elevated()`

**Safety invariant:**
- `SHELLEXECUTEINFOW` is initialized with `cbSize` set to `size_of::<SHELLEXECUTEINFOW>()`.
- All wide-string pointers (`lpVerb`, `lpFile`, `lpParameters`, `lpDirectory`) point to null-terminated `Vec<u16>` buffers that outlive the call.
- `SEE_MASK_NOCLOSEPROCESS` causes `hProcess` to be populated; we close it with `CloseHandle`.
- The `sei` struct is `mut` as required by the API.

**Risk assessment:** Medium. Standard elevation pattern with proper handle cleanup.

### 5. WinHTTP Session Lifecycle (2 blocks)

**File:** `velocity-core/src/downloader.rs`

**Pattern:**
```rust
unsafe {
    let session = WinHttpOpen(...);
    let connection = WinHttpConnect(session, ...);
    let request = WinHttpOpenRequest(connection, ...);
    WinHttpSendRequest(request, ...);
    WinHttpReceiveResponse(request, ...);
    // ... read data ...
    WinHttpCloseHandle(request);
    WinHttpCloseHandle(connection);
    WinHttpCloseHandle(session);
}
```

**Safety invariant:**
- Handles are closed in reverse order of creation (request → connection → session).
- Every error path closes all handles created up to that point.
- `PCWSTR` pointers are derived from stack-allocated `Vec<u16>` buffers that outlive the calls.
- No handle is used after being closed (linear lifecycle).

**Risk assessment:** Medium. The main risk is handle leaks on error paths; all paths have been verified to close handles.

### 6. Named Mutex Operations (3 blocks)

**File:** `velocity-core/src/installer_mutex.rs`

**Safety invariant:**
- `CreateMutexW` returns a handle that must be closed with `CloseHandle`.
- `Drop` impl calls `ReleaseMutex` then `CloseHandle` exactly once, guarded by null check.
- `OpenMutexW` in `is_another_running` always closes the returned handle.
- Mutex name is derived from sanitized input (alphanumeric + underscore only), preventing injection.

**Risk assessment:** Medium. RAII `Drop` ensures no handle leaks.

### 7. Environment Broadcast (1 block)

**File:** `velocity-core/src/env_vars.rs` — `broadcast_env_change()`

**Safety invariant:**
- `"Environment\0"` is a static string literal — always valid, always null-terminated.
- `SendMessageTimeoutW` with `SMTO_ABORTIFHUNG` and 5-second timeout prevents hanging.
- `HWND_BROADCAST` is a valid broadcast target.
- The `Vec<u16>` buffer outlives the synchronous `SendMessageTimeoutW` call.

**Risk assessment:** Low. Static string, synchronous call, timeout prevents hangs.

### 8. SHGetKnownFolderPath (1 block)

**File:** `velocity-core/src/shortcuts.rs` — `get_known_folder_path()`

**Safety invariant:**
- `SHGetKnownFolderPath` allocates memory via the COM allocator — must be freed with `CoTaskMemFree`.
- We manually walk the null-terminated UTF-16 string to determine length before creating a slice.
- `from_raw_parts` is called with the correct length (excluding null terminator).
- `CoTaskMemFree` is called after the data is copied into a Rust `String`/`PathBuf`.
- No use-after-free: the pointer is not accessed after `CoTaskMemFree`.

**Risk assessment:** Medium. Manual pointer walking is the highest-risk pattern here; `windows` crate's `PWSTR::to_string()` could be used instead for safety.

**Recommended improvement:** Replace manual pointer walk with `path_ptr.to_string()` from the `windows` crate to eliminate raw pointer manipulation.

### 9. Architecture Detection via GetProcAddress (1 block)

**File:** `velocity-core/src/arch_detect.rs`

**Pattern:**
```rust
let func = GetProcAddress(module, s!("IsWow64Process"));
let is_wow64_fn: unsafe extern "system" fn(HANDLE, *mut BOOL) -> BOOL = std::mem::transmute(func);
```

**Safety invariant:**
- `GetProcAddress` returns a function pointer that is valid for the module's lifetime.
- `kernel32.dll` is always loaded (it's the base Windows library).
- The `transmute` target signature matches the actual `IsWow64Process` signature exactly.
- `GetCurrentProcess` returns a pseudo-handle that doesn't need to be closed.
- This code only runs on 32-bit targets (`cfg(target_pointer_width = "32")`).

**Risk assessment:** Medium. `transmute` is inherently unsafe, but the function signature is well-documented by Microsoft and matches exactly.

### 10. Win32 Window Creation and Message Loop (1 block)

**File:** `velocity-ui/src/native_wizard.rs` — `run_wizard_window()`

**Pattern:**
```rust
unsafe {
    InitCommonControlsEx(&icc);
    // RegisterClassW, CreateWindowExW, ShowWindow, message loop
    // Box::into_raw(data) stored as GWLP_USERDATA
    // Box::from_raw(data) retrieved in WndProc
}
```

**Safety invariant:**
- `INITCOMMONCONTROLSEX.cbSize` is set correctly for the struct.
- Window data (`WizardData`) is heap-allocated via `Box::into_raw` and stored in `GWLP_USERDATA`.
- The raw pointer is reclaimed via `Box::from_raw` in `WM_DESTROY` handler, preventing leaks.
- All child window handles are stored in the `WizardData` struct and become invalid when the window is destroyed (which is correct — they're not used after).
- The message loop runs on the same thread that created the window (single-threaded apartment).

**Risk assessment:** High. This is the most complex unsafe block. The raw pointer handoff via `GWLP_USERDATA` requires careful lifetime management.

**Recommended improvement:** Consider using `SetWindowLongPtrW` with proper typed wrappers, or migrate to a framework that abstracts Win32 window management.

## Recommendations

1. **SHGetKnownFolderPath** — Replace manual pointer walk with `windows::core::PWSTR::to_string()`.
2. **native_wizard.rs** — The `Box::into_raw`/`Box::from_raw` pattern should be documented with a comment at the reclamation site.
3. **arch_detect.rs** — The `transmute` could be replaced with `std::mem::transmute_copy` or a union, but the current usage is correct and well-bounded.
4. **Consider `windows` crate safe wrappers** — Many of these unsafe blocks exist because the `windows` crate requires `unsafe` for Win32 calls. As the crate adds safe wrappers (e.g., `MessageBox` free function), migrate to them.

## Audit Metadata

- **Auditor:** Automated review
- **Date:** 2026-08-17
- **Total unsafe blocks:** 25
- **Files affected:** 9
- **Critical findings:** 0 (all blocks have valid safety invariants)
- **Improvement recommendations:** 2 (SHGetKnownFolderPath, native_wizard raw pointer docs)
