//! RavPDF Core Engine — Foundational ISO 32000-1 / ISO 32000-2 PDF Object Model, Parser, and Writer.
//!
//! Traceability:
//! - FR-01.1 (Document Rendering low-level data structures)
//! - FR-02.1 / FR-02.3 (PDF Creation from scratch)
//! - DR-01 (ISO 32000 internal structure conformance)
//! - DR-02 (Data storage, incremental save, object resolution)
//! - NFR-03.3 / NFR-03.4 (Graceful corrupted PDF handling, document integrity)
//! - NFR-04.2 / NFR-04.3 (Memory safety, parser input validation)
//! - NFR-07.1 (Workspace separation of core engine)

pub mod document;
pub mod error;
pub mod filter;
pub mod lexer;
pub mod object;
pub mod outline;
pub mod parser;
pub mod writer;

// Re-exports
pub use document::{paper_sizes, Page, PdfDocument};
pub use error::{PdfError, Result};
pub use object::{ObjectId, Object, PdfDictionary, PdfName, PdfStream, PdfString};
pub use outline::{add_bookmark, get_document_bookmarks, Bookmark};
pub use parser::Parser;
pub use writer::PdfWriter;
