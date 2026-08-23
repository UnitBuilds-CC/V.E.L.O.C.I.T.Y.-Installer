# VELOCITY INSTALLER - ROBUSTNESS ASSESSMENT

**Date:** August 23, 2026  
**Status:** ⚠️ LIMITED ROBUSTNESS - SIZE CONSTRAINT IDENTIFIED

---

## Executive Summary

**Is it as simple as a 1-liner that generates a perfect installer every time, no matter the payload?**

**Answer: NO.** While the velocity-installer works perfectly for small to medium payloads (under 7MB), it has a **critical size limitation** that causes MSI corruption for larger payloads.

---

## Test Results Summary

### ✅ What Works

| Payload Size | Files | Result | Install Time | Notes |
|--------------|-------|--------|--------------|-------|
| 10 KB | 9 | ✅ SUCCESS | <1s | Sample app test |
| 1 MB | 1 | ✅ SUCCESS | <1s | Single binary |
| 5 MB | 1 | ✅ SUCCESS | <1s | Single binary |
| 6 MB | 1 | ✅ SUCCESS | <1s | Single binary |

**Features Working:**
- ✅ File installation and removal
- ✅ Component registration (GUIDs)
- ✅ Registry entries
- ✅ Shortcuts (start menu, desktop)
- ✅ Environment variables
- ✅ Custom actions
- ✅ Full uninstall with clean removal
- ✅ Special characters in filenames
- ✅ Empty files and nested directories
- ✅ Multiple components

### ❌ What Fails

| Payload Size | Files | Result | Error | Notes |
|--------------|-------|--------|-------|-------|
| 7 MB | 1 | ❌ FAIL | 1620 | MSI corruption |
| 8 MB | 1 | ❌ FAIL | 3 | MSI corruption |
| 10 MB | 2 | ❌ FAIL | 3 | MSI corruption |
| 50 MB | 5 | ❌ FAIL | 1620 | MSI corruption |
| 100 MB | 11 | ❌ FAIL | 1620 | MSI corruption |

**Error Codes:**
- **1620**: Fatal error during installation (MSI structure corrupted)
- **3**: Path not found (MSI cannot be opened)

---

## Root Cause Analysis

### The Problem

The custom OLE Compound File writer in velocity-msi **corrupts the MSI structure** when the file size exceeds approximately **6.5 MB**.

**Evidence:**
```
6MB MSI:  Header valid, installs successfully
7MB MSI:  Header corrupted (garbage bytes), error 1620
```

**Binary Analysis of 100MB MSI:**
```
Expected header: \xD0\xCF\x11\xE0\xA1\xB1\x1A\xE1
Actual header:   ??\x11\xE0\x1A\x00\x00\x00 (corrupted)
```

### Likely Cause

The OLE V3 format uses:
- **512-byte sectors** for the main file
- **64-byte mini-sectors** for small streams (<4096 bytes)
- **FAT (File Allocation Table)** for sector chain tracking

**Hypothesis:** The FAT chain or sector allocation logic has an integer overflow or buffer issue when handling files larger than ~6.5MB. This could be:

1. **FAT entry overflow**: 32-bit FAT entries may overflow at certain thresholds
2. **Sector chain corruption**: Long sector chains may not be properly terminated
3. **Mini-stream threshold**: Issues with the transition between mini-streams and regular streams
4. **Buffer size limits**: Fixed-size buffers may overflow with large payloads

---

## Practical Impact

### Who This Affects

**Small Applications (<5MB):** ✅ Safe to use
- Simple utilities
- Configuration tools
- Small desktop apps
- Documentation packages

**Medium Applications (5-7MB):** ⚠️ Borderline
- May work, may fail
- Not recommended for production

**Large Applications (>7MB):** ❌ Cannot use
- Full desktop applications
- Applications with large assets
- Multi-media packages
- Enterprise software

### Real-World Examples

**Would Work:**
- Notepad++ (3MB) ✅
- 7-Zip (1MB) ✅
- VS Code (initial installer ~80MB) ❌
- Chrome (~70MB) ❌
- Firefox (~50MB) ❌

**Would Fail:**
- Most commercial desktop applications
- Games with assets
- Development environments
- Applications with large DLLs or resources

---

## Comparison with Industry Standards

### Windows Installer (msiexec) Limits

**Official Microsoft Limits:**
- **Maximum MSI size:** 2 GB (theoretical)
- **Practical limit:** ~2 GB per cabinet
- **Recommended:** No hard limit, but test with actual payloads

**Our Implementation:**
- **Maximum MSI size:** ~6.5 MB (actual)
- **Failure mode:** Silent corruption
- **Detection:** Only at install time (too late)

### Other Installers

| Installer | Max Size | Notes |
|-----------|----------|-------|
| Inno Setup | 2 GB+ | Battle-tested, handles large payloads |
| NSIS | 2 GB+ | Used for major applications |
| WiX Toolset | 2 GB+ | Industry standard |
| **Velocity** | **~6.5 MB** | **Critical limitation** |

---

## Recommendations

### Immediate Actions

1. **Document the limitation clearly**
   - Add warning to README
   - Display error during build if payload >5MB
   - Prevent silent corruption

2. **Add size validation**
   ```rust
   if total_size > 6_000_000 {
       return Err("Payload too large (>6MB). Custom OLE writer limitation.".into());
   }
   ```

3. **Provide workaround**
   - Option to use external cabinet files
   - Option to use msi crate (with cfb dependency)
   - Split into multiple MSIs

### Long-Term Solutions

1. **Fix the OLE writer**
   - Audit FAT chain logic
   - Test with progressively larger files
   - Add comprehensive size tests

2. **Add compression**
   - Current zstd compression helps
   - But doesn't solve the root issue
   - Need to fix OLE writer regardless

3. **Consider alternatives**
   - Use msi crate for large payloads
   - Hybrid approach (custom for small, msi for large)
   - External cabinet files

---

## Test Methodology

### Test Environment
- **OS:** Windows 11 (25H2)
- **MSI Engine:** msiexec (Windows Installer)
- **Build:** Commit 785e51f
- **Compression:** zstd level 3

### Test Cases

1. **Small files** (10KB - 1MB)
   - Text files, small binaries
   - Multiple files, special characters

2. **Medium files** (1MB - 6MB)
   - Single large binary
   - Multiple components

3. **Large files** (7MB - 100MB+)
   - Single large binary
   - Multiple large binaries
   - Mixed payloads

### Verification Steps

1. Build MSI with `velocity build --format msi`
2. Install with `msiexec /i installer.msi /qn`
3. Check exit code and log
4. Verify files on disk
5. Uninstall with `msiexec /x {ProductCode} /qn`
6. Verify cleanup

---

## Conclusion

**Is the velocity-installer robust enough for production use?**

**For small applications (<5MB):** ✅ YES
- Works reliably
- Full feature set
- Clean install/uninstall

**For large applications (>7MB):** ❌ NO
- Critical size limitation
- Silent corruption
- Not suitable for production

**Overall Assessment:** ⚠️ **LIMITED**
- Excellent for small utilities and tools
- Not ready for commercial desktop applications
- Needs OLE writer fixes before general use

**Recommendation:** Use for small applications only, or wait for OLE writer fixes.

---

## Appendix: Test Data

### Successful Tests

**Sample App (13.5 KB, 9 files):**
```
Build: SUCCESS
Install: Exit code 0, 9 files installed
Uninstall: Exit code 0, all files removed
Components: 16 registered, 16 unregistered
```

**Small Test (10.5 KB, 10 files):**
```
Build: SUCCESS
Install: Exit code 0
Uninstall: Exit code 0
```

**1MB Test (1.02 MB, 1 file):**
```
Build: SUCCESS
Install: Exit code 0
Uninstall: Exit code 0
```

**5MB Test (5.05 MB, 1 file):**
```
Build: SUCCESS
Install: Exit code 0
Uninstall: Exit code 0
```

**6MB Test (6.06 MB, 1 file):**
```
Build: SUCCESS
Install: Exit code 0
Uninstall: Exit code 0
```

### Failed Tests

**7MB Test (7.07 MB, 1 file):**
```
Build: SUCCESS (no error)
Install: Exit code 1620
Log: "MainEngineThread is returning 1620"
Header: Corrupted
```

**8MB Test (8.1 MB, 1 file):**
```
Build: SUCCESS (no error)
Install: Exit code 3
Log: "MainEngineThread is returning 3"
```

**50MB Test (50.43 MB, 5 files):**
```
Build: SUCCESS (no error)
Install: Exit code 1620
Log: "MainEngineThread is returning 1620"
```

**100MB Test (100.85 MB, 11 files):**
```
Build: SUCCESS (no error)
Install: Exit code 1620
Log: "MainEngineThread is returning 1620"
Header: Corrupted (garbage bytes)
```

---

**Report Generated:** August 23, 2026  
**Tested By:** Automated robustness testing  
**Status:** Awaiting OLE writer fix for large payload support
