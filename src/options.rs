//! Compression options for ARCB.

/// Configuration options for the ARCB encoder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompressOptions {
    /// Enable adaptive range coding for Small values (0-7).
    ///
    /// When `false` (default), Small values are stored raw at 3 bits/value.
    /// When `true`, Small values are compressed with an adaptive 8-symbol
    /// range coder (symbols 0-7, initial counts `[1; 8]`), which approaches
    /// the entropy limit for non-uniform distributions.
    pub compress_small: bool,

    /// Placeholder for future parallel block processing support.
    pub parallel: bool,
}

impl Default for CompressOptions {
    fn default() -> Self {
        Self {
            compress_small: false,
            parallel: false,
        }
    }
}

impl CompressOptions {
    /// Create options with all features at their defaults (no Small compression).
    pub fn new() -> Self {
        Self::default()
    }

    /// Enable adaptive Small value compression.
    pub fn with_compress_small(mut self, enable: bool) -> Self {
        self.compress_small = enable;
        self
    }

    /// Enable parallel block processing (reserved).
    pub fn with_parallel(mut self, enable: bool) -> Self {
        self.parallel = enable;
        self
    }

    /// Packed flags byte for the superblock header.
    ///
    /// Bit 0: compress_small
    /// Bit 1: has_checksum (reserved)
    /// Bits 2-7: reserved
    pub fn flags(&self) -> u8 {
        let mut f = 0u8;
        if self.compress_small {
            f |= 0x01;
        }
        f
    }
}
