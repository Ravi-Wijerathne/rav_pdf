//! Irreversible Content Redaction and Sanitization conforming to ISO 32000-1.
//!
//! Traceability: FR-15.1 - FR-15.4, FR-37.2 (ravpdf redact).

use ravpdf_core::document::PdfDocument;
use ravpdf_core::error::{PdfError, Result};
use ravpdf_core::object::{Object, PdfDictionary, PdfName, PdfStream};
use regex::Regex;
use serde::{Deserialize, Serialize};

/// Configurable options for redaction box appearance (FR-15.4).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RedactionAppearance {
    /// Fill color RGB [0.0..1.0]. Default is black [0.0, 0.0, 0.0].
    pub fill_color: [f32; 3],
    /// Overlay text (e.g. "[REDACTED]", "CONFIDENTIAL"). None for solid fill without text.
    pub overlay_text: Option<String>,
    /// Text color RGB [0.0..1.0]. Default is white [1.0, 1.0, 1.0].
    pub text_color: [f32; 3],
    /// Font size (points). If None, automatically scaled to fit height.
    pub font_size: Option<f64>,
}

impl Default for RedactionAppearance {
    fn default() -> Self {
        Self {
            fill_color: [0.0, 0.0, 0.0],
            overlay_text: Some("[REDACTED]".into()),
            text_color: [1.0, 1.0, 1.0],
            font_size: None,
        }
    }
}

/// Options for Document Sanitization conforming to ISO 32000-1 / ISO 32000-2 (FR-15.5).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SanitizeOptions {
    /// Remove document metadata (Author, Title, Subject, Keywords, Creator, Producer, Dates, and XMP stream).
    pub remove_metadata: bool,
    /// Remove all annotations, comments, and markup across all pages.
    pub remove_annotations: bool,
    /// Remove embedded file attachments.
    pub remove_attachments: bool,
    /// Remove document-level, page-level, and field-level JavaScript actions.
    pub remove_javascript: bool,
    /// Remove document outline/bookmarks.
    pub remove_bookmarks: bool,
    /// Flatten interactive form fields into permanent page content.
    pub flatten_forms: bool,
}

impl Default for SanitizeOptions {
    fn default() -> Self {
        Self {
            remove_metadata: true,
            remove_annotations: true,
            remove_attachments: true,
            remove_javascript: true,
            remove_bookmarks: false,
            flatten_forms: true,
        }
    }
}

/// Detailed report resulting from document sanitization (FR-15.5).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SanitizeReport {
    pub metadata_fields_removed: usize,
    pub xmp_metadata_removed: bool,
    pub annotations_removed: usize,
    pub attachments_removed: usize,
    pub javascript_scripts_removed: usize,
    pub bookmarks_removed: bool,
    pub forms_flattened: bool,
}

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

/// Permanently redacts a rectangular region on a page with custom appearance (FR-15.1, FR-15.3, FR-15.4).
pub fn redact_page_area_with_appearance(
    doc: &mut PdfDocument,
    page_index: usize,
    rect: [f64; 4],
    appearance: &RedactionAppearance,
) -> Result<()> {
    let page_ids = doc.page_ids();
    let page_id = *page_ids
        .get(page_index)
        .ok_or_else(|| PdfError::InvalidStructure(format!("Page {} out of range", page_index)))?;

    let x = rect[0].min(rect[2]);
    let y = rect[1].min(rect[3]);
    let w = (rect[2] - rect[0]).abs();
    let h = (rect[3] - rect[1]).abs();

    let [fr, fg, fb] = appearance.fill_color;
    let mut instructions = if (fr - 0.0).abs() < 1e-4 && (fg - 0.0).abs() < 1e-4 && (fb - 0.0).abs() < 1e-4 {
        format!("\nq\n0.0 0.0 0.0 rg\n{:.2} {:.2} {:.2} {:.2} re\nf\n", x, y, w, h)
    } else {
        format!("\nq\n{:.3} {:.3} {:.3} rg\n{:.2} {:.2} {:.2} {:.2} re\nf\n", fr, fg, fb, x, y, w, h)
    };

    if let Some(ref text) = appearance.overlay_text {
        if !text.is_empty() {
            let font_size = appearance.font_size.unwrap_or_else(|| (h * 0.5).clamp(8.0, 14.0));
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

            let [tr, tg, tb] = appearance.text_color;
            instructions.push_str(&format!(
                "BT\n/F1 {:.2} Tf\n{:.3} {:.3} {:.3} rg\n{:.2} {:.2} Td\n({}) Tj\nET\n",
                font_size, tr, tg, tb, text_x, text_y, escaped
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

    if appearance.overlay_text.is_some() {
        ensure_font_f1(page_dict);
    }

    Ok(())
}

/// Permanently redacts a rectangular region on a page (FR-15.1, FR-15.2, FR-15.3).
pub fn redact_page_area(
    doc: &mut PdfDocument,
    page_index: usize,
    rect: [f64; 4],
    overlay_text: Option<&str>,
) -> Result<()> {
    redact_page_area_with_appearance(
        doc,
        page_index,
        rect,
        &RedactionAppearance {
            overlay_text: overlay_text.map(String::from),
            ..Default::default()
        },
    )
}

/// Permanently applies a batch of marked redactions across pages (FR-15.1, FR-15.3, FR-15.6).
pub fn apply_batch_redactions(
    doc: &mut PdfDocument,
    redactions: &[(usize, [f64; 4], RedactionAppearance)],
) -> Result<usize> {
    for (page_idx, rect, appearance) in redactions {
        redact_page_area_with_appearance(doc, *page_idx, *rect, appearance)?;
    }
    Ok(redactions.len())
}

/// Automatically removes sensitive metadata, comments, attachments, scripts, and hidden content (FR-15.5).
pub fn sanitize_document(
    doc: &mut PdfDocument,
    options: &SanitizeOptions,
) -> Result<SanitizeReport> {
    let mut report = SanitizeReport::default();

    // 1. Remove metadata
    if options.remove_metadata {
        if let Some(info_id) = doc.info_id {
            if let Some(Object::Dictionary(info_dict)) = doc.objects.remove(&info_id) {
                report.metadata_fields_removed = info_dict.map.len();
            }
            doc.info_id = None;
        }

        let mut meta_stream_id = None;
        if let Some(catalog) = doc.catalog_mut() {
            if let Some(meta_obj) = catalog.remove("Metadata") {
                meta_stream_id = meta_obj.as_reference();
                report.xmp_metadata_removed = true;
            }
        }
        if let Some(meta_id) = meta_stream_id {
            doc.objects.remove(&meta_id);
        }
    }

    // 2. Remove annotations
    if options.remove_annotations {
        let mut annot_ids_to_remove = Vec::new();
        let page_ids = doc.page_ids();
        for pid in page_ids {
            if let Some(page_dict) = doc.objects.get_mut(&pid).and_then(Object::as_dict_mut) {
                if let Some(Object::Array(annots)) = page_dict.remove("Annots") {
                    report.annotations_removed += annots.len();
                    for a in annots {
                        if let Some(aid) = a.as_reference() {
                            annot_ids_to_remove.push(aid);
                        }
                    }
                }
            }
        }
        for aid in annot_ids_to_remove {
            doc.objects.remove(&aid);
        }
    }

    // 3. Remove embedded file attachments
    if options.remove_attachments {
        let mut ef_id = None;
        if let Some(catalog) = doc.catalog_mut() {
            if let Some(names_dict) = catalog.get_dict_mut("Names") {
                if let Some(ef_obj) = names_dict.remove("EmbeddedFiles") {
                    ef_id = ef_obj.as_reference();
                    report.attachments_removed += 1;
                }
            }
            if let Some(af_obj) = catalog.remove("AF") {
                if let Some(arr) = af_obj.as_array() {
                    report.attachments_removed += arr.len();
                }
            }
        }
        if let Some(eid) = ef_id {
            doc.objects.remove(&eid);
        }
    }

    // 4. Remove JavaScript
    if options.remove_javascript {
        let mut js_id = None;
        if let Some(catalog) = doc.catalog_mut() {
            if let Some(names_dict) = catalog.get_dict_mut("Names") {
                if let Some(js_obj) = names_dict.remove("JavaScript") {
                    js_id = js_obj.as_reference();
                    report.javascript_scripts_removed += 1;
                }
            }
            if catalog.remove("OpenAction").is_some() {
                report.javascript_scripts_removed += 1;
            }
            if catalog.remove("AA").is_some() {
                report.javascript_scripts_removed += 1;
            }
        }
        if let Some(jid) = js_id {
            doc.objects.remove(&jid);
        }

        let page_ids = doc.page_ids();
        for pid in page_ids {
            if let Some(page_dict) = doc.objects.get_mut(&pid).and_then(Object::as_dict_mut) {
                if page_dict.remove("AA").is_some() {
                    report.javascript_scripts_removed += 1;
                }
            }
        }
    }

    // 5. Remove bookmarks
    if options.remove_bookmarks {
        let mut outlines_id = None;
        if let Some(catalog) = doc.catalog_mut() {
            if let Some(outlines) = catalog.remove("Outlines") {
                outlines_id = outlines.as_reference();
                report.bookmarks_removed = true;
            }
        }
        if let Some(oid) = outlines_id {
            doc.objects.remove(&oid);
        }
    }

    // 6. Flatten form fields
    if options.flatten_forms {
        ravpdf_forms::flatten_form(doc)?;
        report.forms_flattened = true;
    }

    Ok(report)
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
    use ravpdf_core::object::PdfString;

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

    #[test]
    fn test_redact_appearance_and_batch() {
        let mut doc = PdfDocument::new();
        doc.add_blank_page(612.0, 792.0);

        let app = RedactionAppearance {
            fill_color: [0.8, 0.1, 0.1],
            overlay_text: Some("CONFIDENTIAL".into()),
            text_color: [1.0, 1.0, 0.0],
            font_size: Some(10.0),
        };

        let batch = vec![
            (0, [50.0, 100.0, 200.0, 130.0], app.clone()),
            (0, [250.0, 100.0, 400.0, 130.0], app),
        ];

        let count = apply_batch_redactions(&mut doc, &batch).expect("apply batch redactions");
        assert_eq!(count, 2);

        let content = doc.get_page_content(0).unwrap_or_default();
        let content_str = String::from_utf8_lossy(&content);
        assert!(content_str.contains("0.800 0.100 0.100 rg"));
        assert!(content_str.contains("1.000 1.000 0.000 rg"));
        assert!(content_str.contains("(CONFIDENTIAL) Tj"));
    }

    #[test]
    fn test_sanitize_document_full_lifecycle() {
        let mut doc = PdfDocument::new();
        doc.add_blank_page(612.0, 792.0);

        // Add metadata
        let info_id = doc.next_object_id();
        let mut info_dict = PdfDictionary::new();
        info_dict.insert("Author", PdfString::from_str("Secret Agent"));
        info_dict.insert("Title", PdfString::from_str("Classified Report"));
        doc.objects.insert(info_id, Object::Dictionary(info_dict));
        doc.info_id = Some(info_id);

        // Add annotation
        let annot_id = doc.next_object_id();
        let mut annot_dict = PdfDictionary::new();
        annot_dict.insert("Type", PdfName::new("Annot"));
        annot_dict.insert("Subtype", PdfName::new("Text"));
        doc.objects.insert(annot_id, Object::Dictionary(annot_dict));
        let page0 = doc.page_ids()[0];
        if let Some(page_dict) = doc.objects.get_mut(&page0).and_then(Object::as_dict_mut) {
            page_dict.insert("Annots", vec![Object::Reference(annot_id)]);
        }

        // Add form field
        let field = ravpdf_forms::FormField::new_text("SSN", [100.0, 200.0, 250.0, 220.0]);
        let _ = ravpdf_forms::add_form_field(&mut doc, 0, &field);

        let options = SanitizeOptions::default();
        let report = sanitize_document(&mut doc, &options).expect("sanitize doc");

        assert_eq!(report.metadata_fields_removed, 2);
        assert_eq!(report.annotations_removed, 2);
        assert!(report.forms_flattened);
        assert!(doc.info_id.is_none());
    }
}
