//! Encoder for ARCB.

use crate::adaptive_model::{AdaptiveBinaryModel, AdaptiveSymbolModel};
use crate::options::CompressOptions;
use crate::superblock::{
    HEADER_SIZE, SuperblockHeader, DEFAULT_SUPERBLOCK_SIZE,
};
use constriction::stream::{Encode, queue::DefaultRangeEncoder};

/// ARCB decimal digit encoder.
///
/// ## Example
///
/// ```rust
/// use arcb::ArcbEncoder;
/// let mut enc = ArcbEncoder::new();
/// for d in [8, 3, 9, 1, 0, 2, 7, 4, 6, 5] {
///     enc.push_digit(d);
/// }
/// let compressed = enc.encode_block();
/// ```
pub struct ArcbEncoder {
    n: usize,
    small_count: usize,
    mask: Vec<u32>,
    large_bits: Vec<u32>,
    small_vals: Vec<u8>,
    opts: CompressOptions,
}

impl Default for ArcbEncoder {
    fn default() -> Self {
        Self::new()
    }
}

impl ArcbEncoder {
    /// Create a new encoder with default options (no Small compression).
    pub fn new() -> Self {
        Self::with_options(CompressOptions::default())
    }

    /// Create a new encoder with specific options.
    pub fn with_options(opts: CompressOptions) -> Self {
        Self {
            n: 0,
            small_count: 0,
            mask: Vec::with_capacity(DEFAULT_SUPERBLOCK_SIZE as usize),
            large_bits: Vec::with_capacity(DEFAULT_SUPERBLOCK_SIZE as usize / 5),
            small_vals: Vec::with_capacity(DEFAULT_SUPERBLOCK_SIZE as usize * 4 / 5),
            opts,
        }
    }

    /// Push a single digit (0-9) into the current block.
    pub fn push_digit(&mut self, d: u8) {
        assert!(d <= 9, "digit out of range 0..9");
        self.n += 1;
        if d < 8 {
            self.mask.push(0);
            self.small_vals.push(d);
            self.small_count += 1;
        } else {
            self.mask.push(1);
            self.large_bits.push(if d == 9 { 1 } else { 0 });
        }
    }

    /// Encode the current block into a compressed `Vec<u8>`.
    ///
    /// Clears internal state so the encoder can be reused.
    pub fn encode_block(&mut self) -> Vec<u8> {
        if self.n == 0 {
            return Vec::new();
        }
        assert!(
            self.n <= DEFAULT_SUPERBLOCK_SIZE as usize,
            "block size exceeds MAX (65535)"
        );

        let n = self.n as u16;
        let small_count = self.small_count as u16;
        let flags = self.opts.flags();

        // --- Compress mask via Range Coding with adaptive binary model ---
        // Range coding is queue-based (FIFO): encode forward, decode forward
        // => model updates stay in sync between encoder and decoder
        let mut mask_encoder = DefaultRangeEncoder::new();
        let mut mask_model = AdaptiveBinaryModel::new();
        for &bit in &self.mask {
            mask_encoder
                .encode_symbol(bit as usize, &mask_model)
                .unwrap();
            mask_model.update(bit as usize);
        }
        let mask_compressed: Vec<u32> = mask_encoder.into_compressed().unwrap();

        // --- Compress large_bits via Range Coding with adaptive binary model ---
        let mut large_encoder = DefaultRangeEncoder::new();
        let mut large_model = AdaptiveBinaryModel::new();
        for &bit in &self.large_bits {
            large_encoder
                .encode_symbol(bit as usize, &large_model)
                .unwrap();
            large_model.update(bit as usize);
        }
        let large_compressed: Vec<u32> = large_encoder.into_compressed().unwrap();

        // Convert Vec<u32> to Vec<u8> (big-endian)
        let mask_bytes: Vec<u8> = mask_compressed
            .iter()
            .flat_map(|w| w.to_be_bytes())
            .collect();
        let large_bytes: Vec<u8> = large_compressed
            .iter()
            .flat_map(|w| w.to_be_bytes())
            .collect();

        let mask_len = mask_bytes.len() as u16;
        let large_len = large_bytes.len() as u16;

        // --- Compress small_vals ---
        let (small_bytes, small_len) = if self.opts.compress_small && self.small_count > 0 {
            // Range-code each small value (0-7) as a symbol
            let mut small_encoder = DefaultRangeEncoder::new();
            let mut small_model = AdaptiveSymbolModel::<8>::new();
            for &val in &self.small_vals {
                small_encoder.encode_symbol(val as usize, &small_model).unwrap();
                small_model.update(val as usize);
            }
            let small_compressed: Vec<u32> = small_encoder.into_compressed().unwrap();
            let bytes: Vec<u8> = small_compressed
                .iter()
                .flat_map(|w| w.to_be_bytes())
                .collect();
            let len = bytes.len() as u16;
            (bytes, len)
        } else {
            // Raw 3 bits per value
            let raw_bytes = self.small_count as usize * 3;
            let byte_len = raw_bytes.div_ceil(8);
            let mut buf = vec![0u8; byte_len];
            let mut bit_pos = 0usize;
            for &val in &self.small_vals {
                for b in (0..3).rev() {
                    let byte_idx = bit_pos / 8;
                    let bit_idx = 7 - (bit_pos % 8);
                    buf[byte_idx] |= ((val >> b) & 1) << bit_idx;
                    bit_pos += 1;
                }
            }
            let len = buf.len() as u16;
            (buf, len)
        };

        let mut output = Vec::with_capacity(
            HEADER_SIZE + mask_bytes.len() + large_bytes.len() + small_bytes.len() + 16,
        );
        output.resize(HEADER_SIZE, 0);
        output.extend_from_slice(&mask_bytes);
        output.extend_from_slice(&large_bytes);
        output.extend_from_slice(&small_bytes);

        // Write header (11 bytes)
        SuperblockHeader::new(flags, n, small_count, mask_len, large_len, small_len)
            .write(&mut output[0..HEADER_SIZE]);

        self.clear();
        output
    }

    fn clear(&mut self) {
        self.n = 0;
        self.small_count = 0;
        self.mask.clear();
        self.large_bits.clear();
        self.small_vals.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decode_block;

    #[test]
    fn roundtrip_10_digits() {
        let digits = [8, 3, 9, 1, 0, 2, 7, 4, 6, 5];
        let mut enc = ArcbEncoder::new();
        for &d in &digits {
            enc.push_digit(d);
        }
        let compressed = enc.encode_block();
        let decoded = decode_block(&compressed).unwrap();
        assert_eq!(&decoded, &digits);
    }

    #[test]
    fn roundtrip_with_small_compression() {
        let digits = [8, 3, 9, 1, 0, 2, 7, 4, 6, 5];
        let mut enc = ArcbEncoder::with_options(CompressOptions::new().with_compress_small(true));
        for &d in &digits {
            enc.push_digit(d);
        }
        let compressed = enc.encode_block();
        let decoded = decode_block(&compressed).unwrap();
        assert_eq!(&decoded, &digits);
    }

    #[test]
    fn empty_block() {
        let mut enc = ArcbEncoder::new();
        let compressed = enc.encode_block();
        assert!(compressed.is_empty());
    }

    #[test]
    fn reuse_encoder() {
        let mut enc = ArcbEncoder::new();
        for &d in &[9, 9, 9] {
            enc.push_digit(d);
        }
        let c1 = enc.encode_block();
        let d1 = decode_block(&c1).unwrap();
        assert_eq!(d1, vec![9, 9, 9]);

        for &d in &[0, 0, 8, 7] {
            enc.push_digit(d);
        }
        let c2 = enc.encode_block();
        let d2 = decode_block(&c2).unwrap();
        assert_eq!(d2, vec![0, 0, 8, 7]);
    }

    #[test]
    fn small_compression_all_zeros() {
        let digits = vec![0u8; 1000];
        let mut enc = ArcbEncoder::with_options(CompressOptions::new().with_compress_small(true));
        for &d in &digits {
            enc.push_digit(d);
        }
        let compressed = enc.encode_block();
        let decoded = decode_block(&compressed).unwrap();
        assert_eq!(decoded, digits);
    }

    #[test]
    fn small_compression_all_large() {
        let digits = vec![8u8; 500];
        let mut enc = ArcbEncoder::with_options(CompressOptions::new().with_compress_small(true));
        for &d in &digits {
            enc.push_digit(d);
        }
        let compressed = enc.encode_block();
        let decoded = decode_block(&compressed).unwrap();
        assert_eq!(decoded, digits);
    }

    #[test]
    fn small_compression_mixed() {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let digits: Vec<u8> = (0..10000).map(|_| rng.gen_range(0..10)).collect();
        let mut enc = ArcbEncoder::with_options(CompressOptions::new().with_compress_small(true));
        for &d in &digits {
            enc.push_digit(d);
        }
        let compressed = enc.encode_block();
        let decoded = decode_block(&compressed).unwrap();
        assert_eq!(decoded, digits);
    }
}
