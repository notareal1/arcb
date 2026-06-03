//! Error types for the ARCB library.

use thiserror::Error;

/// Errors that can occur when decompressing or processing ARCB data.
#[derive(Debug, Error)]
pub enum ArcbError {
    /// Data block is too short to contain the expected header or body.
    #[error("Truncated block: not enough bytes")]
    TruncatedBlock,

    /// The number of 1-bits in the mask does not match the large-digit count
    /// declared in the header.
    #[error("Corrupted data: mask ones count does not match large count")]
    MaskLargeMismatch,

    /// Compressed data is corrupted (ANS/range coder failed to decode).
    #[error("Corrupted data: ANS decoding failed")]
    CorruptedData,

    /// CRC-32 checksum mismatch: file data is corrupted.
    #[error("CRC-32 checksum mismatch: data corrupted")]
    ChecksumMismatch,

    /// I/O error when reading/writing a bit stream.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Invalid input (contains non-digit characters).
    #[error("Invalid input: must contain only digits 0-9")]
    InvalidInput,

    /// Invalid magic bytes (not an ARCB file).
    #[error("Invalid magic bytes: not an ARCB file")]
    InvalidMagic,
}
