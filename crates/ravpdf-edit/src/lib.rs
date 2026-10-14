//! RavPDF Document Editing & Page Operations.
//!
//! Traceability:
//! - FR-04 (Text Editing)
//! - FR-05 (Image Editing)
//! - FR-06 (Organize Pages: insert, delete, reorder, rotate, crop, extract)
//! - FR-07 (Merge, Split, and Combine)

pub mod combine;
pub mod content;
pub mod pages;

pub use combine::{merge_documents, split_document};
pub use content::{append_text_block, insert_raw_image};
pub use pages::{
    crop_page, delete_page, extract_pages, insert_blank_page, reorder_pages, rotate_page,
};
