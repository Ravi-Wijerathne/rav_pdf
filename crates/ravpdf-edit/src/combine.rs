//! Merge, Split, and Combine operations conforming to ISO 32000-1.
//!
//! Traceability: FR-07.1 - FR-07.7.

use crate::pages::extract_pages;
use ravpdf_core::document::PdfDocument;
use ravpdf_core::error::{PdfError, Result};

/// Merges multiple PDF documents into a single document preserving page order.
pub fn merge_documents(docs: &[PdfDocument]) -> Result<PdfDocument> {
    if docs.is_empty() {
        return Err(PdfError::InvalidStructure("No documents provided to merge".into()));
    }

    let mut merged = PdfDocument::new();

    for doc in docs {
        let count = doc.page_count();
        let indices: Vec<usize> = (0..count).collect();
        for idx in indices {
            let page = doc
                .get_page(idx)
                .ok_or_else(|| PdfError::InvalidStructure(format!("Page {} missing", idx)))?;

            let content = doc.get_page_content(idx).unwrap_or_default();
            let box_w = (page.media_box[2] - page.media_box[0]).abs();
            let box_h = (page.media_box[3] - page.media_box[1]).abs();

            merged.add_page_with_content(box_w, box_h, content);
        }
    }

    Ok(merged)
}

/// Splits a document into multiple documents based on slice ranges (start_idx, end_idx inclusive).
pub fn split_document(
    doc: &PdfDocument,
    ranges: &[(usize, usize)],
) -> Result<Vec<PdfDocument>> {
    let mut results = Vec::new();
    let total_pages = doc.page_count();

    for &(start, end) in ranges {
        if start >= total_pages || end >= total_pages || start > end {
            return Err(PdfError::InvalidStructure(format!(
                "Invalid split range: {}-{} for doc with {} pages",
                start, end, total_pages
            )));
        }

        let page_indices: Vec<usize> = (start..=end).collect();
        let split_doc = extract_pages(doc, &page_indices)?;
        results.push(split_doc);
    }

    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_merge_and_split() {
        let mut doc1 = PdfDocument::new();
        doc1.add_blank_page(612.0, 792.0);
        doc1.add_blank_page(612.0, 792.0);

        let mut doc2 = PdfDocument::new();
        doc2.add_blank_page(595.0, 842.0);

        let merged = merge_documents(&[doc1, doc2]).expect("merge docs");
        assert_eq!(merged.page_count(), 3);

        let splits = split_document(&merged, &[(0, 1), (2, 2)]).expect("split docs");
        assert_eq!(splits.len(), 2);
        assert_eq!(splits[0].page_count(), 2);
        assert_eq!(splits[1].page_count(), 1);
    }
}
