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

/// Options for generating a structured PDF document from Markdown (FR-02.5, FR-02.6, FR-03.1.6).
#[derive(Debug, Clone)]
pub struct MarkdownPdfOptions {
    /// Target page size (default: A4).
    pub page_size: (f64, f64),
    /// Page margin in points (default: 54.0 pt / 0.75 in).
    pub margin: f64,
    /// Base body text font size in points (default: 11.0 pt).
    pub base_font_size: f64,
    /// Line spacing in points (default: 15.0 pt).
    pub line_spacing: f64,
    /// Document header title (optional).
    pub title: Option<String>,
    /// Whether to generate page number footers ("Page X of Y").
    pub include_page_numbers: bool,
}

impl Default for MarkdownPdfOptions {
    fn default() -> Self {
        Self {
            page_size: paper_sizes::A4,
            margin: 54.0,
            base_font_size: 11.0,
            line_spacing: 15.0,
            title: None,
            include_page_numbers: true,
        }
    }
}

/// Generates a publication-quality, multi-page PDF document from structured Markdown text.
///
/// Supports headings (#, ##, ###), blockquotes (>), bullet lists (- or *), numbered lists (1.),
/// code blocks (```), horizontal rules (---), and multi-page pagination with headers and footers.
///
/// Conforms to ISO 32000-1 Clause 9 (Text). Traceability: FR-02.5, FR-02.6, FR-03.1.6.
pub fn create_pdf_from_markdown(markdown: &str, options: &MarkdownPdfOptions) -> Result<PdfDocument> {
    let mut doc = PdfDocument::new();

    let page_w = options.page_size.0;
    let page_h = options.page_size.1;
    let margin = options.margin;
    let usable_w = (page_w - 2.0 * margin).max(40.0);

    let top_margin = if options.title.is_some() { margin + 28.0 } else { margin };
    let bottom_margin = if options.include_page_numbers { margin + 24.0 } else { margin };

    let mut pages_instructions: Vec<String> = Vec::new();
    let mut current_page_cmds = String::from("q\n");
    let mut cur_y = page_h - top_margin;

    let mut in_code_block = false;
    let mut code_lines: Vec<String> = Vec::new();

    let flush_page = |pages: &mut Vec<String>, cmds: &mut String, cur_y: &mut f64| {
        cmds.push_str("Q\n");
        pages.push(cmds.clone());
        *cmds = String::from("q\n");
        *cur_y = page_h - top_margin;
    };

    let check_page_space = |pages: &mut Vec<String>, cmds: &mut String, cur_y: &mut f64, needed_space: f64| {
        if *cur_y - needed_space < bottom_margin {
            flush_page(pages, cmds, cur_y);
        }
    };

    for raw_line in markdown.lines() {
        let trimmed = raw_line.trim();

        // Check code block fence
        if trimmed.starts_with("```") {
            if in_code_block {
                let line_h = options.base_font_size * 1.25;
                let total_h = code_lines.len() as f64 * line_h + 12.0;

                check_page_space(&mut pages_instructions, &mut current_page_cmds, &mut cur_y, total_h);

                let box_y = cur_y - total_h;
                current_page_cmds.push_str(&format!(
                    "q\n0.95 0.95 0.95 rg\n{:.2} {:.2} {:.2} {:.2} re f\n0.85 0.85 0.85 RG 0.5 w\n{:.2} {:.2} {:.2} {:.2} re S\nQ\n",
                    margin, box_y, usable_w, total_h, margin, box_y, usable_w, total_h
                ));

                current_page_cmds.push_str(&format!(
                    "BT\n/F_CR {:.2} Tf\n0.15 0.15 0.20 rg\n",
                    options.base_font_size * 0.9
                ));

                let mut code_y = cur_y - 8.0 - options.base_font_size * 0.9;
                for cl in &code_lines {
                    let esc = crate::content::escape_pdf_string(cl);
                    current_page_cmds.push_str(&format!(
                        "1 0 0 1 {:.2} {:.2} Tm\n({}) Tj\n",
                        margin + 10.0, code_y, esc
                    ));
                    code_y -= line_h;
                }
                current_page_cmds.push_str("ET\n");

                cur_y -= total_h + 10.0;
                code_lines.clear();
                in_code_block = false;
            } else {
                in_code_block = true;
                code_lines.clear();
            }
            continue;
        }

        if in_code_block {
            code_lines.push(raw_line.to_string());
            continue;
        }

        if trimmed.is_empty() {
            cur_y -= options.line_spacing * 0.6;
            continue;
        }

        // Horizontal rule
        if trimmed == "---" || trimmed == "***" || trimmed == "___" {
            check_page_space(&mut pages_instructions, &mut current_page_cmds, &mut cur_y, 20.0);
            cur_y -= 8.0;
            current_page_cmds.push_str(&format!(
                "q\n0.75 w\n0.80 0.80 0.85 RG\n{:.2} {:.2} m {:.2} {:.2} l S\nQ\n",
                margin, cur_y, margin + usable_w, cur_y
            ));
            cur_y -= 12.0;
            continue;
        }

        // Headings
        if let Some(h1_text) = trimmed.strip_prefix("# ") {
            let font_sz = options.base_font_size * 1.8;
            let spacing = font_sz * 1.35;
            check_page_space(&mut pages_instructions, &mut current_page_cmds, &mut cur_y, spacing + 14.0);
            cur_y -= 10.0;
            let esc = crate::content::escape_pdf_string(h1_text.trim());
            current_page_cmds.push_str(&format!(
                "BT\n/F_HB {:.2} Tf\n0.10 0.15 0.30 rg\n1 0 0 1 {:.2} {:.2} Tm\n({}) Tj\nET\n",
                font_sz, margin, cur_y - font_sz, esc
            ));
            cur_y -= spacing + 4.0;
            continue;
        }

        if let Some(h2_text) = trimmed.strip_prefix("## ") {
            let font_sz = options.base_font_size * 1.4;
            let spacing = font_sz * 1.3;
            check_page_space(&mut pages_instructions, &mut current_page_cmds, &mut cur_y, spacing + 10.0);
            cur_y -= 8.0;
            let esc = crate::content::escape_pdf_string(h2_text.trim());
            current_page_cmds.push_str(&format!(
                "BT\n/F_HB {:.2} Tf\n0.15 0.20 0.35 rg\n1 0 0 1 {:.2} {:.2} Tm\n({}) Tj\nET\n",
                font_sz, margin, cur_y - font_sz, esc
            ));
            cur_y -= spacing + 2.0;
            continue;
        }

        if let Some(h3_text) = trimmed.strip_prefix("### ") {
            let font_sz = options.base_font_size * 1.15;
            let spacing = font_sz * 1.25;
            check_page_space(&mut pages_instructions, &mut current_page_cmds, &mut cur_y, spacing + 6.0);
            cur_y -= 6.0;
            let esc = crate::content::escape_pdf_string(h3_text.trim());
            current_page_cmds.push_str(&format!(
                "BT\n/F_HB {:.2} Tf\n0.20 0.22 0.30 rg\n1 0 0 1 {:.2} {:.2} Tm\n({}) Tj\nET\n",
                font_sz, margin, cur_y - font_sz, esc
            ));
            cur_y -= spacing;
            continue;
        }

        // Blockquote
        if let Some(quote_text) = trimmed.strip_prefix("> ") {
            let quote_w = usable_w - 24.0;
            let wrapped = wrap_text_to_width(quote_text.trim(), quote_w, options.base_font_size * 0.52);
            let needed_h = wrapped.len() as f64 * options.line_spacing + 8.0;

            check_page_space(&mut pages_instructions, &mut current_page_cmds, &mut cur_y, needed_h);

            let bar_x = margin + 4.0;
            let bar_y_top = cur_y;
            let bar_y_bot = cur_y - needed_h;
            current_page_cmds.push_str(&format!(
                "q\n2.5 w\n0.25 0.45 0.85 RG\n{:.2} {:.2} m {:.2} {:.2} l S\nQ\n",
                bar_x, bar_y_bot, bar_x, bar_y_top
            ));

            current_page_cmds.push_str(&format!(
                "BT\n/F_HO {:.2} Tf\n0.30 0.32 0.38 rg\n",
                options.base_font_size
            ));
            let mut line_y = cur_y - options.base_font_size;
            for line in wrapped {
                let esc = crate::content::escape_pdf_string(&line);
                current_page_cmds.push_str(&format!(
                    "1 0 0 1 {:.2} {:.2} Tm\n({}) Tj\n",
                    margin + 16.0, line_y, esc
                ));
                line_y -= options.line_spacing;
            }
            current_page_cmds.push_str("ET\n");

            cur_y -= needed_h + 4.0;
            continue;
        }

        // Unordered list item (- or *)
        let is_bullet = trimmed.starts_with("- ") || trimmed.starts_with("* ");
        if is_bullet {
            let item_text = trimmed[2..].trim();
            let text_w = usable_w - 18.0;
            let wrapped = wrap_text_to_width(item_text, text_w, options.base_font_size * 0.52);
            let needed_h = wrapped.len() as f64 * options.line_spacing;

            check_page_space(&mut pages_instructions, &mut current_page_cmds, &mut cur_y, needed_h);

            let dot_x = margin + 4.0;
            let dot_y = cur_y - options.base_font_size * 0.7;
            current_page_cmds.push_str(&format!(
                "q\n0.20 0.20 0.25 rg\n{:.2} {:.2} 3.00 3.00 re f\nQ\n",
                dot_x, dot_y
            ));

            current_page_cmds.push_str(&format!(
                "BT\n/F1 {:.2} Tf\n0.10 0.10 0.10 rg\n",
                options.base_font_size
            ));
            let mut line_y = cur_y - options.base_font_size;
            for line in wrapped {
                let esc = crate::content::escape_pdf_string(&line);
                current_page_cmds.push_str(&format!(
                    "1 0 0 1 {:.2} {:.2} Tm\n({}) Tj\n",
                    margin + 16.0, line_y, esc
                ));
                line_y -= options.line_spacing;
            }
            current_page_cmds.push_str("ET\n");

            cur_y -= needed_h + 2.0;
            continue;
        }

        // Standard Paragraph
        let wrapped = wrap_text_to_width(trimmed, usable_w, options.base_font_size * 0.52);
        let needed_h = wrapped.len() as f64 * options.line_spacing;

        check_page_space(&mut pages_instructions, &mut current_page_cmds, &mut cur_y, needed_h);

        current_page_cmds.push_str(&format!(
            "BT\n/F1 {:.2} Tf\n0.08 0.08 0.08 rg\n",
            options.base_font_size
        ));
        let mut line_y = cur_y - options.base_font_size;
        for line in wrapped {
            let esc = crate::content::escape_pdf_string(&line);
            current_page_cmds.push_str(&format!(
                "1 0 0 1 {:.2} {:.2} Tm\n({}) Tj\n",
                margin, line_y, esc
            ));
            line_y -= options.line_spacing;
        }
        current_page_cmds.push_str("ET\n");

        cur_y -= needed_h + 4.0;
    }

    current_page_cmds.push_str("Q\n");
    pages_instructions.push(current_page_cmds);

    let total_pages = pages_instructions.len();

    for (page_idx, body_cmds) in pages_instructions.into_iter().enumerate() {
        let page_id = insert_blank_page(&mut doc, page_idx, page_w, page_h)?;
        let mut full_page = String::new();

        if let Some(ref title) = options.title {
            let header_y = page_h - margin + 8.0;
            let esc_title = crate::content::escape_pdf_string(title);
            full_page.push_str(&format!(
                "q\nBT\n/F1 8.50 Tf\n0.45 0.45 0.50 rg\n1 0 0 1 {:.2} {:.2} Tm\n({}) Tj\nET\n0.5 w\n0.82 0.82 0.85 RG\n{:.2} {:.2} m {:.2} {:.2} l S\nQ\n",
                margin, header_y, esc_title, margin, header_y - 4.0, margin + usable_w, header_y - 4.0
            ));
        }

        full_page.push_str(&body_cmds);

        if options.include_page_numbers {
            let footer_y = margin - 16.0;
            let footer_text = format!("Page {} of {}", page_idx + 1, total_pages);
            let esc_footer = crate::content::escape_pdf_string(&footer_text);
            let footer_x = page_w - margin - (footer_text.len() as f64 * 4.5);
            full_page.push_str(&format!(
                "q\nBT\n/F1 8.50 Tf\n0.45 0.45 0.50 rg\n1 0 0 1 {:.2} {:.2} Tm\n({}) Tj\nET\nQ\n",
                footer_x, footer_y, esc_footer
            ));
        }

        let stream_id = doc.next_object_id();
        let mut stream = PdfStream::with_plain_data(PdfDictionary::new(), full_page.into_bytes());
        let _ = stream.compress_flate();
        doc.objects.insert(stream_id, Object::Stream(stream));

        attach_markdown_fonts_to_page(&mut doc, page_id, stream_id)?;
    }

    Ok(doc)
}

fn wrap_text_to_width(text: &str, max_w: f64, avg_char_w: f64) -> Vec<String> {
    let max_chars = ((max_w / avg_char_w).floor() as usize).max(8);
    let words: Vec<&str> = text.split_whitespace().collect();
    let mut lines = Vec::new();
    let mut cur = String::new();

    for word in words {
        if cur.is_empty() {
            cur.push_str(word);
        } else if cur.len() + 1 + word.len() <= max_chars {
            cur.push(' ');
            cur.push_str(word);
        } else {
            lines.push(cur);
            cur = word.to_string();
        }
    }
    if !cur.is_empty() {
        lines.push(cur);
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

fn attach_markdown_fonts_to_page(
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

    crate::content::ensure_font_registered(page_dict, "F1");
    crate::content::ensure_font_registered(page_dict, "F_HB");
    crate::content::ensure_font_registered(page_dict, "F_HO");
    crate::content::ensure_font_registered(page_dict, "F_CR");

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
        let img1 = vec![255_u8; 10 * 10 * 3];
        let img2 = vec![128_u8; 10 * 10 * 3];
        let images = vec![(img1, 10, 10), (img2, 10, 10)];

        let options = ImagePdfOptions::default();
        let doc = create_pdf_from_raw_images(images, &options).expect("Image PDF creation failed");
        assert_eq!(doc.page_count(), 2);
    }

    #[test]
    fn test_create_pdf_from_markdown_multipage() {
        let md = r#"# Product Architecture Overview

RavPDF is a complete desktop PDF editor engine engineered in Rust.

## Key Features

- Safe memory parsing conforming to ISO 32000-1 and ISO 32000-2
- Full digital signature certification with DocMDP and LTV
- Advanced in-place content stream editing

> "High-performance systems require verifiable type safety and zero-cost abstraction."

### Implementation Snippet

```rust
fn main() {
    println!("Hello RavPDF!");
}
```

---

Continued content on the next page with extensive explanations.
"#;

        let options = MarkdownPdfOptions {
            page_size: (400.0, 300.0), // Compact size to test multi-page distribution
            margin: 25.0,
            base_font_size: 10.0,
            line_spacing: 14.0,
            title: Some("RavPDF Architecture Spec".to_string()),
            include_page_numbers: true,
        };

        let doc = create_pdf_from_markdown(md, &options).expect("Markdown PDF generation failed");
        assert!(doc.page_count() >= 2);

        let p0_content = doc.get_page_content(0).expect("page 0 content");
        let p0_str = String::from_utf8_lossy(&p0_content);
        assert!(p0_str.contains("Product Architecture Overview"));
        assert!(p0_str.contains("RavPDF Architecture Spec")); // Header title
        assert!(p0_str.contains("Page 1 of")); // Footer page numbers
    }
}

