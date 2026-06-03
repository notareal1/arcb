//! Superblock structure and header for ARCB.
//!
//! Each superblock holds up to 65535 decimal digits, compressed independently.
//!
//! ## Header layout (11 bytes, big-endian)
//!
//! | Offset | Size | Field       | Description                                           |
//! |--------|------|-------------|-------------------------------------------------------|
//! | 0      | 1    | flags       | Bit flags (compress_small, checksum, ...)             |
//! | 1-2    | 2    | n           | Total number of digits (1..65535)                     |
//! | 3-4    | 2    | small_count | Number of Small digits (group 0-7)                    |
//! | 5-6    | 2    | mask_len    | Compressed mask size in bytes                         |
//! | 7-8    | 2    | large_len   | Compressed large-bits size in bytes                   |
//! | 9-10   | 2    | small_len   | Small data size in bytes (raw or range-compressed)    |

/// Default superblock capacity (max digits per block).
pub const DEFAULT_SUPERBLOCK_SIZE: u16 = 65535;

/// Fixed header size in bytes.
pub const HEADER_SIZE: usize = 11;

// Flag bits
pub const FLAG_COMPRESS_SMALL: u8 = 0x01;
pub const FLAG_HAS_CHECKSUM: u8 = 0x02;

/// Header of a compressed superblock.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SuperblockHeader {
    /// Bit flags (see FLAG_* constants).
    pub flags: u8,
    /// Total number of digits (N).
    pub n: u16,
    /// Number of Small digits (group 0-7).
    pub small_count: u16,
    /// Compressed mask length in bytes.
    pub mask_len: u16,
    /// Compressed large-bits length in bytes.
    pub large_len: u16,
    /// Small data length in bytes.
    pub small_len: u16,
}

impl SuperblockHeader {
    pub fn new(
        flags: u8,
        n: u16,
        small_count: u16,
        mask_len: u16,
        large_len: u16,
        small_len: u16,
    ) -> Self {
        Self {
            flags,
            n,
            small_count,
            mask_len,
            large_len,
            small_len,
        }
    }

    /// Write the header into `buf` (at least `HEADER_SIZE` bytes).
    pub fn write(&self, buf: &mut [u8]) {
        debug_assert!(buf.len() >= HEADER_SIZE);
        buf[0] = self.flags;
        buf[1..3].copy_from_slice(&self.n.to_be_bytes());
        buf[3..5].copy_from_slice(&self.small_count.to_be_bytes());
        buf[5..7].copy_from_slice(&self.mask_len.to_be_bytes());
        buf[7..9].copy_from_slice(&self.large_len.to_be_bytes());
        buf[9..11].copy_from_slice(&self.small_len.to_be_bytes());
    }

    /// Read a header from `block`. Returns `None` if there are not enough bytes.
    pub fn read(block: &[u8]) -> Option<Self> {
        if block.len() < HEADER_SIZE {
            return None;
        }
        let flags = block[0];
        let n = u16::from_be_bytes([block[1], block[2]]);
        let small_count = u16::from_be_bytes([block[3], block[4]]);
        let mask_len = u16::from_be_bytes([block[5], block[6]]);
        let large_len = u16::from_be_bytes([block[7], block[8]]);
        let small_len = u16::from_be_bytes([block[9], block[10]]);
        Some(Self {
            flags,
            n,
            small_count,
            mask_len,
            large_len,
            small_len,
        })
    }

    /// Number of Large digits (group 8-9).
    pub fn large_count(&self) -> u16 {
        self.n.saturating_sub(self.small_count)
    }

    /// Raw Small data size in bytes (when not range-compressed).
    pub fn small_raw_bytes(&self) -> usize {
        (self.small_count as usize * 3).div_ceil(8)
    }

    /// Whether Small values are range-coded.
    pub fn compress_small(&self) -> bool {
        self.flags & FLAG_COMPRESS_SMALL != 0
    }

    /// Whether the block has a trailing CRC-32 checksum.
    pub fn has_checksum(&self) -> bool {
        self.flags & FLAG_HAS_CHECKSUM != 0
    }

    /// Total block size in bytes (header + mask + large + small).
    pub fn total_block_size(&self) -> usize {
        HEADER_SIZE + self.mask_len as usize + self.large_len as usize + self.small_len as usize
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_write_and_read() {
        let header = SuperblockHeader::new(0x03, 65535, 52428, 40000, 30000, 1234);
        let mut buf = [0u8; HEADER_SIZE];
        header.write(&mut buf);
        let decoded = SuperblockHeader::read(&buf).unwrap();
        assert_eq!(decoded, header);
    }

    #[test]
    fn header_read_too_short() {
        assert!(SuperblockHeader::read(&[0u8; 10]).is_none());
    }

    #[test]
    fn header_flags() {
        let h = SuperblockHeader::new(FLAG_COMPRESS_SMALL, 100, 80, 50, 30, 20);
        assert!(h.compress_small());
        assert!(!h.has_checksum());

        let h2 = SuperblockHeader::new(FLAG_HAS_CHECKSUM, 100, 80, 50, 30, 20);
        assert!(!h2.compress_small());
        assert!(h2.has_checksum());
    }

    #[test]
    fn large_count() {
        let header = SuperblockHeader::new(0, 100, 80, 0, 0, 0);
        assert_eq!(header.large_count(), 20);
    }
}
