//! RavPDF Document Tools, Watermarking, Bates Numbering, Metadata, Search, and Redaction.
//!
//! Traceability:
//! - FR-15 (Redaction)
//! - FR-17 (Document Comparison)
//! - FR-18 (Headers, Footers, Watermarks, Bates Numbering)
//! - FR-22 & FR-23 (Standards Conformance & Preflight)
//! - FR-25 (Optimization & Compression)
//! - FR-29 (File Attachments)
//! - FR-30 (Batch Processing & Action Wizard)
//! - FR-33 (Document Metadata)
//! - FR-34 (Search & Text Extraction)

pub mod attachments;
pub mod batch;
pub mod bates;
pub mod compare;
pub mod header_footer;
pub mod metadata;
pub mod optimize;
pub mod preflight;
pub mod redact;
pub mod search;
pub mod watermark;

pub use attachments::{add_attachment, get_attachments, remove_attachment, FileAttachment};
pub use batch::{execute_batch, BatchAction, BatchFileResult, BatchJob, BatchResult};
pub use bates::apply_bates_numbering;
pub use compare::{compare_documents, ComparisonReport, PageDiff};
pub use header_footer::{apply_headers_and_footers, HeaderFooterConfig};
pub use metadata::{get_document_info, set_document_info, DocumentInfo};
pub use optimize::{audit_space_usage, optimize_document, OptimizationStats, SpaceAudit};
pub use preflight::{run_preflight, PreflightCheck, PreflightProfile, PreflightReport, PreflightSeverity};
pub use redact::{redact_page_area, redact_pattern};
pub use search::{
    build_search_regex, extract_all_text, extract_page_text, find_and_replace,
    find_and_replace_page, search_document, SearchMatch,
};
pub use watermark::apply_text_watermark;

#[cfg(test)]
mod tests {
    use super::*;
    use ravpdf_core::document::PdfDocument;

    #[test]
    fn test_watermark_and_metadata() {
        let mut doc = PdfDocument::new();
        doc.add_blank_page(612.0, 792.0);

        let mut info = DocumentInfo::default();
        info.title = Some("Confidential Agreement".into());
        info.author = Some("Legal Counsel".into());
        set_document_info(&mut doc, &info).expect("set doc info");

        let read_info = get_document_info(&doc).expect("get doc info");
        assert_eq!(read_info.title.as_deref(), Some("Confidential Agreement"));

        apply_text_watermark(&mut doc, "CONFIDENTIAL", 0.3, 45.0, 60.0)
            .expect("apply watermark");
        apply_bates_numbering(&mut doc, "DOC", 1, 6, "").expect("apply bates");

        let serialized = doc.to_bytes().expect("serialize doc");
        let reloaded = PdfDocument::load_from_bytes(&serialized).expect("reload doc");
        assert_eq!(reloaded.page_count(), 1);
    }
}
