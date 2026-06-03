// edge_cases.rs — Edge case tests for ARCB.

use arcb::{ArcbEncoder, ArcbError, decode_block};

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

// ---------------------------------------------------------------------------
// Minimum sizes
// ---------------------------------------------------------------------------

#[test]
fn block_single_digit_zero() {
    assert_roundtrip(&[0]);
}

#[test]
fn block_single_digit_nine() {
    assert_roundtrip(&[9]);
}

#[test]
fn block_two_digits_mixed() {
    assert_roundtrip(&[8, 7]);
}

#[test]
fn block_two_digits_both_large() {
    assert_roundtrip(&[8, 9]);
}

#[test]
fn block_two_digits_both_small() {
    assert_roundtrip(&[3, 6]);
}

// ---------------------------------------------------------------------------
// Special patterns
// ---------------------------------------------------------------------------

#[test]
fn all_zeros_long() {
    let digits = vec![0u8; 10_000];
    assert_roundtrip(&digits);
}

#[test]
fn all_eights_long() {
    let digits = vec![8u8; 10_000];
    assert_roundtrip(&digits);
}

#[test]
fn all_nines_long() {
    let digits = vec![9u8; 10_000];
    assert_roundtrip(&digits);
}

#[test]
fn alternating_8_9() {
    let digits: Vec<u8> = (0..5000).map(|i| if i % 2 == 0 { 8 } else { 9 }).collect();
    assert_roundtrip(&digits);
}

#[test]
fn alternating_small_large() {
    let digits: Vec<u8> = (0..5000).map(|i| if i % 2 == 0 { 0 } else { 8 }).collect();
    assert_roundtrip(&digits);
}

#[test]
fn only_one_large_at_end() {
    let mut digits = vec![7u8; 9999];
    digits.push(8);
    assert_roundtrip(&digits);
}

#[test]
fn only_one_large_at_start() {
    let mut digits = vec![9u8];
    digits.extend_from_slice(&vec![3u8; 9999]);
    assert_roundtrip(&digits);
}

#[test]
fn exact_max_block_size() {
    let digits = vec![5u8; 65535];
    assert_roundtrip(&digits);
}

// ---------------------------------------------------------------------------
// Corruption detection
// ---------------------------------------------------------------------------

#[test]
fn corrupted_range_causes_mask_mismatch() {
    use arcb::HEADER_SIZE;
    let mut enc = ArcbEncoder::new();
    for _ in 0..100 {
        enc.push_digit(8);
    }
    let mut data = enc.encode_block();
    for i in HEADER_SIZE..std::cmp::min(data.len(), HEADER_SIZE + 10) {
        data[i] ^= 0xFF;
    }
    let res = decode_block(&data);
    assert!(res.is_err(), "corrupted data should produce an error, got {res:?}");
}

#[test]
fn truncated_range_stream() {
    let mut enc = ArcbEncoder::new();
    for _ in 0..50 {
        enc.push_digit(4);
    }
    let mut data = enc.encode_block();
    let new_len = data.len() - 3;
    data.truncate(new_len);
    let res = decode_block(&data);
    assert!(res.is_err());
}

// ---------------------------------------------------------------------------
// Encoder reuse with extreme blocks
// ---------------------------------------------------------------------------

#[test]
fn reuse_encoder_after_all_small_then_all_large() {
    let mut enc = ArcbEncoder::new();
    for _ in 0..1000 {
        enc.push_digit(0);
    }
    let c1 = enc.encode_block();
    let d1 = decode_block(&c1).unwrap();
    assert_eq!(d1.len(), 1000);
    assert!(d1.iter().all(|&x| x == 0));

    for _ in 0..1000 {
        enc.push_digit(9);
    }
    let c2 = enc.encode_block();
    let d2 = decode_block(&c2).unwrap();
    assert_eq!(d2.len(), 1000);
    assert!(d2.iter().all(|&x| x == 9));
}

// ---------------------------------------------------------------------------
// Compression ratio vs theoretical entropy
// ---------------------------------------------------------------------------

#[test]
fn compression_ratio_uniform_random() {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let digits: Vec<u8> = (0..10000).map(|_| rng.gen_range(0..10)).collect();
    let mut enc = ArcbEncoder::new();
    for &d in &digits {
        enc.push_digit(d);
    }
    let compressed = enc.encode_block();
    let compressed_bits = compressed.len() * 8;
    let entropy_bits = (digits.len() as f64) * (10.0_f64.log2());
    assert!(
        (compressed_bits as f64) < entropy_bits * 1.15,
        "Compressed {} bits, entropy {:.1} bits, too large!",
        compressed_bits,
        entropy_bits
    );
}
