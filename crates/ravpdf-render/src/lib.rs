//! RavPDF 2D Rendering Engine — Content stream interpretation and rasterization.
//!
//! Traceability:
//! - FR-01.1 (Document Rendering)
//! - FR-01.2.3 (Page thumbnail generation)
//! - FR-01.3.1 (Zoom levels 1% to 6400%)
//! - FR-01.3.4 (Page rotation)
//! - NFR-01.2 (Sub-second page rendering performance)

pub mod color;
pub mod interpreter;

pub use color::PdfColor;
pub use interpreter::{ContentInterpreter, GraphicsState, RenderOptions};
pub use tiny_skia::{Color, Pixmap};

use ravpdf_core::error::{PdfError, Result};
use ravpdf_core::document::PdfDocument;

/// Renders a single PDF page into a high-resolution software `tiny_skia::Pixmap`.
pub fn render_page(
    doc: &PdfDocument,
    page_index: usize,
    options: &RenderOptions,
) -> Result<Pixmap> {
    let page = doc
        .get_page(page_index)
        .ok_or_else(|| PdfError::InvalidStructure(format!("Page {} out of range", page_index)))?;

    let box_width = (page.crop_box[2] - page.crop_box[0]).abs();
    let box_height = (page.crop_box[3] - page.crop_box[1]).abs();

    let px_width = ((box_width as f32) * options.zoom).round().max(1.0) as u32;
    let px_height = ((box_height as f32) * options.zoom).round().max(1.0) as u32;

    let mut pixmap = Pixmap::new(px_width, px_height)
        .ok_or_else(|| PdfError::ParseError("Failed to allocate Pixmap buffer".into()))?;

    pixmap.fill(options.background_color);

    let content_bytes = doc.get_page_content(page_index).unwrap_or_default();
    if !content_bytes.is_empty() {
        let mut interpreter =
            ContentInterpreter::new(&mut pixmap, page.crop_box, options.zoom);
        let _ = interpreter.interpret(&content_bytes, page.resources.as_ref());
    }

    Ok(pixmap)
}

/// Renders a fast preview thumbnail for page navigation panels.
pub fn render_thumbnail(
    doc: &PdfDocument,
    page_index: usize,
    max_dimension: u32,
) -> Result<Pixmap> {
    let page = doc
        .get_page(page_index)
        .ok_or_else(|| PdfError::InvalidStructure(format!("Page {} out of range", page_index)))?;

    let box_width = (page.crop_box[2] - page.crop_box[0]).abs().max(1.0) as f32;
    let box_height = (page.crop_box[3] - page.crop_box[1]).abs().max(1.0) as f32;

    let max_box = box_width.max(box_height);
    let zoom = (max_dimension as f32) / max_box;

    let options = RenderOptions {
        zoom,
        rotation: page.rotate,
        background_color: Color::WHITE,
    };

    render_page(doc, page_index, &options)
}

/// Encodes a Pixmap to PNG byte buffer.
pub fn pixmap_to_png(pixmap: &Pixmap) -> Result<Vec<u8>> {
    pixmap
        .encode_png()
        .map_err(|e| PdfError::ParseError(format!("PNG encoding failed: {}", e)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_blank_page() {
        let mut doc = PdfDocument::new();
        doc.add_blank_page(612.0, 792.0);

        let options = RenderOptions::default();
        let pixmap = render_page(&doc, 0, &options).expect("render blank page");

        assert_eq!(pixmap.width(), 612);
        assert_eq!(pixmap.height(), 792);

        let png = pixmap_to_png(&pixmap).expect("encode png");
        assert!(png.starts_with(b"\x89PNG\r\n\x1a\n"));
    }

    #[test]
    fn test_render_vector_content() {
        let mut doc = PdfDocument::new();
        // Draw a red rectangle and blue line in PDF operators
        let content = b"1 0 0 rg\n 50 50 100 100 re\n f\n 0 0 1 RG\n 2 w\n 50 50 m\n 150 150 l\n S\n";
        doc.add_page_with_content(612.0, 792.0, content.to_vec());

        let options = RenderOptions::default();
        let pixmap = render_page(&doc, 0, &options).expect("render vector content");
        assert_eq!(pixmap.width(), 612);
    }
}
