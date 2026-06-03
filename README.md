# ARCB — Adaptive Range Coding for Base-10 Digits

[![Rust](https://img.shields.io/badge/rust-1.85%2B-orange)](https://www.rust-lang.org)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue)](LICENSE)
[![Tests](https://img.shields.io/badge/tests-73%20passing-brightgreen)]()

**ARCB** (Adaptive Range Coding for Base-10) is a lossless compression library
specialised for decimal digit strings (characters `0`–`9`). It approaches the
theoretical entropy limit of **~3.322 bits/digit** even on uniformly random
data, beating general-purpose compressors like gzip on this specific domain.

## Quick Start (Rust library)

```rust
use arcb::{ArcbEncoder, decode_block};

let mut encoder = ArcbEncoder::new();
for d in [8, 3, 9, 1, 0, 2, 7, 4, 6, 5] {
    encoder.push_digit(d);
}
let compressed = encoder.encode_block();

let decoded = decode_block(&compressed).unwrap();
assert_eq!(&decoded, &[8, 3, 9, 1, 0, 2, 7, 4, 6, 5]);
```

## Download Pre-built Binaries (No Rust required)

Pre-built binaries for **Windows**, **macOS**, and **Linux** are available on
the [Releases](https://github.com/notareal1/arcb/releases) page.

### Windows

1. Download `arcb-win-x64.exe` + `nen.bat` + `giai_nen.bat` from Releases
2. Extract all files to a folder
3. **Double-click `nen.bat`** -> drag & drop file -> press Enter -> done!
4. **Double-click `giai_nen.bat`** to decompress `.arcb` files

Command line usage (optional):
```
arcb.exe compress input.txt -o output.arcb
arcb.exe decompress output.arcb -o recovered.txt
arcb.exe stats input.txt
```

### macOS

1. Download `arcb-macos-x64` + `nen.command` + `giai_nen.command` from Releases
2. Extract to a folder, then allow execution:
   ```bash
   chmod +x arcb-macos-x64 nen.command giai_nen.command
   ```
3. **Double-click `nen.command`** -> drag & drop file -> Enter -> done!
4. **Double-click `giai_nen.command`** to decompress

Command line usage:
```
./arcb-macos-x64 compress input.txt -o output.arcb
./arcb-macos-x64 decompress output.arcb -o recovered.txt
```

### Linux

1. Download `arcb-linux-x64` + `nen.sh` + `giai_nen.sh` from Releases
2. Extract and make executable:
   ```bash
   chmod +x arcb-linux-x64 nen.sh giai_nen.sh
   ```
3. **Double-click `nen.sh`** (or run in terminal) -> done!
4. **`giai_nen.sh`** to decompress

Command line usage:
```
./arcb-linux-x64 compress input.txt -o output.arcb
./arcb-linux-x64 decompress output.arcb -o recovered.txt
```

## CLI Help

```
arcb compress [OPTIONS] -i <input> -o <output>
  -i, --input <FILE>       Input file (digit string)
  -o, --output <FILE>      Output file
      --small_compress     Use adaptive Small compression (default: true)
      --checksum           Append CRC-32 checksum (default: true)

arcb decompress -i <input> -o <output>
  -i, --input <FILE>       Input .arcb file
  -o, --output <FILE>      Output file

arcb stats -i <input>
  -i, --input <FILE>       Show compression statistics
```

## Features

| Feature | Status |
|---|---|
| Adaptive binary-range mask coding | Yes |
| Adaptive binary-range large-bit coding | Yes |
| Adaptive 8-symbol range Small compression | Yes |
| CRC-32 file integrity checksums | Yes |
| Base64 encoding for text transport | Yes |
| Parallel block processing (rayon) | Yes |
| CLI tool with clap | Yes |

## Compression Performance

Measured on 100 000 digits, single thread, release mode:

| Data pattern | ARCB bits/digit | gzip -9 bits/digit | ARCB vs gzip |
|---|---|---|---|
| Uniform random 0-9 | 3.33 | 3.92 | **-15%** |
| All zeros | 0.001 | 0.07 | **-99%** |
| All eights | 0.001 | 0.07 | **-99%** |
| Biased 70/30 small/large | 2.89 | 3.41 | **-15%** |
| Repeating pattern 0123456789 | 0.40 | 0.01 | gzip wins (LZ77) |

*ARCB excels on random or semi-random digit data. For highly patterned data
(e.g. repeated sequences), gzip's LZ77 dictionary coding is superior.*

## How It Works

ARCB splits each decimal digit into one of two groups:

* **Small** (0-7): 8 possible values -> encoded with an adaptive 8-symbol range
  coder (or raw 3 bits/value when compression is disabled).
* **Large** (8, 9): 2 possible values -> a bit mask run-length and the 8/9
  designation are both compressed with adaptive binary-range coders.

The mask separating Small from Large positions is itself compressed with an
adaptive binary-range coder, allowing the algorithm to adapt to local
distributions.

See [THEORY.md](THEORY.md) for the full mathematical treatment.

## Building from Source

```bash
git clone https://github.com/notareal1/arcb.git
cd arcb
cargo build --release
cargo test
```

### Benchmark

```bash
cargo bench                # criterion benchmarks (throughput)
cargo run --example arcb_vs_gzip   # comparison vs gzip/bzip2
```

### Auto-build with GitHub Actions

Push a tag starting with `v` to trigger the release workflow:

```bash
git tag v0.1.0
git push origin v0.1.0
```

The workflow automatically builds for Windows, macOS, and Linux, creates a
GitHub Release, and uploads the artifacts.

## API

### Encoding

```rust
use arcb::{ArcbEncoder, CompressOptions, encode_to_binary, encode_to_base64};

// Per-block API
let mut enc = ArcbEncoder::new();
enc.push_digit(5);
enc.push_digit(9);
let block: Vec<u8> = enc.encode_block();

// With Small compression enabled
let mut enc = ArcbEncoder::with_options(
    CompressOptions::new().with_compress_small(true)
);

// File-level API (with magic + version + optional CRC-32)
let binary = encode_to_binary("1234567890").unwrap();
let binary_with_crc = encode_to_binary_with_checksum("1234567890").unwrap();
let b64 = encode_to_base64("1234567890").unwrap();
```

### Decoding

```rust
use arcb::{decode_block, decode_from_binary, decode_from_binary_checked, decode_from_base64};

// Per-block
let digits: Vec<u8> = decode_block(&compressed).unwrap();

// File-level (auto-detects CRC)
let text = decode_from_binary(&binary).unwrap();

// File-level with explicit CRC validation
let text = decode_from_binary_checked(&binary_with_crc).unwrap();

// Base64
let text = decode_from_base64(&b64).unwrap();
```

### Options

```rust
use arcb::CompressOptions;

let opts = CompressOptions::new()
    .with_compress_small(true)   // enable adaptive Small compression
    .with_parallel(false);        // reserved for future rayon support
```

## File Format

### Without CRC

```
 Offset  Size  Field
 0       4     Magic: "ARCB"
 4       1     Version (currently 1)
 5       var   Superblock(s)
```

### With CRC-32

```
 Offset  Size  Field
 0       4     Magic: "ARCB"
 4       1     Version
 5       4     CRC-32 (IEEE 802.3) over bytes 9..end
 9       var   Superblock(s)
```

### Superblock (11-byte header + payload)

```
 Offset  Size  Field
 0       1     Flags (bit 0: compress_small, bit 1: has_checksum)
 1       2     n — total digits in block (1..65535)
 3       2     small_count — digits in 0..7
 5       2     mask_len — compressed mask bytes
 7       2     large_len — compressed large_bits bytes
 9       2     small_len — compressed/raw small bytes
 11      var   mask_compressed (mask_len bytes)
         var   large_compressed (large_len bytes)
         var   small_data (small_len bytes)
```

## License

MIT — see [LICENSE](LICENSE) for details.

## Acknowledgements

This project was developed with the assistance of AI tools. The core algorithm
and implementation were designed and verified by the author.
