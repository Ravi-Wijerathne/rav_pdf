//! RavPDF Document Editing & Page Operations.
//!
//! Traceability:
//! - FR-02 (PDF Creation from Images and Plain Text)
//! - FR-04 (Text Editing)
//! - FR-05 (Image Editing)
//! - FR-06 (Organize Pages: insert, delete, reorder, rotate, crop, extract)
//! - FR-07 (Merge, Split, and Combine)

pub mod combine;
pub mod content;
pub mod create;
pub mod labels;
pub mod pages;

pub use combine::{merge_documents, split_document};
pub use content::{
    append_text_block, extract_page_text_spans, insert_formatted_text_block, insert_image_file,
    insert_raw_image, replace_text_span_in_page, suggest_font_substitute,
    FontSubstitute, FormattedTextBlockOptions, TextAlignment, TextSpan,
};
pub use create::{
    create_pdf_from_images, create_pdf_from_markdown, create_pdf_from_raw_images,
    create_pdf_from_text, ImagePdfOptions, MarkdownPdfOptions, TextPdfOptions,
};
pub use labels::{
    format_page_label, get_page_label, get_page_labels, set_page_labels, PageLabelRange,
    PageLabelStyle,
};
pub use pages::{
    crop_page, delete_page, duplicate_page, extract_pages, insert_blank_page, reorder_pages,
    rotate_page,
};

