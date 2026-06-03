//! # ARCB: Adaptive Range Coding for Base-10 Digits
//!
//! Lossless compression library specialised for decimal digit strings (0-9),
//! achieving the theoretical entropy limit of ~3.322 bits per digit even on
//! uniformly random data.
//!
//! ## Quick Example
//!
//! ```rust
//! use arcb::ArcbEncoder;
//!
//! let mut encoder = ArcbEncoder::new();
//! for d in [8, 3, 9, 1, 0, 2, 7, 4, 6, 5] {
//!     encoder.push_digit(d);
//! }
//! let compressed = encoder.encode_block();
//!
//! let decompressed = arcb::decode_block(&compressed).unwrap();
//! assert_eq!(&decompressed, &[8, 3, 9, 1, 0, 2, 7, 4, 6, 5]);
//! ```

mod adaptive_model;
// mod bitstream; // removed: dead code, unused
mod decoder;
mod encoder;
mod error;
pub mod options;
mod superblock;

pub use decoder::decode_block;
pub use encoder::ArcbEncoder;
pub use error::ArcbError;
pub use options::CompressOptions;
pub use superblock::{
    SuperblockHeader, DEFAULT_SUPERBLOCK_SIZE, HEADER_SIZE, FLAG_COMPRESS_SMALL,
    FLAG_HAS_CHECKSUM,
};

/// Compute CRC-32 (IEEE 802.3) over a byte slice.
pub fn crc32(data: &[u8]) -> u32 {
    crc32_update(0, data)
}

/// Encode a single superblock into file format (magic + version + optional CRC + block).
pub fn encode_block_to_file_format(block: &[u8], with_checksum: bool) -> Vec<u8> {
    if with_checksum {
        let crc = crc32(block);
        let mut out = Vec::with_capacity(FILE_PREFIX_SIZE + 4 + block.len());
        out.extend_from_slice(&MAGIC);
        out.push(VERSION);
        out.extend_from_slice(&crc.to_be_bytes());
        out.extend_from_slice(block);
        out
    } else {
        let mut out = Vec::with_capacity(FILE_PREFIX_SIZE + block.len());
        out.extend_from_slice(&MAGIC);
        out.push(VERSION);
        out.extend_from_slice(block);
        out
    }
}

/// File-format magic bytes.
pub const MAGIC: [u8; 4] = *b"ARCB";

/// Current file-format version.
pub const VERSION: u8 = 1;

/// File-format prefix size: magic(4) + version(1) = 5 bytes.
pub const FILE_PREFIX_SIZE: usize = 5;

/// CRC-32 polynomial (IEEE 802.3, reflected).
const CRC32_POLY: u32 = 0xEDB8_8320;

fn crc32_init_table() -> [u32; 256] {
    let mut table = [0u32; 256];
    for i in 0..256 {
        let mut crc = i as u32;
        for _ in 0..8 {
            if crc & 1 != 0 {
                crc = (crc >> 1) ^ CRC32_POLY;
            } else {
                crc >>= 1;
            }
        }
        table[i as usize] = crc;
    }
    table
}

fn crc32_update(crc: u32, data: &[u8]) -> u32 {
    #[inline]
    fn table() -> &'static [u32; 256] {
        use std::sync::OnceLock;
        static TABLE: OnceLock<[u32; 256]> = OnceLock::new();
        TABLE.get_or_init(crc32_init_table)
    }

    let t = table();
    let mut crc = !crc;
    for &byte in data {
        let idx = ((crc ^ byte as u32) & 0xFF) as usize;
        crc = t[idx] ^ (crc >> 8);
    }
    !crc
}

/// Encode a decimal digit string to ARCB binary format with CRC-32 checksum.
///
/// File layout: magic(4) + version(1) + crc32(4) + block_data(N)
/// The CRC-32 covers everything after the CRC field (i.e. block_data).
pub fn encode_to_binary_with_checksum(input: &str) -> Result<Vec<u8>, ArcbError> {
    encode_to_binary_inner(input, true)
}

/// Encode a decimal digit string to ARCB binary format (no checksum).
pub fn encode_to_binary(input: &str) -> Result<Vec<u8>, ArcbError> {
    encode_to_binary_inner(input, false)
}

fn encode_to_binary_inner(input: &str, with_checksum: bool) -> Result<Vec<u8>, ArcbError> {
    if !input.chars().all(|c| c.is_ascii_digit()) {
        return Err(ArcbError::InvalidInput);
    }
    let crc_size = if with_checksum { 4 } else { 0 };
    if input.is_empty() {
        let mut out = Vec::with_capacity(FILE_PREFIX_SIZE + crc_size);
        out.extend_from_slice(&MAGIC);
        out.push(VERSION);
        if with_checksum {
            let crc = crc32_update(0, &[]);
            out.extend_from_slice(&crc.to_be_bytes());
        }
        return Ok(out);
    }
    let digits: Vec<u8> = input
        .chars()
        .map(|c| c.to_digit(10).unwrap() as u8)
        .collect();
    let mut encoder = ArcbEncoder::new();
    for &d in &digits {
        encoder.push_digit(d);
    }
    let mut compressed = encoder.encode_block();

    if with_checksum {
        let crc = crc32_update(0, &compressed);
        let mut out = Vec::with_capacity(FILE_PREFIX_SIZE + crc_size + compressed.len());
        out.extend_from_slice(&MAGIC);
        out.push(VERSION);
        out.extend_from_slice(&crc.to_be_bytes());
        out.append(&mut compressed);
        Ok(out)
    } else {
        let mut out = Vec::with_capacity(FILE_PREFIX_SIZE + compressed.len());
        out.extend_from_slice(&MAGIC);
        out.push(VERSION);
        out.append(&mut compressed);
        Ok(out)
    }
}

/// Decode ARCB binary format (magic + version prefix) to a digit string.
///
/// Auto-detects CRC presence. When the data after the 5-byte prefix is
/// long enough, attempts CRC validation; otherwise decodes as legacy format.
pub fn decode_from_binary(data: &[u8]) -> Result<String, ArcbError> {
    decode_from_binary_inner(data, true)
}

fn decode_from_binary_inner(data: &[u8], _validate_checksum: bool) -> Result<String, ArcbError> {
    if data.len() < FILE_PREFIX_SIZE {
        return Err(ArcbError::TruncatedBlock);
    }
    if &data[0..4] != &MAGIC {
        return Err(ArcbError::InvalidMagic);
    }
    let _version = data[4];

    let rest = &data[FILE_PREFIX_SIZE..];
    if rest.is_empty() {
        return Ok(String::new());
    }

    // Try legacy (no CRC) first
    match try_decode_legacy(rest) {
        Ok(s) => Ok(s),
        Err(_) if rest.len() > 4 => {
            // Try with CRC
            let expected_crc = u32::from_be_bytes([rest[0], rest[1], rest[2], rest[3]]);
            let block = &rest[4..];
            let actual_crc = crc32_update(0, block);
            if actual_crc == expected_crc {
                let digits = decode_block(block)?;
                return Ok(digits.iter().map(|d| (b'0' + d) as char).collect());
            }
            Err(ArcbError::ChecksumMismatch)
        }
        Err(e) => Err(e),
    }
}

fn try_decode_legacy(rest: &[u8]) -> Result<String, ArcbError> {
    let digits = decode_block(rest)?;
    Ok(digits.iter().map(|d| (b'0' + d) as char).collect())
}

/// Decode ARCB binary format with explicit CRC-32 validation.
pub fn decode_from_binary_checked(data: &[u8]) -> Result<String, ArcbError> {
    if data.len() < FILE_PREFIX_SIZE + 4 {
        return Err(ArcbError::TruncatedBlock);
    }
    if &data[0..4] != &MAGIC {
        return Err(ArcbError::InvalidMagic);
    }
    let expected_crc = u32::from_be_bytes([
        data[FILE_PREFIX_SIZE],
        data[FILE_PREFIX_SIZE + 1],
        data[FILE_PREFIX_SIZE + 2],
        data[FILE_PREFIX_SIZE + 3],
    ]);
    let block = &data[FILE_PREFIX_SIZE + 4..];
    let actual_crc = crc32_update(0, block);
    if actual_crc != expected_crc {
        return Err(ArcbError::ChecksumMismatch);
    }
    let digits = decode_block(block)?;
    Ok(digits.iter().map(|d| (b'0' + d) as char).collect())
}

/// Encode a decimal digit string to a Base64 text string.
pub fn encode_to_base64(input: &str) -> Result<String, ArcbError> {
    let binary = encode_to_binary(input)?;
    Ok(encode_base64(&binary))
}

/// Decode a Base64 string back to a decimal digit string.
pub fn decode_from_base64(b64: &str) -> Result<String, ArcbError> {
    let binary = decode_base64(b64).map_err(|_| ArcbError::CorruptedData)?;
    decode_from_binary(&binary)
}

// --- Base64 helpers (no external crate needed) ---

const B64_TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn encode_base64(data: &[u8]) -> String {
    let mut result = String::with_capacity((data.len() + 2) / 3 * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0];
        let b1 = chunk.get(1).copied().unwrap_or(0);
        let b2 = chunk.get(2).copied().unwrap_or(0);

        result.push(B64_TABLE[(b0 >> 2) as usize] as char);
        result.push(B64_TABLE[(((b0 & 0x03) << 4) | (b1 >> 4)) as usize] as char);
        if chunk.len() > 1 {
            result.push(B64_TABLE[(((b1 & 0x0F) << 2) | (b2 >> 6)) as usize] as char);
        } else {
            result.push('=');
        }
        if chunk.len() > 2 {
            result.push(B64_TABLE[(b2 & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }
    }
    result
}

/// Encode multiple superblocks in parallel using rayon.
///
/// Splits `digits` into chunks of `block_size` (max 65535), encodes each
/// independently, and concatenates the results.
///
/// Requires the `parallel` feature to be enabled.
#[cfg(feature = "parallel")]
pub fn encode_parallel(
    digits: &[u8],
    block_size: usize,
    compress_small: bool,
) -> Result<Vec<u8>, ArcbError> {
    use rayon::prelude::*;

    assert!(
        block_size > 0 && block_size <= 65535,
        "block_size must be 1..=65535"
    );

    let opts = CompressOptions::new().with_compress_small(compress_small);

    let chunks: Vec<&[u8]> = digits.chunks(block_size).collect();
    let encoded_blocks: Vec<Vec<u8>> = chunks
        .par_iter()
        .map(|chunk| {
            let mut enc = ArcbEncoder::with_options(opts.clone());
            for &d in chunk.iter() {
                enc.push_digit(d);
            }
            enc.encode_block()
        })
        .collect();

    let total_len: usize = encoded_blocks.iter().map(|b| b.len()).sum();
    let mut result = Vec::with_capacity(total_len);
    for block in encoded_blocks {
        result.extend_from_slice(&block);
    }
    Ok(result)
}

/// Decode multiple superblocks in parallel.
///
/// Expects a concatenation of superblocks (as produced by `encode_parallel`).
///
/// Requires the `parallel` feature to be enabled.
#[cfg(feature = "parallel")]
pub fn decode_parallel(data: &[u8]) -> Result<Vec<u8>, ArcbError> {
    use rayon::prelude::*;

    if data.is_empty() {
        return Ok(Vec::new());
    }

    // Parse individual superblocks
    let mut blocks: Vec<&[u8]> = Vec::new();
    let mut offset = 0;
    while offset < data.len() {
        if offset + HEADER_SIZE > data.len() {
            return Err(ArcbError::TruncatedBlock);
        }
        let header = SuperblockHeader::read(&data[offset..]).ok_or(ArcbError::TruncatedBlock)?;
        let total = header.total_block_size();
        if offset + total > data.len() {
            return Err(ArcbError::TruncatedBlock);
        }
        blocks.push(&data[offset..offset + total]);
        offset += total;
    }

    let decoded_chunks: Result<Vec<Vec<u8>>, ArcbError> = blocks
        .par_iter()
        .map(|block| decode_block(block))
        .collect();

    let decoded_chunks = decoded_chunks?;
    let total_len: usize = decoded_chunks.iter().map(|c| c.len()).sum();
    let mut result = Vec::with_capacity(total_len);
    for chunk in decoded_chunks {
        result.extend_from_slice(&chunk);
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc32_known_values() {
        assert_eq!(crc32(b"123456789"), 0xCBF43926);
        assert_eq!(crc32(b""), 0);
        assert_eq!(crc32(b"hello"), 0x3610A686);
    }

    #[test]
    fn file_format_roundtrip() {
        let input = "12345678901234567890";
        let binary = encode_to_binary(input).unwrap();
        let decoded = decode_from_binary(&binary).unwrap();
        assert_eq!(decoded, input);

        let binary_crc = encode_to_binary_with_checksum(input).unwrap();
        let decoded_crc = decode_from_binary_checked(&binary_crc).unwrap();
        assert_eq!(decoded_crc, input);
    }

    #[test]
    fn file_format_corrupted_crc() {
        let input = "1234567890";
        let mut binary = encode_to_binary_with_checksum(input).unwrap();
        if binary.len() > 10 {
            binary[10] ^= 0xFF;
        }
        let result = decode_from_binary_checked(&binary);
        assert!(matches!(result, Err(ArcbError::ChecksumMismatch)));
    }

    #[test]
    fn base64_roundtrip() {
        let input = "12345678901234567890";
        let b64 = encode_to_base64(input).unwrap();
        let decoded = decode_from_base64(&b64).unwrap();
        assert_eq!(decoded, input);
    }

    #[test]
    fn empty_input() {
        let binary = encode_to_binary("").unwrap();
        assert!(decode_from_binary(&binary).unwrap().is_empty());

        let binary_crc = encode_to_binary_with_checksum("").unwrap();
        assert!(decode_from_binary_checked(&binary_crc).unwrap().is_empty());
    }

    #[test]
    fn compress_options_builder() {
        let opts = CompressOptions::new()
            .with_compress_small(true)
            .with_parallel(false);
        assert!(opts.compress_small);
        assert!(!opts.parallel);
        assert_eq!(opts.flags(), FLAG_COMPRESS_SMALL);
    }

    #[cfg(feature = "parallel")]
    #[test]
    fn parallel_roundtrip() {
        let digits: Vec<u8> = (0..200_000).map(|i| (i % 10) as u8).collect();
        for block_size in [1000, 32767, 65535] {
            for compress_small in [false, true] {
                let compressed = encode_parallel(&digits, block_size, compress_small).unwrap();
                let decoded = decode_parallel(&compressed).unwrap();
                assert_eq!(decoded, digits);
            }
        }
    }
}

fn decode_base64(s: &str) -> Result<Vec<u8>, &'static str> {
    let s = s.trim();
    if s.is_empty() {
        return Ok(Vec::new());
    }
    let mut table = [0u8; 256];
    for (i, &c) in B64_TABLE.iter().enumerate() {
        table[c as usize] = i as u8;
    }
    let bytes = s.as_bytes();
    if bytes.len() % 4 != 0 {
        return Err("invalid base64 length");
    }
    let mut result = Vec::with_capacity(bytes.len() / 4 * 3);
    for chunk in bytes.chunks(4) {
        if chunk.len() < 4 {
            return Err("invalid base64 length");
        }
        let c0 = table[chunk[0] as usize];
        let c1 = table[chunk[1] as usize];
        let c2 = if chunk[2] == b'=' {
            0
        } else {
            table[chunk[2] as usize]
        };
        let c3 = if chunk[3] == b'=' {
            0
        } else {
            table[chunk[3] as usize]
        };
        result.push((c0 << 2) | (c1 >> 4));
        if chunk[2] != b'=' {
            result.push((c1 << 4) | (c2 >> 2));
        }
        if chunk[3] != b'=' {
            result.push((c2 << 6) | c3);
        }
    }
    Ok(result)
}
