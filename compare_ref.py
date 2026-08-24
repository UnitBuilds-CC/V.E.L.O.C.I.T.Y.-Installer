"""Compare the Python msilib reference MSI with velocity-msi output.
Reads both MSIs using the msi crate (via Rust) and compares streams."""
import struct
import os
import sys

def read_ole_streams(path):
    """Read an OLE file and extract all streams with their data."""
    with open(path, 'rb') as f:
        data = f.read()
    
    # Verify magic
    magic = data[:8]
    assert magic == bytes([0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1]), f"Not an OLE file: {path}"
    
    # Read header
    major_ver = struct.unpack_from('<H', data, 26)[0]
    sector_shift = struct.unpack_from('<H', data, 30)[0]
    mini_sector_shift = struct.unpack_from('<H', data, 32)[0]
    sector_size = 1 << sector_shift
    mini_sector_size = 1 << mini_sector_shift
    mini_stream_cutoff = struct.unpack_from('<I', data, 56)[0]
    num_fat_sectors = struct.unpack_from('<I', data, 44)[0]
    first_dir_sector = struct.unpack_from('<I', data, 48)[0]
    first_minifat_sector = struct.unpack_from('<I', data, 60)[0]
    num_minifat_sectors = struct.unpack_from('<I', data, 64)[0]
    
    # Read DIFAT array from header
    difat = []
    for i in range(109):
        val = struct.unpack_from('<I', data, 76 + i * 4)[0]
        if val != 0xFFFFFFFE:  # Not FREE_SECT
            difat.append(val)
    
    # Helper to read sector data
    def sector_offset(s):
        return 512 + s * sector_size
    
    # Read FAT
    fat = []
    for fat_sector in difat:
        off = sector_offset(fat_sector)
        for j in range(sector_size // 4):
            fat.append(struct.unpack_from('<I', data, off + j * 4)[0])
    
    # Follow sector chain
    def follow_chain(start):
        chain = []
        current = start
        while current != 0xFFFFFFFE and current != 0xFFFFFFFF:
            chain.append(current)
            current = fat[current]
        return chain
    
    # Read directory entries
    dir_chain = follow_chain(first_dir_sector)
    dir_data = b''
    for s in dir_chain:
        dir_data += data[sector_offset(s):sector_offset(s) + sector_size]
    
    entries = []
    for i in range(len(dir_data) // 128):
        entry = dir_data[i * 128:(i + 1) * 128]
        name_len = struct.unpack_from('<H', entry, 64)[0]
        if name_len == 0:
            continue
        name_bytes = name_len - 2  # exclude null terminator
        name = entry[:name_bytes].decode('utf-16-le', errors='replace')
        obj_type = entry[66]
        left = struct.unpack_from('<i', entry, 68)[0]
        right = struct.unpack_from('<i', entry, 72)[0]
        child = struct.unpack_from('<i', entry, 76)[0]
        start_sect = struct.unpack_from('<I', entry, 116)[0]
        stream_size = struct.unpack_from('<Q', entry, 120)[0]
        entries.append({
            'name': name,
            'obj_type': obj_type,
            'left': left,
            'right': right,
            'child': child,
            'start': start_sect,
            'size': stream_size,
        })
    
    # Root entry
    root = entries[0]
    mini_stream_start = root['start']
    mini_stream_size = root['size']
    
    # Read mini-stream data
    mini_chain = follow_chain(mini_stream_start)
    mini_data = b''
    for s in mini_chain:
        mini_data += data[sector_offset(s):sector_offset(s) + sector_size]
    mini_data = mini_data[:mini_stream_size]
    
    # Read MiniFAT
    minifat_chain = follow_chain(first_minifat_sector)
    minifat = []
    for s in minifat_chain:
        off = sector_offset(s)
        for j in range(sector_size // 4):
            minifat.append(struct.unpack_from('<I', data, off + j * 4)[0])
    
    # Read stream data
    streams = {}
    for entry in entries[1:]:  # skip root
        name = entry['name']
        size = entry['size']
        if size == 0:
            streams[name] = b''
            continue
        
        if size < mini_stream_cutoff:
            # Mini stream
            mini_offset = entry['start'] * mini_sector_size
            # Follow mini FAT chain
            current = entry['start']
            stream_data = b''
            while current != 0xFFFFFFFE and current != 0xFFFFFFFF:
                off = current * mini_sector_size
                remaining = size - len(stream_data)
                to_read = min(mini_sector_size, remaining)
                stream_data += mini_data[off:off + to_read]
                current = minifat[current] if current < len(minifat) else 0xFFFFFFFE
            streams[name] = stream_data[:size]
        else:
            # Regular stream
            chain = follow_chain(entry['start'])
            stream_data = b''
            for s in chain:
                off = sector_offset(s)
                remaining = size - len(stream_data)
                to_read = min(sector_size, remaining)
                stream_data += data[off:off + to_read]
            streams[name] = stream_data[:size]
    
    return streams

def hex_dump(data, max_bytes=256):
    """Pretty hex dump of first max_bytes."""
    lines = []
    for i in range(0, min(len(data), max_bytes), 16):
        chunk = data[i:i+16]
        hex_str = ' '.join(f'{b:02x}' for b in chunk)
        ascii_str = ''.join(chr(b) if 32 <= b < 127 else '.' for b in chunk)
        lines.append(f'  {i:04x}: {hex_str:<48s} {ascii_str}')
    if len(data) > max_bytes:
        lines.append(f'  ... ({len(data)} total bytes)')
    return '\n'.join(lines)

def compare_streams(ref_path, our_path):
    """Compare streams between reference and our MSI."""
    print(f"Reference: {ref_path} ({os.path.getsize(ref_path)} bytes)")
    print(f"Our MSI:   {our_path} ({os.path.getsize(our_path)} bytes)")
    
    ref_streams = read_ole_streams(ref_path)
    our_streams = read_ole_streams(our_path)
    
    print(f"\nReference streams: {len(ref_streams)}")
    print(f"Our streams:       {len(our_streams)}")
    
    # List all stream names
    ref_names = set(ref_streams.keys())
    our_names = set(our_streams.keys())
    
    print(f"\n--- Stream names ---")
    only_ref = ref_names - our_names
    only_ours = our_names - ref_names
    common = ref_names & our_names
    
    if only_ref:
        print(f"\nOnly in reference ({len(only_ref)}):")
        for n in sorted(only_ref):
            print(f"  {repr(n)} ({len(ref_streams[n])} bytes)")
    
    if only_ours:
        print(f"\nOnly in our MSI ({len(only_ours)}):")
        for n in sorted(only_ours):
            print(f"  {repr(n)} ({len(our_streams[n])} bytes)")
    
    print(f"\nCommon streams: {len(common)}")
    
    # Compare common streams
    print(f"\n--- Stream comparison ---")
    for name in sorted(common):
        ref_data = ref_streams[name]
        our_data = our_streams[name]
        
        if ref_data == our_data:
            print(f"\n  {repr(name)}: IDENTICAL ({len(ref_data)} bytes)")
        else:
            print(f"\n  {repr(name)}: DIFFERENT")
            print(f"    Reference: {len(ref_data)} bytes")
            print(f"    Our:       {len(our_data)} bytes")
            
            # Find first difference
            min_len = min(len(ref_data), len(our_data))
            for i in range(min_len):
                if ref_data[i] != our_data[i]:
                    print(f"    First diff at byte {i} (0x{i:x})")
                    print(f"    Reference: {hex_dump(ref_data[max(0,i-8):i+32])}")
                    print(f"    Our:       {hex_dump(our_data[max(0,i-8):i+32])}")
                    break
            else:
                if len(ref_data) != len(our_data):
                    print(f"    Same content up to byte {min_len}, but different lengths")
                else:
                    print(f"    ERROR: Same length but different content, no diff found?!")
    
    # Show streams only in reference
    for name in sorted(only_ref):
        print(f"\n  {repr(name)} (reference only): {len(ref_streams[name])} bytes")
        print(hex_dump(ref_streams[name]))

if __name__ == '__main__':
    ref = "python_ref.msi"
    our = "velocity_comp.msi"
    
    if not os.path.exists(ref):
        print(f"Reference MSI not found: {ref}")
        sys.exit(1)
    if not os.path.exists(our):
        print(f"Our MSI not found: {our}")
        print("Build velocity-msi output first (run the Rust test)")
        sys.exit(1)
    
    compare_streams(ref, our)
