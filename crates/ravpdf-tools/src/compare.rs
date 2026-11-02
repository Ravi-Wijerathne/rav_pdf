//! PDF Document Comparison Engine.
//!
//! Traceability:
//! - FR-17.1 (Compare two PDF documents and generate difference report)
//! - FR-17.3 (Highlight and categorize differences as text additions/deletions/modifications)
//! - FR-17.4 (Summary panel listing all identified changes)
//! - FR-17.7 (Compare text mode)

use crate::metadata::get_document_info;
use crate::search::extract_page_text;
use ravpdf_core::document::PdfDocument;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Categorized page-level differences between two PDF documents (FR-17.3).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageDiff {
    pub page_number: usize,
    pub has_differences: bool,
    pub additions: Vec<String>,
    pub deletions: Vec<String>,
    pub summary: String,
}

/// Comprehensive comparison report between two documents (FR-17.1, FR-17.4).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ComparisonReport {
    pub doc_a_pages: usize,
    pub doc_b_pages: usize,
    pub metadata_diffs: Vec<String>,
    pub page_diffs: Vec<PageDiff>,
    pub total_differences: usize,
}

impl ComparisonReport {
    pub fn is_identical(&self) -> bool {
        self.total_differences == 0
    }
}

/// Compares two PDF documents and generates a detailed change report (FR-17.1).
pub fn compare_documents(doc_a: &PdfDocument, doc_b: &PdfDocument) -> ComparisonReport {
    let mut report = ComparisonReport {
        doc_a_pages: doc_a.page_count(),
        doc_b_pages: doc_b.page_count(),
        metadata_diffs: Vec::new(),
        page_diffs: Vec::new(),
        total_differences: 0,
    };

    // 1. Compare Document Metadata (/Info)
    let info_a = get_document_info(doc_a).unwrap_or_default();
    let info_b = get_document_info(doc_b).unwrap_or_default();

    if info_a.title != info_b.title {
        report.metadata_diffs.push(format!(
            "Title changed: '{}' ➔ '{}'",
            info_a.title.as_deref().unwrap_or("<empty>"),
            info_b.title.as_deref().unwrap_or("<empty>")
        ));
    }
    if info_a.author != info_b.author {
        report.metadata_diffs.push(format!(
            "Author changed: '{}' ➔ '{}'",
            info_a.author.as_deref().unwrap_or("<empty>"),
            info_b.author.as_deref().unwrap_or("<empty>")
        ));
    }
    if info_a.subject != info_b.subject {
        report.metadata_diffs.push(format!(
            "Subject changed: '{}' ➔ '{}'",
            info_a.subject.as_deref().unwrap_or("<empty>"),
            info_b.subject.as_deref().unwrap_or("<empty>")
        ));
    }
    report.total_differences += report.metadata_diffs.len();

    // 2. Compare Page Counts
    if report.doc_a_pages != report.doc_b_pages {
        report.total_differences += 1;
    }

    // 3. Compare Content on each Page
    let max_pages = report.doc_a_pages.max(report.doc_b_pages);

    for p in 0..max_pages {
        let page_num = p + 1;
        let mut additions = Vec::new();
        let mut deletions = Vec::new();

        if p >= report.doc_a_pages {
            additions.push(format!("Page {} was added in new document", page_num));
            report.total_differences += 1;
            report.page_diffs.push(PageDiff {
                page_number: page_num,
                has_differences: true,
                additions,
                deletions,
                summary: "Page added".into(),
            });
            continue;
        }

        if p >= report.doc_b_pages {
            deletions.push(format!(
                "Page {} was deleted from original document",
                page_num
            ));
            report.total_differences += 1;
            report.page_diffs.push(PageDiff {
                page_number: page_num,
                has_differences: true,
                additions,
                deletions,
                summary: "Page deleted".into(),
            });
            continue;
        }

        // Both documents have this page; compare extracted lines
        let text_a = extract_page_text(doc_a, p).unwrap_or_default();
        let text_b = extract_page_text(doc_b, p).unwrap_or_default();

        let lines_a: Vec<&str> = text_a
            .lines()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .collect();
        let lines_b: Vec<&str> = text_b
            .lines()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .collect();

        let set_a: HashSet<&str> = lines_a.iter().copied().collect();
        let set_b: HashSet<&str> = lines_b.iter().copied().collect();

        for line in &lines_b {
            if !set_a.contains(line) {
                additions.push((*line).to_string());
            }
        }

        for line in &lines_a {
            if !set_b.contains(line) {
                deletions.push((*line).to_string());
            }
        }

        let has_diff = !additions.is_empty() || !deletions.is_empty();
        if has_diff {
            report.total_differences += additions.len() + deletions.len();
        }

        let summary = if !has_diff {
            "Identical".to_string()
        } else {
            format!(
                "+{} additions, -{} deletions",
                additions.len(),
                deletions.len()
            )
        };

        report.page_diffs.push(PageDiff {
            page_number: page_num,
            has_differences: has_diff,
            additions,
            deletions,
            summary,
        });
    }

    report
}

/// Generates an ISO 32000-compliant PDF comparison report document from a `ComparisonReport`.
///
/// Conforms to ISO 32000-1 and SRS FR-17.1, FR-17.4, FR-37.2.
pub fn generate_comparison_pdf_report(
    report: &ComparisonReport,
    name_a: &str,
    name_b: &str,
) -> Result<PdfDocument, ravpdf_core::error::PdfError> {
    use ravpdf_edit::{create_pdf_from_markdown, MarkdownPdfOptions};

    let mut md = String::new();
    md.push_str("# RavPDF Document Comparison Report\n\n");
    md.push_str(&format!("**Original Document:** {}\n\n", name_a));
    md.push_str(&format!("**Modified Document:** {}\n\n", name_b));
    md.push_str(&format!("- Original Page Count: {}\n", report.doc_a_pages));
    md.push_str(&format!("- Modified Page Count: {}\n", report.doc_b_pages));
    md.push_str(&format!(
        "- Total Differences Identified: {}\n",
        report.total_differences
    ));
    let status_str = if report.is_identical() {
        "**Status:** IDENTICAL (No differences found)"
    } else {
        "**Status:** DIFFERENCES DETECTED"
    };
    md.push_str(&format!("- {}\n\n", status_str));

    md.push_str("## Metadata Differences\n\n");
    if report.metadata_diffs.is_empty() {
        md.push_str("None. Document information dictionary entries match.\n\n");
    } else {
        for m in &report.metadata_diffs {
            md.push_str(&format!("- {}\n", m));
        }
        md.push('\n');
    }

    md.push_str("## Detailed Page Changes\n\n");
    if report.page_diffs.iter().all(|p| !p.has_differences) {
        md.push_str("All pages have matching text content.\n\n");
    } else {
        for p in &report.page_diffs {
            if p.has_differences {
                md.push_str(&format!("### Page {} — {}\n\n", p.page_number, p.summary));
                for add in &p.additions {
                    md.push_str(&format!("- **[ADD]** `{}`\n", add));
                }
                for del in &p.deletions {
                    md.push_str(&format!("- **[DEL]** `{}`\n", del));
                }
                md.push('\n');
            }
        }
    }

    let opts = MarkdownPdfOptions {
        base_font_size: 10.5,
        margin: 54.0,
        title: Some("Document Comparison Report".to_string()),
        ..Default::default()
    };

    create_pdf_from_markdown(&md, &opts)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metadata::{set_document_info, DocumentInfo};
    use ravpdf_edit::{create_pdf_from_text, TextPdfOptions};

    #[test]
    fn test_compare_documents() {
        let text_v1 =
            "Invoice #1001\nDue Date: 2026-01-01\nTotal: $500.00\nThank you for your business!";
        let text_v2 = "Invoice #1001\nDue Date: 2026-02-01\nTotal: $750.00\nPayment Terms: Net 30\nThank you for your business!";

        let opts = TextPdfOptions::default();
        let mut doc1 = create_pdf_from_text(text_v1, &opts).expect("doc1");
        let mut doc2 = create_pdf_from_text(text_v2, &opts).expect("doc2");

        let info1 = DocumentInfo {
            title: Some("Invoice 1001 Original".into()),
            ..Default::default()
        };
        set_document_info(&mut doc1, &info1).expect("info1");

        let info2 = DocumentInfo {
            title: Some("Invoice 1001 Revised".into()),
            ..Default::default()
        };
        set_document_info(&mut doc2, &info2).expect("info2");

        let report = compare_documents(&doc1, &doc2);
        assert!(!report.is_identical());
        assert_eq!(report.metadata_diffs.len(), 1);
        assert!(report.metadata_diffs[0].contains("Title changed"));
        assert_eq!(report.page_diffs.len(), 1);

        let p1 = &report.page_diffs[0];
        assert!(p1.has_differences);
        assert!(p1.additions.iter().any(|s| s.contains("$750.00")));
        assert!(p1.deletions.iter().any(|s| s.contains("$500.00")));
    }
}
