//! Collaboration and Review Workflows conforming to ISO 32000-1 Clause 12.5.
//!
//! Traceability:
//! - FR-31.3 (Merge annotations from multiple reviewer copies into a single master)
//! - FR-31.4 (Compare annotations between reviewer copies)
//! - FR-31.5 (Generate annotation summary reports exportable as CSV or structured report)

use ravpdf_annot::{add_annotation_to_page, get_page_annotations, Annotation};
use ravpdf_core::document::PdfDocument;
use ravpdf_core::error::Result;
use serde::{Deserialize, Serialize};

/// Structured representation of a review comment or markup item (FR-31.5).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReviewComment {
    /// 0-based page index.
    pub page_index: usize,
    /// Author / Reviewer who created the markup.
    pub author: String,
    /// Classification name (e.g., "Sticky Note", "Highlight", "FreeText", "Stamp", "Link").
    pub comment_type: String,
    /// Text content or review note.
    pub contents: String,
    /// Bounding rectangle in PDF coordinates [x1, y1, x2, y2].
    pub rect: [f64; 4],
    /// Review status (e.g. "Accepted", "Rejected", "Pending").
    pub status: Option<String>,
}

/// Discrepancy comparison between reviewer and master copies (FR-31.4).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AnnotationDiff {
    pub unique_to_reviewer: Vec<ReviewComment>,
    pub unique_to_master: Vec<ReviewComment>,
    pub matching_comments: Vec<ReviewComment>,
}

/// Retrieves all review comments and markup items across the entire document (FR-31.5).
pub fn get_document_comments(doc: &PdfDocument) -> Result<Vec<ReviewComment>> {
    let mut comments = Vec::new();
    let total_pages = doc.page_count();

    for page_idx in 0..total_pages {
        let annots = get_page_annotations(doc, page_idx).unwrap_or_default();
        for annot in annots {
            let author = annot.author.clone().unwrap_or_else(|| "Anonymous".to_string());
            let contents = annot.contents.clone().unwrap_or_default();
            let comment_type = annot.annot_type.subtype_name().to_string();

            comments.push(ReviewComment {
                page_index: page_idx,
                author,
                comment_type,
                contents,
                rect: annot.rect,
                status: None,
            });
        }
    }

    Ok(comments)
}

/// Exports review comments as an RFC 4180 compliant CSV document (FR-31.5).
pub fn export_comments_to_csv(comments: &[ReviewComment]) -> String {
    let mut csv = String::new();
    csv.push_str("Page,Author,Type,Status,BoundingBox,Comment\n");

    for c in comments {
        let bbox = format!("[{:.1}, {:.1}, {:.1}, {:.1}]", c.rect[0], c.rect[1], c.rect[2], c.rect[3]);
        let page_label = (c.page_index + 1).to_string();
        let status = c.status.as_deref().unwrap_or("Unspecified");

        csv.push_str(&format!(
            "{},{},{},{},{},{}\n",
            escape_csv_field(&page_label),
            escape_csv_field(&c.author),
            escape_csv_field(&c.comment_type),
            escape_csv_field(status),
            escape_csv_field(&bbox),
            escape_csv_field(&c.contents)
        ));
    }

    csv
}

/// Merges annotations from a reviewer copy into a master PDF document (FR-31.3).
///
/// Returns the number of new annotations imported.
pub fn merge_reviewer_annotations(
    master: &mut PdfDocument,
    reviewer: &PdfDocument,
) -> Result<usize> {
    let mut imported_count = 0;
    let page_count = master.page_count().min(reviewer.page_count());

    for page_idx in 0..page_count {
        let master_annots = get_page_annotations(master, page_idx).unwrap_or_default();
        let reviewer_annots = get_page_annotations(reviewer, page_idx).unwrap_or_default();

        for rev_annot in reviewer_annots {
            // Check if master page already has this identical annotation
            let already_exists = master_annots.iter().any(|m| {
                m.annot_type == rev_annot.annot_type
                    && (m.rect[0] - rev_annot.rect[0]).abs() < 1.0
                    && (m.rect[1] - rev_annot.rect[1]).abs() < 1.0
                    && (m.rect[2] - rev_annot.rect[2]).abs() < 1.0
                    && (m.rect[3] - rev_annot.rect[3]).abs() < 1.0
                    && m.contents == rev_annot.contents
            });

            if !already_exists {
                let mut new_annot = Annotation::new(rev_annot.annot_type.clone(), rev_annot.rect);
                new_annot.contents = rev_annot.contents.clone();
                new_annot.author = rev_annot.author.clone();
                new_annot.color = rev_annot.color;
                new_annot.opacity = rev_annot.opacity;

                add_annotation_to_page(master, page_idx, &new_annot)?;
                imported_count += 1;
            }
        }
    }

    Ok(imported_count)
}

/// Compares annotations between master and reviewer documents (FR-31.4).
pub fn compare_reviewer_comments(
    master: &PdfDocument,
    reviewer: &PdfDocument,
) -> Result<AnnotationDiff> {
    let master_comments = get_document_comments(master)?;
    let reviewer_comments = get_document_comments(reviewer)?;

    let mut diff = AnnotationDiff::default();

    for rev in &reviewer_comments {
        let matched = master_comments.iter().any(|m| comments_match(m, rev));
        if matched {
            diff.matching_comments.push(rev.clone());
        } else {
            diff.unique_to_reviewer.push(rev.clone());
        }
    }

    for m in &master_comments {
        let matched = reviewer_comments.iter().any(|rev| comments_match(m, rev));
        if !matched {
            diff.unique_to_master.push(m.clone());
        }
    }

    Ok(diff)
}

fn comments_match(a: &ReviewComment, b: &ReviewComment) -> bool {
    a.page_index == b.page_index
        && a.comment_type == b.comment_type
        && (a.rect[0] - b.rect[0]).abs() < 1.0
        && (a.rect[1] - b.rect[1]).abs() < 1.0
        && a.contents == b.contents
}

/// Helper to sanitize CSV field with proper RFC 4180 quotation.
fn escape_csv_field(field: &str) -> String {
    if field.contains(',') || field.contains('"') || field.contains('\n') || field.contains('\r') {
        let escaped = field.replace('"', "\"\"");
        format!("\"{}\"", escaped)
    } else {
        field.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ravpdf_annot::AnnotationType;

    #[test]
    fn test_comments_summary_csv_and_merge() {
        let mut master = PdfDocument::new();
        master.add_blank_page(612.0, 792.0);

        let mut reviewer = PdfDocument::new();
        reviewer.add_blank_page(612.0, 792.0);

        // Add annotation in reviewer copy
        let mut note = Annotation::new(AnnotationType::Text, [100.0, 600.0, 130.0, 630.0]);
        note.contents = Some("Please revise paragraph 2.".into());
        note.author = Some("Lead Editor".into());
        add_annotation_to_page(&mut reviewer, 0, &note).expect("add note");

        // Extract comments
        let comments = get_document_comments(&reviewer).expect("get comments");
        assert_eq!(comments.len(), 1);
        assert_eq!(comments[0].author, "Lead Editor");
        assert_eq!(comments[0].contents, "Please revise paragraph 2.");

        // Export to CSV
        let csv = export_comments_to_csv(&comments);
        assert!(csv.contains("Lead Editor"));
        assert!(csv.contains("Please revise paragraph 2."));
        assert!(csv.contains("Page,Author,Type"));

        // Compare before merge
        let diff = compare_reviewer_comments(&master, &reviewer).expect("compare comments");
        assert_eq!(diff.unique_to_reviewer.len(), 1);
        assert_eq!(diff.matching_comments.len(), 0);

        // Merge reviewer comments into master
        let merged_count = merge_reviewer_annotations(&mut master, &reviewer).expect("merge comments");
        assert_eq!(merged_count, 1);

        // Compare after merge
        let diff_after = compare_reviewer_comments(&master, &reviewer).expect("compare after");
        assert_eq!(diff_after.unique_to_reviewer.len(), 0);
        assert_eq!(diff_after.matching_comments.len(), 1);
    }
}
