//! Page Content stream editing (text and image placement) conforming to ISO 32000-1.
//!
//! Traceability: FR-04.5, FR-05.1, FR-05.8.

use ravpdf_core::document::PdfDocument;
use ravpdf_core::error::{PdfError, Result};
use ravpdf_core::object::{Object, PdfDictionary, PdfName, PdfStream};

/// Appends a formatted text block at coordinates `(x, y)` on the specified page.
pub fn append_text_block(
    doc: &mut PdfDocument,
    page_index: usize,
    text: &str,
    x: f64,
    y: f64,
    font_size: f64,
    color: (f32, f32, f32),
) -> Result<()> {
    let page_ids = doc.page_ids();
    let page_id = *page_ids
        .get(page_index)
        .ok_or_else(|| PdfError::InvalidStructure(format!("Page {} out of range", page_index)))?;

    // Escape text literal for PDF
    let mut escaped = String::new();
    for c in text.chars() {
        match c {
            '(' => escaped.push_str("\\("),
            ')' => escaped.push_str("\\)"),
            '\\' => escaped.push_str("\\\\"),
            '\r' => escaped.push_str("\\r"),
            '\n' => escaped.push_str("\\n"),
            _ => escaped.push(c),
        }
    }

    let instruction = format!(
        "\nq\nBT\n/F1 {:.2} Tf\n{:.3} {:.3} {:.3} rg\n{:.2} {:.2} Td\n({}) Tj\nET\nQ\n",
        font_size, color.0, color.1, color.2, x, y, escaped
    );

    // Create a new stream object with this instruction
    let stream_id = doc.next_object_id();
    let mut stream = PdfStream::with_plain_data(PdfDictionary::new(), instruction.into_bytes());
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

    // Ensure standard font /F1 exists in Resources
    ensure_standard_font(page_dict);

    Ok(())
}

/// Appends a raw image XObject onto the page at `(x, y)` with dimensions `(width, height)`.
#[allow(clippy::too_many_arguments)]
pub fn insert_raw_image(
    doc: &mut PdfDocument,
    page_index: usize,
    img_data: Vec<u8>,
    img_width: u32,
    img_height: u32,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
) -> Result<()> {
    let page_ids = doc.page_ids();
    let page_id = *page_ids
        .get(page_index)
        .ok_or_else(|| PdfError::InvalidStructure(format!("Page {} out of range", page_index)))?;

    let img_obj_id = doc.next_object_id();
    let mut img_dict = PdfDictionary::new();
    img_dict.insert("Type", PdfName::new("XObject"));
    img_dict.insert("Subtype", PdfName::new("Image"));
    img_dict.insert("Width", img_width as i64);
    img_dict.insert("Height", img_height as i64);
    img_dict.insert("ColorSpace", PdfName::new("DeviceRGB"));
    img_dict.insert("BitsPerComponent", 8);

    let mut stream = PdfStream::with_plain_data(img_dict, img_data);
    let _ = stream.compress_flate();
    doc.objects.insert(img_obj_id, Object::Stream(stream));

    let im_name = format!("Im{}", img_obj_id.number);
    let paint_cmd = format!(
        "\nq\n{:.2} 0 0 {:.2} {:.2} {:.2} cm\n/{} Do\nQ\n",
        width, height, x, y, im_name
    );

    let cmd_stream_id = doc.next_object_id();
    let mut cmd_stream = PdfStream::with_plain_data(PdfDictionary::new(), paint_cmd.into_bytes());
    let _ = cmd_stream.compress_flate();
    doc.objects.insert(cmd_stream_id, Object::Stream(cmd_stream));

    let page_obj = doc
        .objects
        .get_mut(&page_id)
        .ok_or(PdfError::ObjectNotFound(page_id.number, page_id.generation))?;
    let page_dict = page_obj
        .as_dict_mut()
        .ok_or_else(|| PdfError::InvalidStructure("Expected page dictionary".into()))?;

    // Link into /Contents
    match page_dict.get_mut("Contents") {
        Some(Object::Array(arr)) => {
            arr.push(Object::Reference(cmd_stream_id));
        }
        Some(Object::Reference(orig_id)) => {
            let orig = *orig_id;
            page_dict.insert("Contents", vec![Object::Reference(orig), Object::Reference(cmd_stream_id)]);
        }
        _ => {
            page_dict.insert("Contents", cmd_stream_id);
        }
    }

    // Link image into /Resources /XObject
    let resources = page_dict
        .map
        .entry(PdfName::new("Resources"))
        .or_insert_with(|| Object::Dictionary(PdfDictionary::new()));

    if let Object::Dictionary(res_dict) = resources {
        let xobj_dict = res_dict
            .map
            .entry(PdfName::new("XObject"))
            .or_insert_with(|| Object::Dictionary(PdfDictionary::new()));
        if let Object::Dictionary(xobjs) = xobj_dict {
            xobjs.insert(im_name, img_obj_id);
        }
    }

    Ok(())
}

fn ensure_standard_font(page_dict: &mut PdfDictionary) {
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
