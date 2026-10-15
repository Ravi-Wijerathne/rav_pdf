//! Watermarking operations conforming to ISO 32000-1 Section 14.8.
//!
//! Traceability: FR-18.2, FR-37.2 (ravpdf watermark).

use ravpdf_core::document::PdfDocument;
use ravpdf_core::error::{PdfError, Result};
use ravpdf_core::object::{Object, PdfDictionary, PdfName, PdfStream};

/// Applies a semi-transparent text watermark centered diagonally across every page.
pub fn apply_text_watermark(
    doc: &mut PdfDocument,
    text: &str,
    opacity: f32,
    angle_degrees: f64,
    font_size: f64,
) -> Result<()> {
    let page_ids = doc.page_ids();
    if page_ids.is_empty() {
        return Ok(());
    }

    let rad = angle_degrees.to_radians();
    let cos_a = rad.cos();
    let sin_a = rad.sin();

    // Create an ExtGState object for alpha transparency
    let gs_id = doc.next_object_id();
    let mut gs_dict = PdfDictionary::new();
    gs_dict.insert("Type", PdfName::new("ExtGState"));
    gs_dict.insert("ca", Object::Real(opacity as f64));
    gs_dict.insert("CA", Object::Real(opacity as f64));
    doc.objects.insert(gs_id, Object::Dictionary(gs_dict));

    for (page_idx, &page_id) in page_ids.iter().enumerate() {
        let page = match doc.get_page(page_idx) {
            Some(p) => p,
            None => continue,
        };

        let width = (page.crop_box[2] - page.crop_box[0]).abs();
        let height = (page.crop_box[3] - page.crop_box[1]).abs();

        let center_x = width * 0.5;
        let center_y = height * 0.5;

        // Approximate text centering offset
        let est_text_width = (text.len() as f64) * font_size * 0.55;
        let offset_x = -est_text_width * 0.5;
        let offset_y = -font_size * 0.3;

        let watermark_cmd = format!(
            "\nq\n/GS_WM gs\n0.6 0.6 0.6 rg\n{:.4} {:.4} {:.4} {:.4} {:.2} {:.2} cm\nBT\n/F_WM {:.2} Tf\n{:.2} {:.2} Td\n({}) Tj\nET\nQ\n",
            cos_a, sin_a, -sin_a, cos_a, center_x, center_y, font_size, offset_x, offset_y, text
        );

        let stream_id = doc.next_object_id();
        let mut stream =
            PdfStream::with_plain_data(PdfDictionary::new(), watermark_cmd.into_bytes());
        let _ = stream.compress_flate();
        doc.objects.insert(stream_id, Object::Stream(stream));

        let page_obj = doc
            .objects
            .get_mut(&page_id)
            .ok_or(PdfError::ObjectNotFound(page_id.number, page_id.generation))?;
        let page_dict = page_obj
            .as_dict_mut()
            .ok_or_else(|| PdfError::InvalidStructure("Expected page dictionary".into()))?;

        // Link into page Contents
        match page_dict.get_mut("Contents") {
            Some(Object::Array(arr)) => {
                arr.push(Object::Reference(stream_id));
            }
            Some(Object::Reference(orig)) => {
                let orig_id = *orig;
                page_dict.insert(
                    "Contents",
                    vec![Object::Reference(orig_id), Object::Reference(stream_id)],
                );
            }
            _ => {
                page_dict.insert("Contents", stream_id);
            }
        }

        // Link ExtGState and Font into page Resources
        let resources = page_dict
            .map
            .entry(PdfName::new("Resources"))
            .or_insert_with(|| Object::Dictionary(PdfDictionary::new()));

        if let Object::Dictionary(res) = resources {
            let ext_g_state = res
                .map
                .entry(PdfName::new("ExtGState"))
                .or_insert_with(|| Object::Dictionary(PdfDictionary::new()));
            if let Object::Dictionary(gs) = ext_g_state {
                gs.insert("GS_WM", gs_id);
            }

            let font_dict = res
                .map
                .entry(PdfName::new("Font"))
                .or_insert_with(|| Object::Dictionary(PdfDictionary::new()));
            if let Object::Dictionary(fonts) = font_dict {
                if !fonts.contains_key("F_WM") {
                    let mut font_entry = PdfDictionary::new();
                    font_entry.insert("Type", PdfName::new("Font"));
                    font_entry.insert("Subtype", PdfName::new("Type1"));
                    font_entry.insert("BaseFont", PdfName::new("Helvetica-Bold"));
                    fonts.insert("F_WM", font_entry);
                }
            }
        }
    }

    Ok(())
}
