//! Error types for PDF parsing, reading, writing, and object manipulation.
//!
//! Traceability: NFR-03.3 (graceful error handling), NFR-03.4 (document integrity).

use thiserror::Error;

/// The primary error type for operations in `ravpdf-core`.
#[derive(Debug, Error)]
pub enum PdfError {
    #[error("Invalid PDF header: {0}")]
    InvalidHeader(String),

    #[error("Unexpected end of file while parsing")]
    UnexpectedEof,

    #[error("PDF parse error: {0}")]
    ParseError(String),

    #[error("Object not found: {0} {1} R")]
    ObjectNotFound(u32, u16),

    #[error("Invalid cross-reference table or stream: {0}")]
    InvalidXRef(String),

    #[error("Stream filter decompression failed: {0}")]
    FilterError(String),

    #[error("PDF decryption failed: {0}")]
    DecryptionError(String),

    #[error("Invalid document structure: {0}")]
    InvalidStructure(String),

    #[error("Invalid object type: expected {expected}, found {found}")]
    TypeMismatch {
        expected: &'static str,
        found: &'static str,
    },

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

/// Convenience result alias for `ravpdf-core`.
pub type Result<T> = std::result::Result<T, PdfError>;
