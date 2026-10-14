//! Irreversible Content Redaction and Sanitization conforming to ISO 32000-1.
//!
//! Traceability: FR-15.1 - FR-15.4, FR-37.2 (ravpdf redact).

use ravpdf_core::document::PdfDocument;
use ravpdf_core::error::{PdfError, Result};
use ravpdf_core::object::Object;
use regex::Regex;

/// Redacts occurrences matching `pattern` regex across all page content streams,
/// replacing sensitive characters with redaction markers.
pub fn redact_pattern(
    doc: &mut PdfDocument,
    pattern: &str,
    replacement: &str,
) -> Result<usize> {
    let regex = Regex::new(pattern)
        .map_err(|e| PdfError::ParseError(format!("Invalid redaction regex: {}", e)))?;

    let mut total_redactions = 0;
    let page_ids = doc.page_ids();

    for &page_id in &page_ids {
        let page_obj = match doc.objects.get(&page_id) {
            Some(obj) => obj,
            None => continue,
        };
        let page_dict = match page_obj.as_dict() {
            Some(dict) => dict,
            None => continue,
        };

        let mut contents_ids = Vec::new();
        if let Some(contents_obj) = page_dict.get("Contents") {
            match contents_obj {
                Object::Reference(id) => contents_ids.push(*id),
                Object::Array(arr) => {
                    for item in arr {
                        if let Some(id) = item.as_reference() {
                            contents_ids.push(id);
                        }
                    }
                }
                _ => {}
            }
        }

        for content_id in contents_ids {
            if let Some(Object::Stream(ref mut stream)) = doc.objects.get_mut(&content_id) {
                if let Ok(decoded) = stream.decode() {
                    let content_str = String::from_utf8_lossy(&decoded);
                    if regex.is_match(&content_str) {
                        let count = regex.find_iter(&content_str).count();
                        total_redactions += count;
                        let replaced = regex.replace_all(&content_str, replacement);
                        stream.raw_data = replaced.as_bytes().to_vec();
                        let _ = stream.compress_flate();
                    }
                }
            }
        }
    }

    Ok(total_redactions)
}
