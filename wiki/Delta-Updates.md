# Delta Updates

Velocity Installer supports **binary delta updates** that transfer only the changes between versions, reducing update sizes by 80-95% compared to full packages.

## Overview

### How It Works

Delta updates use **bsdiff** for true binary diffing with **Zstd** compression on top:

```
Build-time: v1.0.0 + v1.0.1 → Delta Generator → v1.0.1-delta.zip
Runtime:    Current install + delta.zip → Delta Applier → Reconstructed v1.0.1
```

### Key Features

- **bsdiff binary patching** — True binary diff algorithm producing patches 80-95% smaller than full files
- **Zstd compression** — Additional compression layer on top of bsdiff patches
- **SHA256 checksum verification** — Every patch verified before and after application
- **Path traversal protection** — All delta paths validated against directory traversal attacks
- **Atomic rollback** — Crash-safe rename-based rollback on any failure
- **File locking** — Exclusive lock prevents concurrent update corruption
- **Disk space verification** — Checks available space before applying (requires 2x install size)
- **Version verification** — Fast-fails if delta's expected version doesn't match installed version
- **Package size limit** — Rejects packages over 2 GB to prevent OOM
- **Multi-hop updates** — Chain multiple deltas (v1.0.0 → v1.0.1 → v1.0.2)
- **Smart heuristic** — Automatically chooses delta vs full download based on size

---

## Delta Package Format

Delta packages use the `.delta.zip` format:

```
delta.zip/
  manifest.json       # File list, checksums, version info
  patches/
    file1.exe.patch   # Zstd-compressed binary patch
    file2.dll.patch
  new/
    file3.txt         # New files (full content, Zstd-compressed)
```

### Manifest Structure

```json
{
  "from_version": "1.0.0",
  "to_version": "1.0.1",
  "patches": [
    {
      "type": "Modified",
      "path": "app.exe",
      "old_checksum": "sha256:abc...",
      "new_checksum": "sha256:def...",
      "patch_data": "<base64>",
      "new_size": 1048576
    },
    {
      "type": "Added",
      "path": "new_feature.dll",
      "checksum": "sha256:ghi...",
      "content": "<base64>",
      "size": 524288
    },
    {
      "type": "Deleted",
      "path": "old_file.txt",
      "checksum": "sha256:jkl..."
    }
  ],
  "total_patch_size": 102400,
  "created_at": "2025-01-15T10:30:00Z"
}
```

### Patch Types

| Type | Description | Contents |
|------|-------------|----------|
| **Modified** | File changed between versions | bsdiff patch + Zstd (falls back to full Zstd if patch is larger) |
| **Added** | New file in the update | Full file content (Zstd-compressed) |
| **Deleted** | File removed in the update | Checksum only (for verification) |

---

## Generating Delta Updates

### CLI Usage

```bash
# Build with delta generation
velocity build --delta

# With custom compression
velocity build --delta --compression 15

# Specify output directory
velocity build --delta --output ./releases/
```

The `--delta` flag requires the previous version's extracted contents to be available in the output directory. Velocity automatically detects the previous version and generates the delta.

### Programmatic Usage

```rust
use velocity_core::delta::{generate_delta, DeltaOptions, save_delta_package};

let options = DeltaOptions {
    compression_level: 9,  // Zstd level (1-22)
    min_patch_size: 1024,  // Skip patching for files < 1KB
    max_file_size: 2_147_483_648, // 2GB limit
};

let delta = generate_delta(
    Path::new("releases/v1.0.0"),
    Path::new("releases/v1.0.1"),
    "1.0.0",
    "1.0.1",
    &options,
)?;

save_delta_package(&delta, Path::new("releases/v1.0.1-delta.zip"))?;
```

---

## Runtime Update Process

The update manager automatically decides between delta and full updates:

```rust
// Heuristic: use delta if total delta size < 70% of full package
if sum(delta_sizes) <= 0.7 * full_size && hops <= 5 {
    download_deltas()
} else {
    download_full_package()
}
```

### Update Check Response

The update endpoint returns delta information alongside the full package:

```json
{
  "version": "1.0.1",
  "download_url": "https://releases.example.com/v1.0.1/installer.exe",
  "release_notes": "Bug fixes and improvements",
  "delta": {
    "url": "https://releases.example.com/v1.0.1-delta.zip",
    "size": 102400,
    "full_size": 2264583,
    "hops": 0
  }
}
```

---

## Multi-hop Updates

When multiple intermediate versions exist, Velocity chains deltas:

```
v1.0.0 → v1.0.1 → v1.0.2 → v1.0.3
  [delta1]  [delta2]  [delta3]
```

### Constraints

- **Maximum hops:** 5 (falls back to full download beyond this)
- **Chain continuity:** Each delta's `to_version` must match the next delta's `from_version`
- **Sequential application:** Deltas are applied in order with verification at each step

### Example

```rust
use velocity_core::delta::{load_delta_package, apply_delta_chain};

let delta1 = load_delta_package(Path::new("v1.0.1-delta.zip"))?;
let delta2 = load_delta_package(Path::new("v1.0.2-delta.zip"))?;
let delta3 = load_delta_package(Path::new("v1.0.3-delta.zip"))?;

apply_delta_chain(
    &[delta1, delta2, delta3],
    Path::new("C:/Program Files/MyApp"),
)?;
```

---

## Rollback on Failure

Every delta application uses **atomic rename-based** backup and rollback:

1. **Lock** — Acquire exclusive file lock (prevents concurrent updates)
2. **Verify space** — Check disk has 2x install size available
3. **Backup** — Current installation atomically renamed to `.backup` (crash-safe)
4. **Apply** — Patches applied with checksum verification (bsdiff or Zstd per file)
5. **Verify** — Each patched file verified (SHA256 + size check)
6. **Commit** — On success, backup is removed and lock released
7. **Rollback** — On any failure, backup atomically restored (crash-safe)

---

## Benchmark

Typical results for a 50MB application:

| Update Type | Size | Reduction |
|-------------|------|-----------|
| Full package | 50 MB | — |
| Delta (minor update) | 2-5 MB | 90-96% |
| Delta (major update) | 10-20 MB | 60-80% |
| Multi-hop (3 versions) | 5-15 MB | 70-90% |

### Factors Affecting Delta Size

- **Binary files** — Highest compression ratio (bsdiff finds byte-level similarities)
- **Text/config files** — Moderate compression ratio
- **Compressed assets** — Lower ratio (images, video already compressed)
- **New files** — Included at full size (compressed)
- **Unchanged files** — Excluded entirely (zero cost)

---

## Configuration

Delta settings can be configured in `velocity.toml`:

```toml
[files.compression]
format = "zstd"
level = 9  # Higher compression for smaller deltas
```

### DeltaOptions Reference

| Option | Default | Description |
|--------|---------|-------------|
| `compression_level` | 9 | Zstd level (1-22). Higher = smaller deltas, slower generation |
| `min_patch_size` | 1024 | Files smaller than this use full content instead of patches |
| `max_file_size` | 2GB | Files larger than this use full content (Zstd limit) |

---

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                  Build Pipeline                              │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│  v1.0.0 ──┐                                                 │
│            ├─→ Delta Generator ─→ .delta.zip                │
│  v1.0.1 ──┘                                                 │
│                                                              │
└─────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────┐
│                 Runtime Update                               │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│  Update Server ─→ DeltaInfo ─→ Heuristic                    │
│                                    │                         │
│                        ┌───────────┤                         │
│                        ▼           ▼                         │
│                   Delta Path   Full Path                     │
│                        │           │                         │
│                        ▼           ▼                         │
│                  Apply Delta  Download Full                  │
│                  (with backup)  Installer                    │
│                                                              │
└─────────────────────────────────────────────────────────────┘
```

---

## Error Handling

| Error | Cause | Recovery |
|-------|-------|----------|
| Checksum mismatch | Corrupted download or tampered file | Automatic rollback, retry download |
| File not found | Installation directory modified externally | Automatic rollback, fall back to full update |
| Size mismatch | Patch produced unexpected output | Automatic rollback, fall back to full update |
| Chain broken | Missing intermediate delta version | Fall back to full update |
| Delta too large | Delta exceeds 70% of full size | Automatically use full update instead |

---

## Security

Delta updates include multiple layers of protection:

| Protection | Description |
|------------|-------------|
| **Path traversal** | All paths validated via `validate_relative_path()` — rejects `../`, absolute paths, null bytes |
| **File locking** | Exclusive `fs2` lock prevents concurrent update corruption |
| **Disk space** | Verified before apply (requires 2x install size + delta size) |
| **Version check** | Fast-fails if delta's `from_version` doesn't match installed version |
| **Package size limit** | `MAX_DELTA_PACKAGE_SIZE` (2 GB) prevents OOM from malicious packages |
| **Download integrity** | Downloaded size verified against server-reported size |
| **Per-file checksums** | SHA256 verified before AND after patching |
| **Atomic rollback** | Rename-based crash recovery — install is never left in a half-applied state |

---

## Limitations

- Delta generation requires both old and new version directories
- Multi-hop updates limited to 5 intermediate versions
- Delta packages are platform-specific (Windows deltas won't apply to Linux)
- Small files (< 1 KB) use full Zstd content instead of bsdiff patches (overhead not worth it)

---

## Further Reading

- [[Cloud-Fetch-Installers]] — Cloud-fetch bootstrapper installers
- [[Security]] — Encryption and security features
- [[Architecture]] — System design and crate structure
