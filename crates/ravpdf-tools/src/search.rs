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

use ravpdf_core::object::Object;

/// Builds a regex with case-sensitive and whole-word options.
pub fn build_search_regex(
    pattern: &str,
    case_sensitive: bool,
    whole_word: bool,
    is_regex: bool,
) -> Result<Regex> {
    let mut pattern_str = if is_regex {
        pattern.to_string()
    } else {
        regex::escape(pattern)
    };

    if whole_word {
        pattern_str = format!(r"\b{}\b", pattern_str);
    }

    if !case_sensitive {
        pattern_str = format!("(?i){}", pattern_str);
    }

    Regex::new(&pattern_str)
        .map_err(|e| PdfError::ParseError(format!("Invalid search regex: {}", e)))
}

/// Searches for occurrences of a string or regular expression across all document pages.
pub fn search_document(
    doc: &PdfDocument,
    pattern: &str,
    is_regex: bool,
    case_sensitive: bool,
) -> Result<Vec<SearchMatch>> {
    let mut matches = Vec::new();
    let regex = build_search_regex(pattern, case_sensitive, false, is_regex)?;
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

/// Replaces occurrences of a search pattern with replacement text on a specific page.
///
/// Returns the number of replacements made.
/// Conforms to ISO 32000-1 Section 7.3.4. Traceability: FR-04.8.
pub fn find_and_replace_page(
    doc: &mut PdfDocument,
    page_index: usize,
    pattern: &str,
    replacement: &str,
    case_sensitive: bool,
    whole_word: bool,
    is_regex: bool,
) -> Result<usize> {
    let regex = build_search_regex(pattern, case_sensitive, whole_word, is_regex)?;

    let page = doc.get_page(page_index).ok_or_else(|| {
        PdfError::InvalidStructure(format!("Page index {} out of range", page_index))
    })?;

    let contents_ids = page.contents_ids;
    let mut total_replacements = 0;

    for content_id in contents_ids {
        if let Some(Object::Stream(ref mut stream)) = doc.objects.get_mut(&content_id) {
            let decoded = stream.decode()?;
            let (new_bytes, count) = replace_in_content_stream(&decoded, &regex, replacement);
            if count > 0 {
                total_replacements += count;
                stream.raw_data = new_bytes;
                let _ = stream.compress_flate();
                stream.update_length();
            }
        }
    }

    Ok(total_replacements)
}

/// Replaces occurrences of a search pattern with replacement text across all pages in the document.
///
/// Returns the total number of text replacements made.
/// Conforms to ISO 32000-1 Section 7.3.4. Traceability: FR-04.8.
pub fn find_and_replace(
    doc: &mut PdfDocument,
    pattern: &str,
    replacement: &str,
    case_sensitive: bool,
    whole_word: bool,
    is_regex: bool,
) -> Result<usize> {
    let total_pages = doc.page_count();
    let mut grand_total = 0;

    for i in 0..total_pages {
        let count = find_and_replace_page(
            doc,
            i,
            pattern,
            replacement,
            case_sensitive,
            whole_word,
            is_regex,
        )?;
        grand_total += count;
    }

    Ok(grand_total)
}

fn replace_in_content_stream(
    data: &[u8],
    regex: &Regex,
    replacement: &str,
) -> (Vec<u8>, usize) {
    let mut out = Vec::with_capacity(data.len());
    let mut count = 0;
    let mut i = 0;

    while i < data.len() {
        let b = data[i];

        if b == b'%' {
            // PDF comment: copy verbatim until newline
            out.push(b);
            i += 1;
            while i < data.len() {
                let c = data[i];
                out.push(c);
                i += 1;
                if c == b'\r' || c == b'\n' {
                    break;
                }
            }
        } else if b == b'(' {
            // Literal string: scan with balanced parens
            let start = i;
            i += 1;
            let mut depth = 1;
            let mut escaped = false;
            while i < data.len() && depth > 0 {
                let c = data[i];
                if escaped {
                    escaped = false;
                } else if c == b'\\' {
                    escaped = true;
                } else if c == b'(' {
                    depth += 1;
                } else if c == b')' {
                    depth -= 1;
                }
                i += 1;
            }

            let end = i; // One past closing ')'
            let raw_inner = if end > start + 2 {
                &data[start + 1..end - 1]
            } else {
                &[]
            };

            let decoded_str = decode_pdf_string(raw_inner);
            let matches = regex.find_iter(&decoded_str).count();

            if matches > 0 {
                count += matches;
                let replaced = regex.replace_all(&decoded_str, replacement);
                let re_escaped = escape_pdf_string(&replaced);
                out.push(b'(');
                out.extend_from_slice(re_escaped.as_bytes());
                out.push(b')');
            } else {
                out.extend_from_slice(&data[start..end]);
            }
        } else if b == b'<' && i + 1 < data.len() && data[i + 1] != b'<' {
            // Hex string: scan until '>'
            let start = i;
            i += 1;
            while i < data.len() && data[i] != b'>' {
                i += 1;
            }
            if i < data.len() {
                i += 1; // Include '>'
            }
            let end = i;
            let raw_hex = if end > start + 2 {
                &data[start + 1..end - 1]
            } else {
                &[]
            };

            if let Some(decoded_bytes) = decode_hex_string(raw_hex) {
                let decoded_str = String::from_utf8_lossy(&decoded_bytes);
                let matches = regex.find_iter(&decoded_str).count();

                if matches > 0 {
                    count += matches;
                    let replaced = regex.replace_all(&decoded_str, replacement);
                    let re_escaped = escape_pdf_string(&replaced);
                    out.push(b'(');
                    out.extend_from_slice(re_escaped.as_bytes());
                    out.push(b')');
                } else {
                    out.extend_from_slice(&data[start..end]);
                }
            } else {
                out.extend_from_slice(&data[start..end]);
            }
        } else {
            out.push(b);
            i += 1;
        }
    }

    (out, count)
}

fn decode_pdf_string(raw: &[u8]) -> String {
    let mut out = Vec::new();
    let mut i = 0;
    while i < raw.len() {
        if raw[i] == b'\\' && i + 1 < raw.len() {
            i += 1;
            match raw[i] {
                b'n' => out.push(b'\n'),
                b'r' => out.push(b'\r'),
                b't' => out.push(b'\t'),
                b'b' => out.push(0x08),
                b'f' => out.push(0x0C),
                b'(' => out.push(b'('),
                b')' => out.push(b')'),
                b'\\' => out.push(b'\\'),
                b'0'..=b'7' => {
                    let mut octal = (raw[i] - b'0') as u32;
                    let mut count = 1;
                    while count < 3 && i + 1 < raw.len() && (b'0'..=b'7').contains(&raw[i + 1]) {
                        i += 1;
                        octal = octal * 8 + (raw[i] - b'0') as u32;
                        count += 1;
                    }
                    out.push(octal as u8);
                }
                c => out.push(c),
            }
        } else {
            out.push(raw[i]);
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).to_string()
}

fn escape_pdf_string(s: &str) -> String {
    let mut escaped = String::with_capacity(s.len() + 10);
    for c in s.chars() {
        match c {
            '(' => escaped.push_str(r"\("),
            ')' => escaped.push_str(r"\)"),
            '\\' => escaped.push_str(r"\\"),
            '\r' => escaped.push_str(r"\r"),
            '\n' => escaped.push_str(r"\n"),
            '\t' => escaped.push_str(r"\t"),
            other => escaped.push(other),
        }
    }
    escaped
}

fn decode_hex_string(raw: &[u8]) -> Option<Vec<u8>> {
    let mut hex_digits = Vec::new();
    for &b in raw {
        if b.is_ascii_whitespace() {
            continue;
        }
        if b.is_ascii_hexdigit() {
            hex_digits.push(b);
        } else {
            return None;
        }
    }
    if hex_digits.len() % 2 != 0 {
        hex_digits.push(b'0');
    }
    let mut bytes = Vec::with_capacity(hex_digits.len() / 2);
    for chunk in hex_digits.chunks(2) {
        let hi = hex_char_to_val(chunk[0])?;
        let lo = hex_char_to_val(chunk[1])?;
        bytes.push((hi << 4) | lo);
    }
    Some(bytes)
}

fn hex_char_to_val(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_and_replace_content_stream() {
        let mut doc = PdfDocument::new();
        let content = b"BT /F1 12 Tf 72 712 Td (Hello World) Tj ET".to_vec();
        doc.add_page_with_content(612.0, 792.0, content);

        let initial_text = extract_page_text(&doc, 0).expect("extract");
        assert_eq!(initial_text, "Hello World");

        let replaced = find_and_replace(
            &mut doc,
            "World",
            "RavPDF Developer",
            true,
            false,
            false,
        )
        .expect("replace");

        assert_eq!(replaced, 1);
        let updated_text = extract_page_text(&doc, 0).expect("extract updated");
        assert_eq!(updated_text, "Hello RavPDF Developer");
    }
}

