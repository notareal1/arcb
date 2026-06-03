# ARCB — Adaptive Range Coding for Base-10 Digits

[![Rust](https://img.shields.io/badge/rust-1.85%2B-orange)](https://www.rust-lang.org)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue)](LICENSE)

**ARCB** is a lossless compression library specialised for decimal digit strings
(`0`–`9`). It approaches the theoretical entropy limit of **~3.322 bits/digit**
even on uniformly random data, beating general-purpose compressors like gzip on
this specific domain.

## Quick Start

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

## Building

```bash
git clone https://github.com/notareal1/arcb.git
cd arcb
cargo build --release
cargo test
```

## CLI Usage

```bash
# Compress
arcb compress input.txt -o output.arcb

# Decompress
arcb decompress output.arcb -o recovered.txt

# Show statistics
arcb stats input.txt
```

## API

### Encoding

```rust
use arcb::{ArcbEncoder, CompressOptions, encode_to_binary, encode_to_base64};

// Per-block API
let mut enc = ArcbEncoder::new();
enc.push_digit(5);
enc.push_digit(9);
let block = enc.encode_block();

// With Small compression enabled
let mut enc = ArcbEncoder::with_options(
    CompressOptions::new().with_compress_small(true),
);

// File-level API
let binary = encode_to_binary("1234567890").unwrap();
let b64 = encode_to_base64("1234567890").unwrap();
```

### Decoding

```rust
use arcb::{decode_block, decode_from_binary, decode_from_base64};

let digits = decode_block(&compressed).unwrap();
let text = decode_from_binary(&binary).unwrap();
let text = decode_from_base64(&b64).unwrap();
```

## Compression Performance

Measured on 100 000 digits, single thread, release mode:

| Data pattern | ARCB bits/digit | gzip -9 | ARCB vs gzip |
|---|---|---|---|
| Uniform random 0-9 | 3.33 | 3.92 | **-15%** |
| All zeros | 0.001 | 0.07 | **-99%** |
| All same digit | 0.001 | 0.07 | **-99%** |
| Biased 70/30 | 2.89 | 3.41 | **-15%** |
| Repeating 0-9 | 0.40 | 0.01 | gzip wins (LZ77) |

ARCB excels on random or semi-random digit data. For highly patterned data
(e.g. repeated sequences), gzip's LZ77 dictionary coding is superior.

## How It Works

ARCB splits each decimal digit into two groups:

* **Small** (0–7): 8 values → encoded with adaptive 8-symbol range coder
* **Large** (8, 9): 2 values → bit mask + designation compressed with binary-range coders

The Small/Large mask itself is compressed with an adaptive binary-range coder.
See [THEORY.md](THEORY.md) for the full mathematical treatment.

## Running Benchmarks

```bash
cargo bench
```

## License

MIT — see [LICENSE](LICENSE).
