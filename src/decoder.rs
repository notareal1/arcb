//! Decoder for ARCB.

use crate::adaptive_model::{AdaptiveBinaryModel, AdaptiveSymbolModel};
use crate::error::ArcbError;
use crate::superblock::{HEADER_SIZE, SuperblockHeader};
use constriction::stream::{Decode, queue::DefaultRangeDecoder};
use std::io::Cursor;
use bitstream_io::{BigEndian, BitRead, BitReader};

/// Decompress a single ARCB superblock into the original digit sequence.
///
/// Supports both range-coded Small (FLAG_COMPRESS_SMALL) and raw 3-bit modes.
pub fn decode_block(block: &[u8]) -> Result<Vec<u8>, ArcbError> {
    if block.is_empty() {
        return Ok(Vec::new());
    }

    if block.len() < HEADER_SIZE {
        return Err(ArcbError::TruncatedBlock);
    }

    let header = SuperblockHeader::read(block).ok_or(ArcbError::TruncatedBlock)?;
    let n = header.n as usize;
    let small_count = header.small_count as usize;
    let large_count = header.large_count() as usize;

    let mask_len = header.mask_len as usize;
    let large_len = header.large_len as usize;

    let mask_start = HEADER_SIZE;
    let large_start = mask_start + mask_len;
    let small_start = large_start + large_len;

    if block.len() < small_start {
        return Err(ArcbError::TruncatedBlock);
    }

    let mask_bytes = &block[mask_start..large_start];
    let large_bytes = &block[large_start..small_start];
    let small_raw_buf = &block[small_start..];

    // Convert Vec<u8> to Vec<u32> (big-endian, 4 bytes per word)
    let mask_words: Vec<u32> = mask_bytes
        .chunks_exact(4)
        .map(|chunk| u32::from_be_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect();
    let large_words: Vec<u32> = large_bytes
        .chunks_exact(4)
        .map(|chunk| u32::from_be_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect();

    // --- Decompress mask (range coding: decode forward) ---
    let mut mask_decoder =
        DefaultRangeDecoder::from_compressed(&mask_words).map_err(|_| ArcbError::CorruptedData)?;
    let mut mask_model = AdaptiveBinaryModel::new();
    let mut mask = vec![0u32; n];
    for m in &mut mask {
        *m = mask_decoder
            .decode_symbol(&mask_model)
            .map_err(|_| ArcbError::CorruptedData)? as u32;
        mask_model.update(*m as usize);
    }

    // --- Decompress large_bits (range coding: decode forward) ---
    let mut large_decoder =
        DefaultRangeDecoder::from_compressed(&large_words).map_err(|_| ArcbError::CorruptedData)?;
    let mut large_model = AdaptiveBinaryModel::new();
    let mut large_bits = vec![0u32; large_count];
    for lb in &mut large_bits {
        *lb = large_decoder
            .decode_symbol(&large_model)
            .map_err(|_| ArcbError::CorruptedData)? as u32;
        large_model.update(*lb as usize);
    }

    // Verify mask: number of 1-bits must equal large_count
    let ones = mask.iter().filter(|&&b| b == 1).count();
    if ones != large_count {
        return Err(ArcbError::MaskLargeMismatch);
    }

    // --- Read small_vals ---
    let mut small_vals = Vec::with_capacity(small_count);

    if header.compress_small() && small_count > 0 {
        // Range decode
        let small_len = header.small_len as usize;
        if small_raw_buf.len() < small_len {
            return Err(ArcbError::TruncatedBlock);
        }
        let small_bytes = &small_raw_buf[..small_len];
        let small_words: Vec<u32> = small_bytes
            .chunks_exact(4)
            .map(|chunk| u32::from_be_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
            .collect();
        let mut small_decoder = DefaultRangeDecoder::from_compressed(&small_words)
            .map_err(|_| ArcbError::CorruptedData)?;
        let mut small_model = AdaptiveSymbolModel::<8>::new();
        for _ in 0..small_count {
            let sym = small_decoder
                .decode_symbol(&small_model)
                .map_err(|_| ArcbError::CorruptedData)?;
            small_model.update(sym);
            small_vals.push(sym as u8);
        }
    } else {
        // Raw 3 bits per value
        let cursor = Cursor::new(small_raw_buf);
        let mut br = BitReader::endian(cursor, BigEndian);
        for _ in 0..small_count {
            let val = br.read::<u8>(3).map_err(ArcbError::Io)?;
            small_vals.push(val);
        }
    }

    // Reconstruct original digit sequence
    let mut digits = Vec::with_capacity(n);
    let mut ptr_small = 0;
    let mut ptr_large = 0;

    for &is_large in &mask {
        if is_large == 1 {
            let digit = if large_bits[ptr_large] == 1 { 9 } else { 8 };
            digits.push(digit);
            ptr_large += 1;
        } else {
            digits.push(small_vals[ptr_small]);
            ptr_small += 1;
        }
    }

    Ok(digits)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ArcbEncoder;

    #[test]
    fn decode_empty_block() {
        let mut enc = ArcbEncoder::new();
        let data = enc.encode_block();
        assert!(data.is_empty());
        let res = decode_block(&data).unwrap();
        assert_eq!(res, Vec::<u8>::new());
    }

    #[test]
    fn decode_single_large() {
        let mut enc = ArcbEncoder::new();
        enc.push_digit(8);
        let data = enc.encode_block();
        let res = decode_block(&data).unwrap();
        assert_eq!(res, vec![8]);
    }

    #[test]
    fn decode_mixed_sequence() {
        let digits = vec![0, 9, 7, 8, 5, 6, 8, 9, 1, 0];
        let mut enc = ArcbEncoder::new();
        for &d in &digits {
            enc.push_digit(d);
        }
        let data = enc.encode_block();
        let res = decode_block(&data).unwrap();
        assert_eq!(res, digits);
    }

    #[test]
    fn decode_with_small_compression() {
        let digits = vec![0, 9, 7, 8, 5, 6, 8, 9, 1, 0];
        let mut enc = ArcbEncoder::with_options(
            crate::CompressOptions::new().with_compress_small(true),
        );
        for &d in &digits {
            enc.push_digit(d);
        }
        let data = enc.encode_block();
        let res = decode_block(&data).unwrap();
        assert_eq!(res, digits);
    }

    #[test]
    fn error_truncated_header() {
        let res = decode_block(&[0u8; 5]);
        assert!(matches!(res, Err(ArcbError::TruncatedBlock)));
    }

    #[test]
    fn error_truncated_range() {
        let mut buf = vec![0u8; HEADER_SIZE];
        let header = SuperblockHeader::new(0, 10, 8, 100, 50, 0);
        header.write(&mut buf[..HEADER_SIZE]);
        let res = decode_block(&buf);
        assert!(matches!(res, Err(ArcbError::TruncatedBlock)));
    }

    #[test]
    fn error_mask_large_mismatch() {
        let mut enc = ArcbEncoder::new();
        for _ in 0..20 {
            enc.push_digit(8);
        }
        let mut data = enc.encode_block();
        if data.len() > 12 {
            data[12] ^= 0xFF;
        }
        let res = decode_block(&data);
        assert!(res.is_err());
    }
}
