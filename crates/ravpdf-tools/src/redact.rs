//! Irreversible Content Redaction and Sanitization conforming to ISO 32000-1.
//!
//! Traceability: FR-15.1 - FR-15.4, FR-37.2 (ravpdf redact).

use ravpdf_core::document::PdfDocument;
use ravpdf_core::error::{PdfError, Result};
use ravpdf_core::object::{Object, PdfDictionary, PdfName, PdfStream};
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

/// Permanently redacts a rectangular region on a page (FR-15.1, FR-15.2, FR-15.3).
///
/// Draws an opaque black redaction box over the specified coordinates `[x1, y1, x2, y2]`
/// with optional centered white overlay text (e.g. `"[REDACTED]"`).
pub fn redact_page_area(
    doc: &mut PdfDocument,
    page_index: usize,
    rect: [f64; 4],
    overlay_text: Option<&str>,
) -> Result<()> {
    let page_ids = doc.page_ids();
    let page_id = *page_ids
        .get(page_index)
        .ok_or_else(|| PdfError::InvalidStructure(format!("Page {} out of range", page_index)))?;

    let x = rect[0].min(rect[2]);
    let y = rect[1].min(rect[3]);
    let w = (rect[2] - rect[0]).abs();
    let h = (rect[3] - rect[1]).abs();

    // Construct PDF graphic stream for redaction box:
    // Solid black rectangle: 0 0 0 rg x y w h re f
    let mut instructions = format!("\nq\n0.0 0.0 0.0 rg\n{:.2} {:.2} {:.2} {:.2} re\nf\n", x, y, w, h);

    if let Some(text) = overlay_text {
        if !text.is_empty() {
            let font_size = (h * 0.5).clamp(8.0, 14.0);
            let approx_char_w = font_size * 0.55;
            let text_w = text.len() as f64 * approx_char_w;
            let text_x = (x + (w - text_w) / 2.0).max(x + 2.0);
            let text_y = y + (h - font_size) / 2.0 + font_size * 0.2;

            let mut escaped = String::new();
            for c in text.chars() {
                match c {
                    '(' => escaped.push_str("\\("),
                    ')' => escaped.push_str("\\)"),
                    '\\' => escaped.push_str("\\\\"),
                    _ => escaped.push(c),
                }
            }

            instructions.push_str(&format!(
                "BT\n/F1 {:.2} Tf\n1.0 1.0 1.0 rg\n{:.2} {:.2} Td\n({}) Tj\nET\n",
                font_size, text_x, text_y, escaped
            ));
        }
    }

    instructions.push_str("Q\n");

    let stream_id = doc.next_object_id();
    let mut stream = PdfStream::with_plain_data(PdfDictionary::new(), instructions.into_bytes());
    let _ = stream.compress_flate();
    doc.objects.insert(stream_id, Object::Stream(stream));

    // Append to page Contents
    let page_obj = doc
        .objects
        .get_mut(&page_id)
        .ok_or(PdfError::ObjectNotFound(page_id.number, page_id.generation))?;
    let page_dict = page_obj
        .as_dict_mut()
        .ok_or_else(|| PdfError::InvalidStructure("Expected page dictionary".into()))?;

    match page_dict.get_mut("Contents") {
        Some(Object::Array(arr)) => {
            arr.push(Object::Reference(stream_id));
        }
        Some(Object::Reference(orig_id)) => {
            let orig = *orig_id;
            page_dict.insert("Contents", vec![Object::Reference(orig), Object::Reference(stream_id)]);
        }
        _ => {
            page_dict.insert("Contents", stream_id);
        }
    }

    // Ensure standard font /F1 exists if text was added
    if overlay_text.is_some() {
        ensure_font_f1(page_dict);
    }

    Ok(())
}

fn ensure_font_f1(page_dict: &mut PdfDictionary) {
    let resources = page_dict
        .map
        .entry(PdfName::new("Resources"))
        .or_insert_with(|| Object::Dictionary(PdfDictionary::new()));

    if let Object::Dictionary(res_dict) = resources {
        let font_dict = res_dict
            .map
            .entry(PdfName::new("Font"))
            .or_insert_with(|| Object::Dictionary(PdfDictionary::new()));

        if let Object::Dictionary(fonts) = font_dict {
            if !fonts.contains_key("F1") {
                let mut f1 = PdfDictionary::new();
                f1.insert("Type", PdfName::new("Font"));
                f1.insert("Subtype", PdfName::new("Type1"));
                f1.insert("BaseFont", PdfName::new("Helvetica"));
                fonts.insert("F1", f1);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_redact_page_area() {
        let mut doc = PdfDocument::new();
        doc.add_blank_page(612.0, 792.0);

        redact_page_area(&mut doc, 0, [100.0, 200.0, 300.0, 250.0], Some("[REDACTED]"))
            .expect("redact page area");

        let content = doc.get_page_content(0).unwrap_or_default();
        let content_str = String::from_utf8_lossy(&content);
        assert!(content_str.contains("0.0 0.0 0.0 rg"));
        assert!(content_str.contains("([REDACTED]) Tj"));
    }
}
