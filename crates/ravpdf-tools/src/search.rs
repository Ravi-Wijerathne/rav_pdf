//! Document Search and Text Extraction conforming to ISO 32000-1.
//!
//! Traceability: FR-34.1 - FR-34.4, FR-37.2 (ravpdf extract-text).

use ravpdf_core::document::PdfDocument;
use ravpdf_core::error::{PdfError, Result};
use ravpdf_core::lexer::{Lexer, Token};
use regex::Regex;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchMatch {
    pub page_index: usize,
    pub match_text: String,
    pub context_snippet: String,
}

/// Extracts all legible text from a single page's content stream.
pub fn extract_page_text(doc: &PdfDocument, page_index: usize) -> Result<String> {
    let content_bytes = doc.get_page_content(page_index).unwrap_or_default();
    if content_bytes.is_empty() {
        return Ok(String::new());
    }

    let mut lexer = Lexer::new(&content_bytes);
    let mut page_text = String::new();
    let mut last_was_text = false;

    while let Ok(token) = lexer.next_token() {
        match token {
            Token::Eof => break,
            Token::StringLiteral(bytes) => {
                let s = String::from_utf8_lossy(&bytes);
                if !page_text.is_empty() && !last_was_text {
                    page_text.push(' ');
                }
                page_text.push_str(&s);
                last_was_text = true;
            }
            Token::HexString(bytes) => {
                let s = String::from_utf8_lossy(&bytes);
                if !page_text.is_empty() && !last_was_text {
                    page_text.push(' ');
                }
                page_text.push_str(&s);
                last_was_text = true;
            }
            Token::Keyword(kw) => {
                if kw == "T*" || kw == "ET" {
                    page_text.push('\n');
                    last_was_text = false;
                }
            }
            _ => {
                last_was_text = false;
            }
        }
    }

    Ok(page_text.trim().to_string())
}

/// Extracts all text across the entire document.
pub fn extract_all_text(doc: &PdfDocument) -> Result<String> {
    let mut full_doc_text = String::new();
    let total_pages = doc.page_count();

    for i in 0..total_pages {
        let page_text = extract_page_text(doc, i)?;
        if !page_text.is_empty() {
            if !full_doc_text.is_empty() {
                full_doc_text.push_str("\n\n--- Page Break ---\n\n");
            }
            full_doc_text.push_str(&page_text);
        }
    }

    Ok(full_doc_text)
}

/// Searches for occurrences of a string or regular expression across all document pages.
pub fn search_document(
    doc: &PdfDocument,
    pattern: &str,
    is_regex: bool,
    case_sensitive: bool,
) -> Result<Vec<SearchMatch>> {
    let mut matches = Vec::new();

    let regex = if is_regex {
        let pattern_str = if case_sensitive {
            pattern.to_string()
        } else {
            format!("(?i){}", pattern)
        };
        Regex::new(&pattern_str)
            .map_err(|e| PdfError::ParseError(format!("Invalid search regex: {}", e)))?
    } else {
        let escaped = regex::escape(pattern);
        let pattern_str = if case_sensitive {
            escaped
        } else {
            format!("(?i){}", escaped)
        };
        Regex::new(&pattern_str)
            .map_err(|e| PdfError::ParseError(format!("Invalid search pattern: {}", e)))?
    };

    let total_pages = doc.page_count();

    for page_idx in 0..total_pages {
        let text = extract_page_text(doc, page_idx)?;
        for mat in regex.find_iter(&text) {
            let start = mat.start().saturating_sub(30);
            let end = (mat.end() + 30).min(text.len());
            let context_snippet = format!("...{}...", &text[start..end]);

            matches.push(SearchMatch {
                page_index: page_idx,
                match_text: mat.as_str().to_string(),
                context_snippet,
            });
        }
    }

    Ok(matches)
}
