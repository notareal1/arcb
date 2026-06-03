//! Integration tests for the full ARCB compression/decompression pipeline.
//!
//! Covers: basic roundtrip, empty block, random data, edge cases (all 8s, all 9s,
//! all 0-7), encoder reuse, and corruption detection.

use arcb::{ArcbEncoder, ArcbError, decode_block};
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

#[test]
fn empty_block() {
    let mut enc = ArcbEncoder::new();
    let data = enc.encode_block();
    assert!(data.is_empty());
    let res = decode_block(&data).unwrap();
    assert!(res.is_empty());
}

#[test]
fn single_digit_each() {
    for d in 0..=9 {
        assert_roundtrip(&[d]);
    }
}

#[test]
fn short_sequence() {
    assert_roundtrip(&[8, 3, 9, 1, 0, 2, 7, 4, 6, 5]);
}

#[test]
fn all_small_digits() {
    let digits: Vec<u8> = (0..100).map(|i| (i % 8) as u8).collect();
    assert_roundtrip(&digits);
}

#[test]
fn all_large_digits_eight() {
    let digits = vec![8u8; 200];
    assert_roundtrip(&digits);
}

#[test]
fn all_large_digits_nine() {
    let digits = vec![9u8; 200];
    assert_roundtrip(&digits);
}

#[test]
fn mixed_large_only() {
    let digits: Vec<u8> = (0..100).map(|i| if i % 2 == 0 { 8 } else { 9 }).collect();
    assert_roundtrip(&digits);
}

#[test]
fn alternating_small_large() {
    let mut digits = Vec::new();
    for i in 0..100 {
        digits.push(if i % 2 == 0 { 3 } else { 9 });
    }
    assert_roundtrip(&digits);
}

#[test]
fn random_small_blocks() {
    let mut rng = rand::thread_rng();
    for _ in 0..20 {
        let len = rng.gen_range(1..500);
        let digits: Vec<u8> = (0..len).map(|_| rng.gen_range(0..10)).collect();
        assert_roundtrip(&digits);
    }
}

#[test]
fn random_max_block() {
    let mut rng = rand::thread_rng();
    let digits: Vec<u8> = (0..65535).map(|_| rng.gen_range(0..10)).collect();
    assert_roundtrip(&digits);
}

#[test]
fn encoder_reuse_produces_independent_blocks() {
    let mut enc = ArcbEncoder::new();
    let block1 = vec![1, 2, 3, 8, 9];
    for &d in &block1 {
        enc.push_digit(d);
    }
    let compressed1 = enc.encode_block();

    let block2 = vec![0, 0, 0, 8, 8];
    for &d in &block2 {
        enc.push_digit(d);
    }
    let compressed2 = enc.encode_block();

    let dec1 = decode_block(&compressed1).unwrap();
    let dec2 = decode_block(&compressed2).unwrap();
    assert_eq!(dec1, block1);
    assert_eq!(dec2, block2);
}

#[test]
fn decode_truncated_header() {
    let too_short = [0u8; 5];
    let res = decode_block(&too_short);
    assert!(matches!(res, Err(ArcbError::TruncatedBlock)));
}

#[test]
fn decode_truncated_range() {
    use arcb::SuperblockHeader;
    use arcb::HEADER_SIZE;
    let mut buf = vec![0u8; HEADER_SIZE];
    let header = SuperblockHeader::new(0, 10, 8, 100, 50, 0);
    header.write(&mut buf[..HEADER_SIZE]);
    let res = decode_block(&buf);
    assert!(matches!(res, Err(ArcbError::TruncatedBlock)));
}

#[test]
fn decode_mask_large_mismatch() {
    use arcb::HEADER_SIZE;
    let mut enc = ArcbEncoder::new();
    for _ in 0..20 {
        enc.push_digit(8);
    }
    let mut data = enc.encode_block();
    for i in HEADER_SIZE..std::cmp::min(data.len(), HEADER_SIZE + 5) {
        data[i] ^= 0xFF;
    }
    let res = decode_block(&data);
    assert!(res.is_err(), "corrupted data should produce an error");
}

#[test]
fn compression_better_than_4bits_per_digit() {
    let mut rng = rand::thread_rng();
    let digits: Vec<u8> = (0..10000).map(|_| rng.gen_range(0..10)).collect();
    let mut enc = ArcbEncoder::new();
    for &d in &digits {
        enc.push_digit(d);
    }
    let compressed = enc.encode_block();
    let naive_bits = digits.len() * 4;
    let compressed_bits = compressed.len() * 8;
    assert!(
        compressed_bits < naive_bits,
        "compressed {} bits >= naive {} bits",
        compressed_bits,
        naive_bits
    );
}

#[test]
fn all_large_still_under_4bits_per_digit() {
    for &digit in &[8u8, 9u8] {
        let digits = vec![digit; 5000];
        let mut enc = ArcbEncoder::new();
        for &d in &digits {
            enc.push_digit(d);
        }
        let compressed = enc.encode_block();
        let compressed_bits = compressed.len() * 8;
        let naive_bits = digits.len() * 4;
        assert!(
            compressed_bits < naive_bits,
            "all-{}: {} bits >= {} bits",
            digit,
            compressed_bits,
            naive_bits
        );
    }
}
