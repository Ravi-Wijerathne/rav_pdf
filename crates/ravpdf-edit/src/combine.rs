//! Merge, Split, and Combine operations conforming to ISO 32000-1.
//!
//! Traceability: FR-07.1 - FR-07.7.

use crate::pages::extract_pages;
use ravpdf_core::document::PdfDocument;
use ravpdf_core::error::{PdfError, Result};
use ravpdf_core::outline::{add_bookmark, get_document_bookmarks, Bookmark};

/// Merges multiple PDF documents into a single document preserving page order.
pub fn merge_documents(docs: &[PdfDocument]) -> Result<PdfDocument> {
    merge_documents_with_options(docs, false)
}

/// Merges multiple PDF documents with optional preservation of bookmarks and structure (FR-07.7).
pub fn merge_documents_with_options(
    docs: &[PdfDocument],
    preserve_bookmarks: bool,
) -> Result<PdfDocument> {
    if docs.is_empty() {
        return Err(PdfError::InvalidStructure("No documents provided to merge".into()));
    }

    let mut merged = PdfDocument::new();
    let mut page_offset = 0;
    let mut accumulated_bookmarks: Vec<Bookmark> = Vec::new();

    for doc in docs {
        let count = doc.page_count();
        if preserve_bookmarks {
            if let Ok(bms) = get_document_bookmarks(doc) {
                for mut bm in bms {
                    offset_bookmark_pages(&mut bm, page_offset);
                    accumulated_bookmarks.push(bm);
                }
            }
        }

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

        page_offset += count;
    }

    if preserve_bookmarks {
        for bm in accumulated_bookmarks {
            let page_idx = bm.page_index.unwrap_or(0);
            let _ = add_bookmark(&mut merged, &bm.title, page_idx);
        }
    }

    Ok(merged)
}

fn offset_bookmark_pages(bm: &mut Bookmark, offset: usize) {
    if let Some(ref mut p) = bm.page_index {
        *p += offset;
    }
    for child in &mut bm.children {
        offset_bookmark_pages(child, offset);
    }
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

/// Splits a document into individual single-page documents (FR-07.5).
pub fn split_document_into_single_pages(doc: &PdfDocument) -> Result<Vec<PdfDocument>> {
    let total_pages = doc.page_count();
    let mut results = Vec::with_capacity(total_pages);
    for idx in 0..total_pages {
        let single_doc = extract_pages(doc, &[idx])?;
        results.push(single_doc);
    }
    Ok(results)
}

/// Splits a document by its top-level bookmarks / chapters (FR-07.4).
pub fn split_document_by_bookmarks(
    doc: &PdfDocument,
) -> Result<Vec<(String, PdfDocument)>> {
    let bookmarks = get_document_bookmarks(doc)?;
    let total_pages = doc.page_count();
    if bookmarks.is_empty() || total_pages == 0 {
        return Ok(Vec::new());
    }

    let mut valid_marks: Vec<(String, usize)> = bookmarks
        .into_iter()
        .filter_map(|b| b.page_index.map(|p| (b.title, p.min(total_pages - 1))))
        .collect();

    valid_marks.sort_by_key(|(_, p)| *p);
    valid_marks.dedup_by_key(|(_, p)| *p);

    let mut results = Vec::new();
    for i in 0..valid_marks.len() {
        let (title, start_page) = &valid_marks[i];
        let end_page = if i + 1 < valid_marks.len() {
            valid_marks[i + 1].1.saturating_sub(1)
        } else {
            total_pages - 1
        };

        if start_page <= &end_page {
            let indices: Vec<usize> = (*start_page..=end_page).collect();
            let split_doc = extract_pages(doc, &indices)?;
            results.push((title.clone(), split_doc));
        }
    }

    Ok(results)
}

/// Splits a document into chunks constrained by maximum serialized file size (FR-07.4).
pub fn split_document_by_size(
    doc: &PdfDocument,
    max_bytes: usize,
) -> Result<Vec<PdfDocument>> {
    let total_pages = doc.page_count();
    if total_pages == 0 {
        return Ok(Vec::new());
    }

    let mut results = Vec::new();
    let mut current_indices = Vec::new();

    for page_idx in 0..total_pages {
        current_indices.push(page_idx);
        let test_doc = extract_pages(doc, &current_indices)?;
        let size = test_doc.to_bytes().map(|b| b.len()).unwrap_or(0);

        if size > max_bytes && current_indices.len() > 1 {
            // Pop the last page and finalize previous chunk
            current_indices.pop();
            let chunk_doc = extract_pages(doc, &current_indices)?;
            results.push(chunk_doc);
            current_indices = vec![page_idx];
        }
    }

    if !current_indices.is_empty() {
        let chunk_doc = extract_pages(doc, &current_indices)?;
        results.push(chunk_doc);
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

        // Test split into single pages (FR-07.5)
        let single_pages = split_document_into_single_pages(&merged).expect("single pages");
        assert_eq!(single_pages.len(), 3);
        for sp in &single_pages {
            assert_eq!(sp.page_count(), 1);
        }

        // Test merge with bookmarks (FR-07.7)
        let mut b_doc1 = PdfDocument::new();
        b_doc1.add_blank_page(612.0, 792.0);
        add_bookmark(&mut b_doc1, "Chapter 1", 0).expect("bm1");

        let mut b_doc2 = PdfDocument::new();
        b_doc2.add_blank_page(612.0, 792.0);
        b_doc2.add_blank_page(612.0, 792.0);
        add_bookmark(&mut b_doc2, "Chapter 2", 0).expect("bm2");

        let b_merged = merge_documents_with_options(&[b_doc1, b_doc2], true).expect("merge with bm");
        assert_eq!(b_merged.page_count(), 3);
        let bms = get_document_bookmarks(&b_merged).expect("get bms");
        assert_eq!(bms.len(), 2);
        assert_eq!(bms[0].title, "Chapter 1");
        assert_eq!(bms[0].page_index, Some(0));
        assert_eq!(bms[1].title, "Chapter 2");
        assert_eq!(bms[1].page_index, Some(1));

        // Test split by bookmarks (FR-07.4)
        let by_bm = split_document_by_bookmarks(&b_merged).expect("split by bm");
        assert_eq!(by_bm.len(), 2);
        assert_eq!(by_bm[0].0, "Chapter 1");
        assert_eq!(by_bm[0].1.page_count(), 1);
        assert_eq!(by_bm[1].0, "Chapter 2");
        assert_eq!(by_bm[1].1.page_count(), 2);
    }
}
