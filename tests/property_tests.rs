//! Property-based tests for ARCB compression.
//!
//! Generates 1000 random blocks of varying sizes and verifies roundtrip,
//! with both raw Small and compressed Small modes.

use arcb::{ArcbEncoder, CompressOptions, decode_block};
use rand::Rng;

fn assert_roundtrip(digits: &[u8]) {
    let mut enc = ArcbEncoder::new();
    for &d in digits {
        enc.push_digit(d);
    }
    let compressed = enc.encode_block();
    let decoded = decode_block(&compressed).expect("decode failed");
    assert_eq!(
        decoded, digits,
        "roundtrip mismatch for input of length {}",
        digits.len()
    );
}

fn assert_roundtrip_with_opts(digits: &[u8], opts: &CompressOptions) {
    let mut enc = ArcbEncoder::with_options(opts.clone());
    for &d in digits {
        enc.push_digit(d);
    }
    let compressed = enc.encode_block();
    let decoded = decode_block(&compressed).expect("decode failed");
    assert_eq!(
        decoded, digits,
        "roundtrip mismatch (opts={opts:?}) for input of length {}",
        digits.len()
    );
}

#[test]
fn property_1000_random_blocks_raw_small() {
    let mut rng = rand::thread_rng();
    for i in 0..1000 {
        let len = rng.gen_range(0..=65535);
        let digits: Vec<u8> = (0..len).map(|_| rng.gen_range(0..10)).collect();
        assert_roundtrip(&digits);
        if i % 250 == 0 {
            eprintln!("  raw_small: passed {i}/1000 (last len={len})");
        }
    }
}

#[test]
fn property_1000_random_blocks_compressed_small() {
    let mut rng = rand::thread_rng();
    let opts = CompressOptions::new().with_compress_small(true);
    for i in 0..1000 {
        let len = rng.gen_range(0..=65535);
        let digits: Vec<u8> = (0..len).map(|_| rng.gen_range(0..10)).collect();
        assert_roundtrip_with_opts(&digits, &opts);
        if i % 250 == 0 {
            eprintln!("  compressed_small: passed {i}/1000 (last len={len})");
        }
    }
}

#[test]
fn property_boundary_sizes() {
    let sizes = [
        0, 1, 2, 3, 7, 8, 15, 16, 31, 32, 63, 64, 127, 128, 255, 256, 511, 512, 1023, 1024,
        4095, 4096, 8191, 8192, 16383, 16384, 32767, 32768, 65534, 65535,
    ];
    for &size in &sizes {
        let mut rng = rand::thread_rng();
        let digits: Vec<u8> = (0..size).map(|_| rng.gen_range(0..10)).collect();
        assert_roundtrip(&digits);
        assert_roundtrip_with_opts(&digits, &CompressOptions::new().with_compress_small(true));
    }
}

#[test]
fn property_all_same_digit() {
    for digit in 0..=9 {
        for size in [1, 10, 100, 1000, 10000] {
            let digits = vec![digit; size];
            assert_roundtrip(&digits);
            assert_roundtrip_with_opts(&digits, &CompressOptions::new().with_compress_small(true));
        }
    }
}

#[test]
fn property_small_compression_ratio_skewed() {
    let mut rng = rand::thread_rng();
    let digits: Vec<u8> = (0..10000)
        .map(|_| {
            if rng.gen_bool(0.9) {
                0
            } else {
                rng.gen_range(0..8)
            }
        })
        .collect();

    let mut enc_raw = ArcbEncoder::new();
    let mut enc_compressed =
        ArcbEncoder::with_options(CompressOptions::new().with_compress_small(true));
    for &d in &digits {
        enc_raw.push_digit(d);
        enc_compressed.push_digit(d);
    }
    let raw = enc_raw.encode_block();
    let compressed = enc_compressed.encode_block();

    assert_eq!(decode_block(&raw).unwrap(), digits);
    assert_eq!(decode_block(&compressed).unwrap(), digits);

    eprintln!(
        "  skewed: raw_size={}, compressed_size={}",
        raw.len(),
        compressed.len()
    );
}
