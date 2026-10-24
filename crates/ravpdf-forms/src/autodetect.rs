//! Form Field Auto-Detection Engine.
//!
//! Traceability:
//! - FR-11.3.1 (Auto-detect potential form fields in static documents and suggest interactive field placement)

use crate::field::{add_form_field, FormField};
use ravpdf_core::document::PdfDocument;
use ravpdf_core::error::{PdfError, Result};
use ravpdf_core::object::{ObjectId, Object};

/// Auto-detected form field suggestion (FR-11.3.1).
#[derive(Debug, Clone)]
pub struct AutoDetectedField {
    pub label: String,
    pub field: FormField,
    pub confidence: f32,
}

/// Analyzes page content stream text and vector rules to detect potential form fields (FR-11.3.1).
pub fn auto_detect_form_fields(
    doc: &PdfDocument,
    page_index: usize,
) -> Result<Vec<AutoDetectedField>> {
    let page_ids = doc.page_ids();
    let page_id = *page_ids
        .get(page_index)
        .ok_or_else(|| PdfError::InvalidStructure(format!("Page {} out of range", page_index)))?;

    let page_obj = doc
        .objects
        .get(&page_id)
        .and_then(Object::as_dict)
        .ok_or_else(|| PdfError::InvalidStructure("Expected page dictionary".into()))?;

    // Extract text operations from /Contents
    let content_bytes = extract_page_contents(doc, page_obj)?;
    let content_text = String::from_utf8_lossy(&content_bytes);

    let mut detected = Vec::new();
    let mut field_idx = 1;

    // Scan text blocks for common form label patterns
    let labels_and_coords = parse_content_text_and_positions(&content_text);

    for (label, x, y, width) in labels_and_coords {
        let trimmed = label.trim();
        let lower = trimmed.to_lowercase();

        // 1. Signature fields
        if lower.contains("signature") || lower.contains("sign here") || lower.starts_with("signed:") {
            let rect = [x + width + 10.0, y - 5.0, x + width + 180.0, y + 30.0];
            let name = format!("Signature_{}", field_idx);
            let field = FormField::new_signature(name, rect);
            detected.push(AutoDetectedField {
                label: trimmed.to_string(),
                field,
                confidence: 0.95,
            });
            field_idx += 1;
            continue;
        }

        // 2. Checkbox markers ([ ], ( ), Checkbox)
        if trimmed == "[ ]" || trimmed == "[]" || trimmed == "( )" || lower.starts_with("agree") || lower.starts_with("i accept") {
            let rect = [x, y, x + 16.0, y + 16.0];
            let name = format!("Checkbox_{}", field_idx);
            let field = FormField::new_checkbox(name, rect, false);
            detected.push(AutoDetectedField {
                label: trimmed.to_string(),
                field,
                confidence: 0.90,
            });
            field_idx += 1;
            continue;
        }

        // 3. Multi-line text (Notes, Comments, Description, Remarks)
        if lower.starts_with("notes:") || lower.starts_with("comments:") || lower.starts_with("description:") || lower.starts_with("remarks:") {
            let rect = [x, y - 60.0, x + 400.0, y - 5.0];
            let clean_name = clean_field_name(trimmed);
            let field = FormField::new_multiline_text(clean_name, rect);
            detected.push(AutoDetectedField {
                label: trimmed.to_string(),
                field,
                confidence: 0.88,
            });
            field_idx += 1;
            continue;
        }

        // 4. Standard single-line text fields with colon (e.g., "Name:", "Date:", "Email:", "Address:")
        if trimmed.ends_with(':') || lower.ends_with("date") || lower.ends_with("phone") || lower.ends_with("email") {
            let field_width = if lower.contains("date") || lower.contains("zip") {
                90.0
            } else if lower.contains("address") || lower.contains("company") {
                250.0
            } else {
                160.0
            };

            let rect = [x + width + 8.0, y - 2.0, x + width + 8.0 + field_width, y + 16.0];
            let clean_name = clean_field_name(trimmed);
            let field = FormField::new_text(clean_name, rect);
            detected.push(AutoDetectedField {
                label: trimmed.to_string(),
                field,
                confidence: 0.85,
            });
            field_idx += 1;
        }
    }

    Ok(detected)
}

/// Applies a list of auto-detected form fields to the target page (FR-11.3.1).
pub fn apply_auto_detected_fields(
    doc: &mut PdfDocument,
    page_index: usize,
    detected_fields: &[AutoDetectedField],
) -> Result<Vec<ObjectId>> {
    let mut added_ids = Vec::new();
    for item in detected_fields {
        let id = add_form_field(doc, page_index, &item.field)?;
        added_ids.push(id);
    }
    Ok(added_ids)
}

fn clean_field_name(label: &str) -> String {
    let stripped = label.trim_end_matches(':').trim();
    let safe: String = stripped
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '_' { c } else { '_' })
        .collect();
    if safe.is_empty() {
        "Field".to_string()
    } else {
        safe
    }
}

fn extract_page_contents(doc: &PdfDocument, page_dict: &ravpdf_core::object::PdfDictionary) -> Result<Vec<u8>> {
    if let Some(contents_obj) = page_dict.get("Contents") {
        match contents_obj {
            Object::Reference(ref_id) => {
                if let Some(stream) = doc.objects.get(ref_id).and_then(Object::as_stream) {
                    return stream.decode();
                }
            }
            Object::Stream(s) => {
                return s.decode();
            }
            Object::Array(arr) => {
                let mut merged = Vec::new();
                for item in arr {
                    if let Some(ref_id) = item.as_reference() {
                        if let Some(stream) = doc.objects.get(&ref_id).and_then(Object::as_stream) {
                            if let Ok(decompressed) = stream.decode() {
                                merged.extend_from_slice(&decompressed);
                            }
                        }
                    }
                }
                return Ok(merged);
            }
            _ => {}
        }
    }
    Ok(Vec::new())
}

/// Parses content stream text operators and derives simulated positions.
fn parse_content_text_and_positions(content: &str) -> Vec<(String, f64, f64, f64)> {
    let mut results = Vec::new();
    let mut cur_x = 72.0;
    let mut cur_y = 700.0;

    let chars = content.chars().collect::<Vec<_>>();
    let mut i = 0;
    let len = chars.len();

    let mut prev_tokens: Vec<String> = Vec::new();

    while i < len {
        // Skip whitespace
        while i < len && chars[i].is_whitespace() {
            i += 1;
        }
        if i >= len {
            break;
        }

        // String literal: (...)
        if chars[i] == '(' {
            let mut s = String::new();
            let mut depth = 1;
            i += 1;
            while i < len && depth > 0 {
                if chars[i] == '\\' && i + 1 < len {
                    s.push(chars[i + 1]);
                    i += 2;
                } else if chars[i] == '(' {
                    depth += 1;
                    s.push('(');
                    i += 1;
                } else if chars[i] == ')' {
                    depth -= 1;
                    if depth > 0 {
                        s.push(')');
                    }
                    i += 1;
                } else {
                    s.push(chars[i]);
                    i += 1;
                }
            }
            prev_tokens.push(s);
            continue;
        }

        // Regular word / number / operator token
        let start = i;
        while i < len
            && !chars[i].is_whitespace()
            && chars[i] != '('
            && chars[i] != ')'
            && chars[i] != '<'
            && chars[i] != '>'
            && chars[i] != '['
            && chars[i] != ']'
        {
            i += 1;
        }
        let token = chars[start..i].iter().collect::<String>();

        match token.as_str() {
            "Td" | "TD" => {
                if prev_tokens.len() >= 2 {
                    if let (Ok(dx), Ok(dy)) = (
                        prev_tokens[prev_tokens.len() - 2].parse::<f64>(),
                        prev_tokens[prev_tokens.len() - 1].parse::<f64>(),
                    ) {
                        cur_x += dx;
                        cur_y += dy;
                    }
                }
                prev_tokens.clear();
            }
            "Tm" => {
                if prev_tokens.len() >= 6 {
                    if let (Ok(e), Ok(f)) = (
                        prev_tokens[prev_tokens.len() - 2].parse::<f64>(),
                        prev_tokens[prev_tokens.len() - 1].parse::<f64>(),
                    ) {
                        cur_x = e;
                        cur_y = f;
                    }
                }
                prev_tokens.clear();
            }
            "Tj" => {
                if let Some(text) = prev_tokens.pop() {
                    let width = (text.len() as f64) * 6.5;
                    results.push((text, cur_x, cur_y, width));
                    cur_x += width;
                }
                prev_tokens.clear();
            }
            _ => {
                prev_tokens.push(token);
                if prev_tokens.len() > 10 {
                    prev_tokens.remove(0);
                }
            }
        }
    }

    results
}

#[cfg(test)]
mod tests {
    use super::*;
    use ravpdf_core::object::{PdfDictionary, PdfStream};

    #[test]
    fn test_auto_detect_fields_from_content() {
        let mut doc = PdfDocument::new();
        doc.add_blank_page(612.0, 792.0);

        let form_content = b"BT /F1 12 Tf 72 700 Td (Full Name:) Tj 0 -40 Td (Email:) Tj 0 -40 Td (Date:) Tj 0 -40 Td (Signature:) Tj ET";
        let stream_id = doc.next_object_id();
        let stream = PdfStream::with_plain_data(PdfDictionary::new(), form_content.to_vec());
        doc.objects.insert(stream_id, Object::Stream(stream));

        let page_id = doc.page_ids()[0];
        if let Some(p) = doc.objects.get_mut(&page_id).and_then(Object::as_dict_mut) {
            p.insert("Contents", Object::Reference(stream_id));
        }

        let detected = auto_detect_form_fields(&doc, 0).expect("auto detect form fields");
        assert_eq!(detected.len(), 4);

        let names: Vec<String> = detected.iter().map(|d| d.field.name.clone()).collect();
        assert!(names.iter().any(|n| n.contains("Full_Name")));
        assert!(names.iter().any(|n| n.contains("Email")));
        assert!(names.iter().any(|n| n.contains("Date")));
        assert!(names.iter().any(|n| n.contains("Signature")));

        // Apply auto-detected fields to document
        let ids = apply_auto_detected_fields(&mut doc, 0, &detected).expect("apply fields");
        assert_eq!(ids.len(), 4);

        let page_fields = crate::field::get_page_form_fields(&doc, 0).expect("get page fields");
        assert_eq!(page_fields.len(), 4);
    }
}
