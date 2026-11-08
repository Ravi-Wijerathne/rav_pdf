//! Format Conversion and Document Export Engine conforming to ISO 32000-1 Clause 7.7.
//!
//! Traceability:
//! - FR-03.2.4 (Export PDFs to image formats: PNG, JPEG with configurable DPI)
//! - FR-03.2.5 (Export PDFs to plain text with configurable page range)
//! - FR-03.2.6 (Export PDFs to HTML format preserving structure, links, and styling)
//! - FR-03.2.9 (Export selected pages or page ranges)

use crate::search::extract_page_text;
use image::{DynamicImage, ImageFormat, RgbaImage};
use ravpdf_annot::{get_page_annotations, AnnotationType, LinkAction};
use ravpdf_core::document::PdfDocument;
use ravpdf_core::error::{PdfError, Result};
use ravpdf_core::object::Object;
use ravpdf_render::{render_page, Color, RenderOptions};
use serde::{Deserialize, Serialize};
use std::io::Cursor;

/// Supported image export formats (FR-03.2.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExportImageFormat {
    Png,
    Jpeg,
}

/// Exports a PDF document to Microsoft Word (.docx) OpenXML binary format (FR-03.2.1).
pub fn export_to_docx(doc: &PdfDocument) -> Result<Vec<u8>> {
    crate::office::export_pdf_to_docx(doc)
        .map_err(|e| PdfError::InvalidStructure(e.to_string()))
}

/// Exports a PDF document to Microsoft Excel (.xlsx) OpenXML binary format (FR-03.2.2).
pub fn export_to_xlsx(doc: &PdfDocument) -> Result<Vec<u8>> {
    crate::office::export_pdf_to_xlsx(doc)
        .map_err(|e| PdfError::InvalidStructure(e.to_string()))
}

/// Exports a PDF document to Microsoft PowerPoint (.pptx) OpenXML binary format (FR-03.2.3).
pub fn export_to_pptx(doc: &PdfDocument) -> Result<Vec<u8>> {
    crate::office::export_pdf_to_pptx(doc)
        .map_err(|e| PdfError::InvalidStructure(e.to_string()))
}

/// Exports specified pages (or all pages if None) to formatted plain text (FR-03.2.5, FR-03.2.9).
pub fn export_to_plain_text(
    doc: &PdfDocument,
    page_indices: Option<&[usize]>,
) -> Result<String> {
    let total_pages = doc.page_count();
    let indices: Vec<usize> = match page_indices {
        Some(list) => list
            .iter()
            .copied()
            .filter(|&idx| idx < total_pages)
            .collect(),
        None => (0..total_pages).collect(),
    };

    if indices.is_empty() {
        return Ok(String::new());
    }

    let mut output = String::new();
    for (seq, &page_idx) in indices.iter().enumerate() {
        if seq > 0 {
            output.push_str("\n\n");
        }
        output.push_str(&format!("--- Page {} of {} ---\n\n", page_idx + 1, total_pages));
        let page_text = extract_page_text(doc, page_idx).unwrap_or_default();
        output.push_str(page_text.trim());
    }

    Ok(output)
}

/// Exports specified pages (or all pages if None) to structured HTML5 with embedded CSS (FR-03.2.6, FR-03.2.9).
pub fn export_to_html(
    doc: &PdfDocument,
    page_indices: Option<&[usize]>,
) -> Result<String> {
    let total_pages = doc.page_count();
    let indices: Vec<usize> = match page_indices {
        Some(list) => list
            .iter()
            .copied()
            .filter(|&idx| idx < total_pages)
            .collect(),
        None => (0..total_pages).collect(),
    };

    // Metadata extraction
    let doc_title = doc
        .info_id
        .and_then(|id| doc.get_object(id))
        .and_then(Object::as_dict)
        .and_then(|d| d.get_str("Title"))
        .filter(|t| !t.trim().is_empty())
        .unwrap_or_else(|| "Exported PDF Document".to_string());

    let doc_lang = doc
        .catalog()
        .and_then(|c| c.get_str("Lang"))
        .unwrap_or_else(|| "en".to_string());

    let mut html = String::new();
    html.push_str("<!DOCTYPE html>\n");
    html.push_str(&format!("<html lang=\"{}\">\n<head>\n", escape_html(&doc_lang)));
    html.push_str("  <meta charset=\"utf-8\">\n");
    html.push_str("  <meta name=\"viewport\" content=\"width=device-width, initial-scale=1.0\">\n");
    html.push_str(&format!("  <title>{}</title>\n", escape_html(&doc_title)));
    html.push_str("  <style>\n");
    html.push_str(
        r#"    :root {
      --bg-color: #f1f5f9;
      --card-bg: #ffffff;
      --text-main: #1e293b;
      --text-muted: #64748b;
      --border-color: #e2e8f0;
      --accent: #2563eb;
      --accent-hover: #1d4ed8;
      --font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif;
    }
    @media (prefers-color-scheme: dark) {
      :root {
        --bg-color: #0f172a;
        --card-bg: #1e293b;
        --text-main: #f8fafc;
        --text-muted: #94a3b8;
        --border-color: #334155;
        --accent: #3b82f6;
        --accent-hover: #60a5fa;
      }
    }
    body {
      margin: 0;
      padding: 2rem 1rem;
      background-color: var(--bg-color);
      color: var(--text-main);
      font-family: var(--font-family);
      line-height: 1.6;
    }
    .pdf-container {
      max-width: 860px;
      margin: 0 auto;
    }
    header.doc-header {
      margin-bottom: 2rem;
      padding-bottom: 1rem;
      border-bottom: 2px solid var(--border-color);
      display: flex;
      justify-content: space-between;
      align-items: baseline;
      flex-wrap: wrap;
      gap: 1rem;
    }
    header.doc-header h1 {
      margin: 0;
      font-size: 1.75rem;
      font-weight: 700;
    }
    .doc-meta {
      font-size: 0.875rem;
      color: var(--text-muted);
    }
    .pdf-page-card {
      background: var(--card-bg);
      border: 1px solid var(--border-color);
      border-radius: 8px;
      padding: 2.5rem 3rem;
      margin-bottom: 2rem;
      box-shadow: 0 4px 6px -1px rgba(0, 0, 0, 0.05), 0 2px 4px -2px rgba(0, 0, 0, 0.05);
      position: relative;
    }
    .page-indicator {
      display: inline-block;
      font-size: 0.75rem;
      font-weight: 600;
      text-transform: uppercase;
      letter-spacing: 0.05em;
      color: var(--text-muted);
      border: 1px solid var(--border-color);
      padding: 0.2rem 0.5rem;
      border-radius: 4px;
      margin-bottom: 1.5rem;
    }
    .pdf-page-card p {
      margin-top: 0;
      margin-bottom: 1rem;
      white-space: pre-wrap;
      word-break: break-word;
    }
    .pdf-page-card h2, .pdf-page-card h3 {
      color: var(--accent);
      margin-top: 1.5rem;
      margin-bottom: 0.75rem;
    }
    a.pdf-link {
      color: var(--accent);
      text-decoration: underline;
      text-underline-offset: 2px;
      transition: color 0.15s ease;
    }
    a.pdf-link:hover {
      color: var(--accent-hover);
    }
    .links-section {
      margin-top: 1.5rem;
      padding-top: 1rem;
      border-top: 1px dashed var(--border-color);
    }
    .links-title {
      font-size: 0.85rem;
      font-weight: 600;
      color: var(--text-muted);
      margin-bottom: 0.5rem;
    }
    .links-list {
      list-style: none;
      padding-left: 0;
      margin: 0;
      font-size: 0.875rem;
    }
    .links-list li {
      margin-bottom: 0.25rem;
    }
    footer.doc-footer {
      text-align: center;
      font-size: 0.8125rem;
      color: var(--text-muted);
      margin-top: 3rem;
    }
    @media print {
      body {
        background: white;
        color: black;
        padding: 0;
      }
      .pdf-page-card {
        box-shadow: none;
        border: none;
        padding: 0;
        page-break-after: always;
      }
      header.doc-header, footer.doc-footer {
        display: none;
      }
    }
"#,
    );
    html.push_str("  </style>\n</head>\n<body>\n");
    html.push_str("  <div class=\"pdf-container\">\n");
    html.push_str("    <header class=\"doc-header\">\n");
    html.push_str(&format!(
        "      <h1>{}</h1>\n",
        escape_html(&doc_title)
    ));
    html.push_str(&format!(
        "      <div class=\"doc-meta\">Total Pages: {} | Exported with RavPDF</div>\n",
        indices.len()
    ));
    html.push_str("    </header>\n\n");

    for &page_idx in &indices {
        html.push_str(&format!("    <article class=\"pdf-page-card\" id=\"page-{}\">\n", page_idx + 1));
        html.push_str(&format!(
            "      <span class=\"page-indicator\">Page {} of {}</span>\n",
            page_idx + 1,
            total_pages
        ));

        // Text content
        let text = extract_page_text(doc, page_idx).unwrap_or_default();
        let trimmed = text.trim();
        if trimmed.is_empty() {
            html.push_str("      <p><em>(Empty Page or Vector Graphics Only)</em></p>\n");
        } else {
            // Split into paragraph blocks
            let paragraphs = trimmed.split("\n\n");
            for para in paragraphs {
                let p_trim = para.trim();
                if p_trim.is_empty() {
                    continue;
                }
                // Check if paragraph looks like a short title / heading
                if p_trim.len() < 60 && !p_trim.contains('.') && p_trim.lines().count() == 1 {
                    html.push_str(&format!("      <h2>{}</h2>\n", escape_html(p_trim)));
                } else {
                    html.push_str(&format!("      <p>{}</p>\n", escape_html(p_trim)));
                }
            }
        }

        // Link annotations on this page (FR-03.2.6, FR-20.1)
        if let Ok(annots) = get_page_annotations(doc, page_idx) {
            let mut web_links = Vec::new();
            for annot in annots {
                if let AnnotationType::Link { action: LinkAction::Uri(uri), .. } = annot.annot_type {
                    web_links.push(uri);
                }
            }

            if !web_links.is_empty() {
                html.push_str("      <div class=\"links-section\">\n");
                html.push_str("        <div class=\"links-title\">Page Links:</div>\n");
                html.push_str("        <ul class=\"links-list\">\n");
                for link in web_links {
                    html.push_str(&format!(
                        "          <li><a class=\"pdf-link\" href=\"{}\" target=\"_blank\" rel=\"noopener noreferrer\">{}</a></li>\n",
                        escape_html(&link),
                        escape_html(&link)
                    ));
                }
                html.push_str("        </ul>\n");
                html.push_str("      </div>\n");
            }
        }

        html.push_str("    </article>\n\n");
    }

    html.push_str("    <footer class=\"doc-footer\">\n");
    html.push_str("      <p>Generated by RavPDF &bull; ISO 32000-1 Conforming Engine</p>\n");
    html.push_str("    </footer>\n");
    html.push_str("  </div>\n</body>\n</html>\n");

    Ok(html)
}

/// Exports a single PDF page to raster image bytes (PNG or JPEG) at configurable DPI (FR-03.2.4).
pub fn export_page_to_image(
    doc: &PdfDocument,
    page_index: usize,
    dpi: f32,
    format: ExportImageFormat,
) -> Result<Vec<u8>> {
    // 72 DPI is 1.0 scale in PDF coordinate space
    let zoom = (dpi / 72.0).clamp(0.25, 33.33);

    let options = RenderOptions {
        zoom,
        rotation: 0,
        background_color: Color::WHITE,
        hidden_ocgs: std::collections::HashSet::new(),
    };

    let pixmap = render_page(doc, page_index, &options)?;

    // Extract RGBA bytes (premultiplied with white background is equivalent to straight RGBA)
    let mut raw_rgba = Vec::with_capacity((pixmap.width() * pixmap.height() * 4) as usize);
    for pixel in pixmap.pixels() {
        raw_rgba.push(pixel.red());
        raw_rgba.push(pixel.green());
        raw_rgba.push(pixel.blue());
        raw_rgba.push(pixel.alpha());
    }

    let rgba_img = RgbaImage::from_raw(pixmap.width(), pixmap.height(), raw_rgba)
        .ok_or_else(|| PdfError::ParseError("Failed to construct image buffer from rendered pixmap".into()))?;

    let dynamic_img = DynamicImage::ImageRgba8(rgba_img);
    let mut encoded_bytes = Vec::new();

    match format {
        ExportImageFormat::Png => {
            dynamic_img
                .write_to(&mut Cursor::new(&mut encoded_bytes), ImageFormat::Png)
                .map_err(|e| PdfError::ParseError(format!("PNG encoding error: {}", e)))?;
        }
        ExportImageFormat::Jpeg => {
            let rgb_img = dynamic_img.to_rgb8();
            DynamicImage::ImageRgb8(rgb_img)
                .write_to(&mut Cursor::new(&mut encoded_bytes), ImageFormat::Jpeg)
                .map_err(|e| PdfError::ParseError(format!("JPEG encoding error: {}", e)))?;
        }
    }

    Ok(encoded_bytes)
}

/// Helper to sanitize text for safe HTML output.
fn escape_html(input: &str) -> String {
    let mut escaped = String::with_capacity(input.len());
    for c in input.chars() {
        match c {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&#39;"),
            other => escaped.push(other),
        }
    }
    escaped
}

/// Exports specified pages (or all pages if None) to Rich Text Format (RTF 1.5) preserving text formatting (FR-03.2.7, FR-03.2.9).
pub fn export_to_rtf(
    doc: &PdfDocument,
    page_indices: Option<&[usize]>,
) -> Result<String> {
    let total_pages = doc.page_count();
    let indices: Vec<usize> = match page_indices {
        Some(list) => list.iter().copied().filter(|&i| i < total_pages).collect(),
        None => (0..total_pages).collect(),
    };

    let mut rtf = String::new();
    rtf.push_str("{\\rtf1\\ansi\\ansicpg1252\\deff0\\deflang1033\n");
    rtf.push_str("{\\fonttbl{\\f0\\fswiss\\fcharset0 Arial;}{\\f1\\fmodern\\fcharset0 Courier New;}}\n");
    rtf.push_str("{\\colortbl ;\\red0\\green0\\blue0;\\red0\\green102\\blue204;}\n");
    rtf.push_str("\\viewkind4\\uc1\\pard\\f0\\fs22\n");

    for (seq, &page_idx) in indices.iter().enumerate() {
        if seq > 0 {
            rtf.push_str("\\page\n");
        }

        let spans = ravpdf_edit::extract_page_text_spans(doc, page_idx).unwrap_or_default();
        if !spans.is_empty() {
            let mut last_y: Option<f64> = None;
            for span in spans {
                if let Some(prev_y) = last_y {
                    if (span.y - prev_y).abs() > 4.0 {
                        rtf.push_str("\\par\n");
                    } else {
                        rtf.push(' ');
                    }
                }
                last_y = Some(span.y);

                let is_bold = span.font_name.to_lowercase().contains("bold");
                let is_italic = span.font_name.to_lowercase().contains("italic") || span.font_name.to_lowercase().contains("oblique");
                let fs_half_points = ((span.font_size * 2.0).round() as i64).clamp(8, 144);

                rtf.push_str(&format!("\\fs{} ", fs_half_points));
                if is_bold {
                    rtf.push_str("\\b ");
                }
                if is_italic {
                    rtf.push_str("\\i ");
                }

                escape_rtf_text(&span.text, &mut rtf);

                if is_italic {
                    rtf.push_str("\\i0 ");
                }
                if is_bold {
                    rtf.push_str("\\b0 ");
                }
            }
            rtf.push_str("\\par\n");
        } else {
            let raw_text = extract_page_text(doc, page_idx).unwrap_or_default();
            for line in raw_text.lines() {
                escape_rtf_text(line, &mut rtf);
                rtf.push_str("\\par\n");
            }
        }
    }

    rtf.push('}');
    Ok(rtf)
}

fn escape_rtf_text(input: &str, output: &mut String) {
    for c in input.chars() {
        match c {
            '\\' => output.push_str("\\\\"),
            '{' => output.push_str("\\{"),
            '}' => output.push_str("\\}"),
            '\n' => output.push_str("\\par\n"),
            '\r' => {}
            '\t' => output.push_str("\\tab "),
            c if (c as u32) > 127 => {
                output.push_str(&format!("\\u{}?", c as i32));
            }
            c => output.push(c),
        }
    }
}

/// Converts Rich Text Format (.rtf) content into an ISO 32000-1 PDF document (FR-03.1.6).
pub fn create_pdf_from_rtf(
    rtf_content: &str,
    options: &ravpdf_edit::TextPdfOptions,
) -> Result<PdfDocument> {
    let plain = strip_rtf_to_plain_text(rtf_content);
    ravpdf_edit::create_pdf_from_text(&plain, options)
}

/// Strips RTF syntax groups and control words to extract clean plain text for PDF generation.
fn strip_rtf_to_plain_text(rtf: &str) -> String {
    let mut output = String::new();
    let mut group_depth: u32 = 0;
    let mut skip_group_depth: u32 = 0;
    let mut chars = rtf.chars().peekable();

    while let Some(c) = chars.next() {
        match c {
            '{' => {
                group_depth += 1;
                let mut peek_buf = String::new();
                let mut iter = chars.clone();
                for _ in 0..10 {
                    if let Some(ch) = iter.next() {
                        peek_buf.push(ch);
                    }
                }
                if peek_buf.starts_with("\\*")
                    || peek_buf.starts_with("\\fonttbl")
                    || peek_buf.starts_with("\\colortbl")
                    || peek_buf.starts_with("\\stylesheet")
                    || peek_buf.starts_with("\\info")
                {
                    skip_group_depth = group_depth;
                }
            }
            '}' => {
                if group_depth == skip_group_depth {
                    skip_group_depth = 0;
                }
                group_depth = group_depth.saturating_sub(1);
            }
            '\\' if skip_group_depth == 0 => {
                let mut word = String::new();
                while let Some(&next) = chars.peek() {
                    if next.is_ascii_alphabetic() || next == '-' || next.is_ascii_digit() {
                        word.push(chars.next().unwrap());
                    } else {
                        break;
                    }
                }
                if let Some(&' ') = chars.peek() {
                    chars.next();
                }

                match word.as_str() {
                    "par" | "line" => output.push('\n'),
                    "page" => output.push_str("\n\n"),
                    "tab" => output.push('\t'),
                    "" => {
                        if let Some(&escaped) = chars.peek() {
                            if escaped == '{' || escaped == '}' || escaped == '\\' {
                                output.push(chars.next().unwrap());
                            }
                        }
                    }
                    _ => {}
                }
            }
            _ => {
                if skip_group_depth == 0 {
                    output.push(c);
                }
            }
        }
    }

    output
}

/// Exports tabular text structure from PDF pages as RFC 4180 CSV spreadsheet format (FR-03.2.2, FR-11.3.2).
pub fn export_to_csv_table(
    doc: &PdfDocument,
    page_indices: Option<&[usize]>,
) -> Result<String> {
    let total_pages = doc.page_count();
    let indices: Vec<usize> = match page_indices {
        Some(list) => list.iter().copied().filter(|&i| i < total_pages).collect(),
        None => (0..total_pages).collect(),
    };

    let mut csv = String::new();
    csv.push_str("Page,Row,Column_1,Column_2,Column_3,Column_4,Raw_Text\n");

    for &page_idx in &indices {
        let spans = ravpdf_edit::extract_page_text_spans(doc, page_idx).unwrap_or_default();
        if spans.is_empty() {
            continue;
        }

        let mut rows: Vec<Vec<ravpdf_edit::TextSpan>> = Vec::new();
        for span in spans {
            if let Some(row) = rows.iter_mut().find(|r| (r[0].y - span.y).abs() <= 5.0) {
                row.push(span);
            } else {
                rows.push(vec![span]);
            }
        }

        rows.sort_by(|a, b| b[0].y.partial_cmp(&a[0].y).unwrap_or(std::cmp::Ordering::Equal));

        for (row_idx, mut row) in rows.into_iter().enumerate() {
            row.sort_by(|a, b| a.x.partial_cmp(&b.x).unwrap_or(std::cmp::Ordering::Equal));

            let col_texts: Vec<String> = row.iter().map(|s| s.text.clone()).collect();
            let raw = col_texts.join(" ");

            let col1 = col_texts.first().cloned().unwrap_or_default();
            let col2 = col_texts.get(1).cloned().unwrap_or_default();
            let col3 = col_texts.get(2).cloned().unwrap_or_default();
            let col4 = col_texts.get(3).cloned().unwrap_or_default();

            csv.push_str(&format!(
                "{},{},{},{},{},{},{}\n",
                page_idx + 1,
                row_idx + 1,
                escape_csv_field(&col1),
                escape_csv_field(&col2),
                escape_csv_field(&col3),
                escape_csv_field(&col4),
                escape_csv_field(&raw),
            ));
        }
    }

    Ok(csv)
}

fn escape_csv_field(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') || s.contains('\r') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ravpdf_edit::append_text_block;

    #[test]
    fn test_export_to_plain_text_and_html() {
        let mut doc = PdfDocument::new();
        doc.add_blank_page(612.0, 792.0);
        append_text_block(&mut doc, 0, "Hello World from RavPDF Export", 50.0, 700.0, 16.0, (0.0, 0.0, 0.0))
            .expect("append text");

        // Plain text export
        let text = export_to_plain_text(&doc, None).expect("export text");
        assert!(text.contains("--- Page 1 of 1 ---"));
        assert!(text.contains("Hello World from RavPDF Export"));

        // HTML export
        let html = export_to_html(&doc, None).expect("export html");
        assert!(html.contains("<!DOCTYPE html>"));
        assert!(html.contains("Hello World from RavPDF Export"));
        assert!(html.contains("pdf-page-card"));

        // RTF export (FR-03.2.7)
        let rtf = export_to_rtf(&doc, None).expect("export rtf");
        assert!(rtf.starts_with("{\\rtf1"));
        assert!(rtf.contains("Hello World from RavPDF Export"));

        // RTF import / create PDF (FR-03.1.6)
        let rtf_doc = create_pdf_from_rtf(&rtf, &ravpdf_edit::TextPdfOptions::default()).expect("create from rtf");
        assert!(rtf_doc.page_count() >= 1);

        // CSV table export (FR-03.2.2)
        let csv = export_to_csv_table(&doc, None).expect("export csv");
        assert!(csv.contains("Page,Row,Column_1"));
        assert!(csv.contains("Hello World from RavPDF Export"));
    }

    #[test]
    fn test_export_page_to_image_png_and_jpeg() {
        let mut doc = PdfDocument::new();
        doc.add_blank_page(200.0, 200.0);

        // Export at 72 DPI
        let png_bytes =
            export_page_to_image(&doc, 0, 72.0, ExportImageFormat::Png).expect("export png");
        assert!(!png_bytes.is_empty());
        assert!(png_bytes.starts_with(b"\x89PNG\r\n\x1a\n"));

        let jpeg_bytes =
            export_page_to_image(&doc, 0, 72.0, ExportImageFormat::Jpeg).expect("export jpeg");
        assert!(!jpeg_bytes.is_empty());
        assert_eq!(&jpeg_bytes[0..2], b"\xFF\xD8"); // JPEG magic bytes
    }
}
