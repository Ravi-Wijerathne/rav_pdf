//! PDF Creation and Generation from Images and Plain Text conforming to ISO 32000-1.
//!
//! Traceability:
//! - FR-02.1 (Configurable page sizes)
//! - FR-02.3 (Generate PDFs programmatically from scratch)
//! - FR-02.4 (Create PDFs from image files: JPEG, PNG, TIFF, BMP, GIF, WebP)
//! - FR-02.5 (Create PDFs from plain text files with configurable font, size, margins, line spacing)
//! - DR-01, DR-02

use std::path::Path;
use ravpdf_core::document::{paper_sizes, PdfDocument};
use ravpdf_core::error::{PdfError, Result};
use ravpdf_core::object::{ObjectId, Object, PdfDictionary, PdfName, PdfStream};
use crate::content::insert_raw_image;
use crate::pages::insert_blank_page;

/// Options for generating a PDF from one or more images (FR-02.4).
#[derive(Debug, Clone)]
pub struct ImagePdfOptions {
    /// Target page size `(width, height)` in points.
    /// If `None`, page dimensions are set directly to the image pixel dimensions (72 DPI).
    pub target_page_size: Option<(f64, f64)>,
    /// Margin in points when fitting to a target page size.
    pub margin: f64,
}

impl Default for ImagePdfOptions {
    fn default() -> Self {
        Self {
            target_page_size: None,
            margin: 0.0,
        }
    }
}

/// Options for generating a PDF from plain text (FR-02.5).
#[derive(Debug, Clone)]
pub struct TextPdfOptions {
    /// Page dimensions `(width, height)` in points (default: A4).
    pub page_size: (f64, f64),
    /// Page margins on all sides in points (default: 54.0 pt / 0.75 in).
    pub margin: f64,
    /// Font size in points (default: 11.0 pt).
    pub font_size: f64,
    /// Vertical line spacing in points (default: 15.0 pt).
    pub line_spacing: f64,
}

impl Default for TextPdfOptions {
    fn default() -> Self {
        Self {
            page_size: paper_sizes::A4,
            margin: 54.0,
            font_size: 11.0,
            line_spacing: 15.0,
        }
    }
}

/// Creates a new multi-page PDF document where each page contains an image from the provided paths (FR-02.4).
pub fn create_pdf_from_images<P: AsRef<Path>>(
    image_paths: &[P],
    options: &ImagePdfOptions,
) -> Result<PdfDocument> {
    if image_paths.is_empty() {
        return Err(PdfError::InvalidStructure("No image paths provided".into()));
    }

    let mut doc = PdfDocument::new();

    for (page_idx, path) in image_paths.iter().enumerate() {
        let p = path.as_ref();
        let dyn_img = image::open(p).map_err(|e| {
            PdfError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("Failed to open image '{}': {}", p.display(), e),
            ))
        })?;

        let orig_w = dyn_img.width();
        let orig_h = dyn_img.height();
        let rgb_img = dyn_img.to_rgb8();
        let raw_bytes = rgb_img.into_raw();

        let (page_w, page_h, draw_x, draw_y, draw_w, draw_h) = match options.target_page_size {
            Some((pw, ph)) => {
                let margin = options.margin.max(0.0);
                let usable_w = (pw - 2.0 * margin).max(10.0);
                let usable_h = (ph - 2.0 * margin).max(10.0);
                let scale = (usable_w / orig_w as f64).min(usable_h / orig_h as f64);
                let dw = orig_w as f64 * scale;
                let dh = orig_h as f64 * scale;
                let dx = margin + (usable_w - dw) / 2.0;
                let dy = margin + (usable_h - dh) / 2.0;
                (pw, ph, dx, dy, dw, dh)
            }
            None => (
                orig_w as f64,
                orig_h as f64,
                0.0,
                0.0,
                orig_w as f64,
                orig_h as f64,
            ),
        };

        insert_blank_page(&mut doc, page_idx, page_w, page_h)?;
        insert_raw_image(
            &mut doc,
            page_idx,
            raw_bytes,
            orig_w,
            orig_h,
            draw_x,
            draw_y,
            draw_w,
            draw_h,
        )?;
    }

    Ok(doc)
}

/// Creates a new multi-page PDF document from in-memory raw image bytes with specified dimensions (FR-02.4).
#[allow(clippy::too_many_arguments)]
pub fn create_pdf_from_raw_images(
    images: Vec<(Vec<u8>, u32, u32)>,
    options: &ImagePdfOptions,
) -> Result<PdfDocument> {
    if images.is_empty() {
        return Err(PdfError::InvalidStructure("No images provided".into()));
    }

    let mut doc = PdfDocument::new();

    for (page_idx, (raw_bytes, orig_w, orig_h)) in images.into_iter().enumerate() {
        let (page_w, page_h, draw_x, draw_y, draw_w, draw_h) = match options.target_page_size {
            Some((pw, ph)) => {
                let margin = options.margin.max(0.0);
                let usable_w = (pw - 2.0 * margin).max(10.0);
                let usable_h = (ph - 2.0 * margin).max(10.0);
                let scale = (usable_w / orig_w as f64).min(usable_h / orig_h as f64);
                let dw = orig_w as f64 * scale;
                let dh = orig_h as f64 * scale;
                let dx = margin + (usable_w - dw) / 2.0;
                let dy = margin + (usable_h - dh) / 2.0;
                (pw, ph, dx, dy, dw, dh)
            }
            None => (
                orig_w as f64,
                orig_h as f64,
                0.0,
                0.0,
                orig_w as f64,
                orig_h as f64,
            ),
        };

        insert_blank_page(&mut doc, page_idx, page_w, page_h)?;
        insert_raw_image(
            &mut doc,
            page_idx,
            raw_bytes,
            orig_w,
            orig_h,
            draw_x,
            draw_y,
            draw_w,
            draw_h,
        )?;
    }

    Ok(doc)
}

/// Creates a multi-page PDF document from plain text with word-wrapping and pagination (FR-02.5).
pub fn create_pdf_from_text(text: &str, options: &TextPdfOptions) -> Result<PdfDocument> {
    let mut doc = PdfDocument::new();

    let usable_w = (options.page_size.0 - 2.0 * options.margin).max(20.0);
    let usable_h = (options.page_size.1 - 2.0 * options.margin).max(20.0);

    // Approximate character width for Helvetica
    let avg_char_w = options.font_size * 0.52;
    let max_chars_per_line = ((usable_w / avg_char_w).floor() as usize).max(10);

    // Break input text into wrapped lines
    let mut wrapped_lines: Vec<String> = Vec::new();
    for raw_line in text.lines() {
        if raw_line.is_empty() {
            wrapped_lines.push(String::new());
            continue;
        }

        let words: Vec<&str> = raw_line.split_whitespace().collect();
        let mut cur_line = String::new();

        for word in words {
            if cur_line.is_empty() {
                cur_line.push_str(word);
            } else if cur_line.len() + 1 + word.len() <= max_chars_per_line {
                cur_line.push(' ');
                cur_line.push_str(word);
            } else {
                wrapped_lines.push(cur_line);
                cur_line = word.to_string();
            }
        }
        if !cur_line.is_empty() {
            wrapped_lines.push(cur_line);
        }
    }

    if wrapped_lines.is_empty() {
        wrapped_lines.push(String::new());
    }

    let max_lines_per_page = ((usable_h / options.line_spacing).floor() as usize).max(1);
    let page_chunks: Vec<&[String]> = wrapped_lines.chunks(max_lines_per_page).collect();

    for (page_idx, lines) in page_chunks.iter().enumerate() {
        let page_id = insert_blank_page(&mut doc, page_idx, options.page_size.0, options.page_size.1)?;

        let mut content = String::from("q\nBT\n/F1 ");
        content.push_str(&format!("{:.2} Tf\n0.0 0.0 0.0 rg\n", options.font_size));

        for (line_idx, line) in lines.iter().enumerate() {
            if line.is_empty() {
                continue;
            }
            let y = options.page_size.1 - options.margin - (line_idx + 1) as f64 * options.line_spacing;
            let x = options.margin;

            let mut escaped = String::with_capacity(line.len() + 8);
            for c in line.chars() {
                match c {
                    '(' => escaped.push_str("\\("),
                    ')' => escaped.push_str("\\)"),
                    '\\' => escaped.push_str("\\\\"),
                    '\r' => escaped.push_str("\\r"),
                    '\n' => escaped.push_str("\\n"),
                    _ => escaped.push(c),
                }
            }

            content.push_str(&format!("1 0 0 1 {:.2} {:.2} Tm\n({}) Tj\n", x, y, escaped));
        }

        content.push_str("ET\nQ\n");

        let stream_id = doc.next_object_id();
        let mut stream = PdfStream::with_plain_data(PdfDictionary::new(), content.into_bytes());
        let _ = stream.compress_flate();
        doc.objects.insert(stream_id, Object::Stream(stream));

        attach_stream_and_font_to_page(&mut doc, page_id, stream_id)?;
    }

    Ok(doc)
}

fn attach_stream_and_font_to_page(
    doc: &mut PdfDocument,
    page_id: ObjectId,
    stream_id: ObjectId,
) -> Result<()> {
    let page_obj = doc
        .objects
        .get_mut(&page_id)
        .ok_or(PdfError::ObjectNotFound(page_id.number, page_id.generation))?;

    let page_dict = page_obj
        .as_dict_mut()
        .ok_or_else(|| PdfError::InvalidStructure("Expected page dictionary".into()))?;

    // Link stream to Contents
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

    // Ensure standard font /F1 is present
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

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_pdf_from_text_multipage() {
        let sample_text = "Line 1\nLine 2\nLine 3\nLine 4\nLine 5\nLine 6\nLine 7\nLine 8\nLine 9\nLine 10";
        let options = TextPdfOptions {
            page_size: (300.0, 100.0), // Tiny page so it spans multiple pages
            margin: 20.0,
            font_size: 10.0,
            line_spacing: 15.0,
        };

        let doc = create_pdf_from_text(sample_text, &options).expect("Text PDF creation failed");
        assert!(doc.page_count() >= 2);
    }

    #[test]
    fn test_create_pdf_from_raw_images() {
        // Create 2 10x10 RGB images
        let img1 = vec![255_u8; 10 * 10 * 3];
        let img2 = vec![128_u8; 10 * 10 * 3];
        let images = vec![(img1, 10, 10), (img2, 10, 10)];

        let options = ImagePdfOptions::default();
        let doc = create_pdf_from_raw_images(images, &options).expect("Image PDF creation failed");
        assert_eq!(doc.page_count(), 2);
    }
}
