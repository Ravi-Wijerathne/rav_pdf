//! Page Content stream editing (text, typography, in-place reflow, and image placement) conforming to ISO 32000-1.
//!
//! Traceability:
//! - FR-04.1 (In-place text editing, character modification)
//! - FR-04.2 (Text reflow within bounding box)
//! - FR-04.3 (Font properties: size, family, color, decoration)
//! - FR-04.4 (Paragraph properties: alignment Left/Center/Right/Justify, line spacing, char spacing)
//! - FR-04.5 (Add new text blocks at arbitrary positions)
//! - FR-04.12 (Font matching and standard substitution warnings)
//! - FR-05.1, FR-05.8 (Image insertion and placement)

use ravpdf_core::document::PdfDocument;
use ravpdf_core::error::{PdfError, Result};
use ravpdf_core::lexer::{Lexer, Token};
use ravpdf_core::object::{Object, PdfDictionary, PdfName, PdfStream};

/// Represents an extracted or formatted text span with its spatial coordinates and typography.
///
/// Conforms to ISO 32000-1 Clause 9 (Text). Traceability: FR-04.1, FR-04.3.
#[derive(Debug, Clone, PartialEq)]
pub struct TextSpan {
    pub text: String,
    pub x: f64,
    pub y: f64,
    pub font_name: String,
    pub font_size: f64,
    pub color: (f32, f32, f32),
    pub char_spacing: f64,
    pub word_spacing: f64,
}

/// Text alignment within a bounded paragraph block.
///
/// Traceability: FR-04.4.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextAlignment {
    Left,
    Center,
    Right,
    Justify,
}

/// Options for inserting a multi-line formatted text block with wrapping, alignment, and decoration.
///
/// Traceability: FR-04.2, FR-04.3, FR-04.4.
#[derive(Debug, Clone)]
pub struct FormattedTextBlockOptions {
    /// Bounding rectangle `[x, y, width, height]` in PDF points.
    pub rect: [f64; 4],
    /// Font family name (e.g. "Helvetica", "Helvetica-Bold", "Times-Roman", "Courier").
    pub font_family: String,
    /// Font size in points.
    pub font_size: f64,
    /// Text alignment.
    pub alignment: TextAlignment,
    /// Line spacing / height in points (e.g. 14.0).
    pub line_spacing: f64,
    /// Extra character spacing `Tc` in points.
    pub char_spacing: f64,
    /// Text color as RGB (0.0 .. 1.0).
    pub color: (f32, f32, f32),
    /// Underline decoration.
    pub underline: bool,
    /// Strikethrough decoration.
    pub strikethrough: bool,
}

impl Default for FormattedTextBlockOptions {
    fn default() -> Self {
        Self {
            rect: [72.0, 72.0, 468.0, 200.0],
            font_family: "Helvetica".to_string(),
            font_size: 11.0,
            alignment: TextAlignment::Left,
            line_spacing: 14.0,
            char_spacing: 0.0,
            color: (0.0, 0.0, 0.0),
            underline: false,
            strikethrough: false,
        }
    }
}

/// Font substitution match information.
///
/// Traceability: FR-04.12.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FontSubstitute {
    pub requested_font: String,
    pub substitute_font: &'static str,
    pub is_exact_match: bool,
    pub note: &'static str,
}

/// Evaluates a requested font family and matches it against standard PDF 14 fonts,
/// issuing recommendations and warnings if visual substitution occurs.
///
/// Conforms to ISO 32000-1 Clause 9.6.2.2. Traceability: FR-04.12.
pub fn suggest_font_substitute(requested_font: &str) -> FontSubstitute {
    let lower = requested_font.to_lowercase();
    if lower.contains("helvetica") || lower == "arial" {
        let is_exact = lower.contains("helvetica");
        let sub = if lower.contains("bold") && (lower.contains("italic") || lower.contains("oblique")) {
            "Helvetica-BoldOblique"
        } else if lower.contains("bold") {
            "Helvetica-Bold"
        } else if lower.contains("italic") || lower.contains("oblique") {
            "Helvetica-Oblique"
        } else {
            "Helvetica"
        };
        FontSubstitute {
            requested_font: requested_font.to_string(),
            substitute_font: sub,
            is_exact_match: is_exact,
            note: if is_exact {
                "Standard PDF Helvetica font match"
            } else {
                "Substituted Arial with standard PDF Helvetica (ISO 32000-1 Clause 9.6.2.2)"
            },
        }
    } else if lower.contains("times") {
        let is_exact = lower.contains("times-roman");
        let sub = if lower.contains("bold") && (lower.contains("italic") || lower.contains("oblique")) {
            "Times-BoldItalic"
        } else if lower.contains("bold") {
            "Times-Bold"
        } else if lower.contains("italic") || lower.contains("oblique") {
            "Times-Italic"
        } else {
            "Times-Roman"
        };
        FontSubstitute {
            requested_font: requested_font.to_string(),
            substitute_font: sub,
            is_exact_match: is_exact,
            note: if is_exact {
                "Standard PDF Times-Roman font match"
            } else {
                "Substituted Times New Roman with standard PDF Times-Roman"
            },
        }
    } else if lower.contains("courier") || lower.contains("mono") || lower.contains("consolas") {
        let is_exact = lower.contains("courier");
        let sub = if lower.contains("bold") {
            "Courier-Bold"
        } else if lower.contains("italic") || lower.contains("oblique") {
            "Courier-Oblique"
        } else {
            "Courier"
        };
        FontSubstitute {
            requested_font: requested_font.to_string(),
            substitute_font: sub,
            is_exact_match: is_exact,
            note: if is_exact {
                "Standard PDF Courier font match"
            } else {
                "Substituted monospace font with standard PDF Courier"
            },
        }
    } else if lower.contains("calibri") || lower.contains("segoe") || lower.contains("tahoma") || lower.contains("roboto") || lower.contains("inter") {
        FontSubstitute {
            requested_font: requested_font.to_string(),
            substitute_font: "Helvetica",
            is_exact_match: false,
            note: "Sans-serif font visually substituted with standard PDF Helvetica",
        }
    } else if lower.contains("georgia") || lower.contains("garamond") || lower.contains("palatino") || lower.contains("cambria") {
        FontSubstitute {
            requested_font: requested_font.to_string(),
            substitute_font: "Times-Roman",
            is_exact_match: false,
            note: "Serif font visually substituted with standard PDF Times-Roman",
        }
    } else if lower.contains("symbol") {
        FontSubstitute {
            requested_font: requested_font.to_string(),
            substitute_font: "Symbol",
            is_exact_match: true,
            note: "Standard PDF Symbol font",
        }
    } else if lower.contains("zapf") || lower.contains("dingbat") {
        FontSubstitute {
            requested_font: requested_font.to_string(),
            substitute_font: "ZapfDingbats",
            is_exact_match: true,
            note: "Standard PDF ZapfDingbats font",
        }
    } else {
        FontSubstitute {
            requested_font: requested_font.to_string(),
            substitute_font: "Helvetica",
            is_exact_match: false,
            note: "Font not recognized; defaulted to standard PDF Helvetica",
        }
    }
}

/// Escapes a string literal for inclusion in a PDF content stream.
pub fn escape_pdf_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 4);
    for c in s.chars() {
        match c {
            '(' => out.push_str("\\("),
            ')' => out.push_str("\\)"),
            '\\' => out.push_str("\\\\"),
            '\r' => out.push_str("\\r"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            _ => out.push(c),
        }
    }
    out
}

/// Decodes a PDF string containing escape sequences into a Rust String.
pub fn decode_pdf_string(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'\\' && i + 1 < bytes.len() {
            i += 1;
            match bytes[i] {
                b'n' => out.push('\n'),
                b'r' => out.push('\r'),
                b't' => out.push('\t'),
                b'b' => out.push('\x08'),
                b'f' => out.push('\x0C'),
                b'(' => out.push('('),
                b')' => out.push(')'),
                b'\\' => out.push('\\'),
                b'0'..=b'7' => {
                    let mut val = (bytes[i] - b'0') as u32;
                    if i + 1 < bytes.len() && (b'0'..=b'7').contains(&bytes[i + 1]) {
                        i += 1;
                        val = val * 8 + (bytes[i] - b'0') as u32;
                        if i + 1 < bytes.len() && (b'0'..=b'7').contains(&bytes[i + 1]) {
                            i += 1;
                            val = val * 8 + (bytes[i] - b'0') as u32;
                        }
                    }
                    if let Some(ch) = char::from_u32(val) {
                        out.push(ch);
                    } else {
                        out.push(val as u8 as char);
                    }
                }
                other => out.push(other as char),
            }
        } else {
            out.push(bytes[i] as char);
        }
        i += 1;
    }
    out
}

fn token_as_f64(token: &Token) -> Option<f64> {
    match token {
        Token::Real(r) => Some(*r),
        Token::Integer(i) => Some(*i as f64),
        _ => None,
    }
}

fn token_as_text(token: &Token) -> Option<String> {
    match token {
        Token::StringLiteral(bytes) => Some(decode_pdf_string(bytes)),
        Token::HexString(bytes) => Some(decode_pdf_string(bytes)),
        _ => None,
    }
}

/// Extracts structured text spans along with their coordinates, font, size, and styling from a page's content stream.
///
/// Conforms to ISO 32000-1 Clause 9 (Text). Traceability: FR-04.1, FR-04.3.
pub fn extract_page_text_spans(doc: &PdfDocument, page_index: usize) -> Result<Vec<TextSpan>> {
    let content_bytes = doc.get_page_content(page_index).unwrap_or_default();
    if content_bytes.is_empty() {
        return Ok(Vec::new());
    }

    let mut spans = Vec::new();
    let mut lexer = Lexer::new(&content_bytes);

    let mut cur_font = "Helvetica".to_string();
    let mut cur_font_size = 12.0;
    let mut cur_color = (0.0f32, 0.0f32, 0.0f32);
    let mut cur_char_spacing = 0.0;
    let mut cur_word_spacing = 0.0;
    let mut cur_leading = 14.0;
    let mut lm_x = 0.0;
    let mut lm_y = 0.0;
    let mut tm_x = 0.0;
    let mut tm_y = 0.0;

    let mut operands: Vec<Token> = Vec::new();

    while let Ok(token) = lexer.next_token() {
        match token {
            Token::Eof => break,
            Token::Keyword(ref kw) => {
                match kw.as_str() {
                    "BT" => {
                        lm_x = 0.0;
                        lm_y = 0.0;
                        tm_x = 0.0;
                        tm_y = 0.0;
                    }
                    "ET" => {}
                    "Tf" => {
                        if operands.len() >= 2 {
                            if let Token::Name(ref name) = operands[operands.len() - 2] {
                                cur_font = name.clone();
                            }
                            if let Some(sz) = token_as_f64(&operands[operands.len() - 1]) {
                                cur_font_size = sz;
                            }
                        }
                    }
                    "rg" => {
                        if operands.len() >= 3 {
                            let r = token_as_f64(&operands[operands.len() - 3]).unwrap_or(0.0) as f32;
                            let g = token_as_f64(&operands[operands.len() - 2]).unwrap_or(0.0) as f32;
                            let b = token_as_f64(&operands[operands.len() - 1]).unwrap_or(0.0) as f32;
                            cur_color = (r, g, b);
                        }
                    }
                    "g" => {
                        if !operands.is_empty() {
                            let gray = token_as_f64(&operands[operands.len() - 1]).unwrap_or(0.0) as f32;
                            cur_color = (gray, gray, gray);
                        }
                    }
                    "k" => {
                        if operands.len() >= 4 {
                            let c = token_as_f64(&operands[operands.len() - 4]).unwrap_or(0.0) as f32;
                            let m = token_as_f64(&operands[operands.len() - 3]).unwrap_or(0.0) as f32;
                            let y = token_as_f64(&operands[operands.len() - 2]).unwrap_or(0.0) as f32;
                            let k = token_as_f64(&operands[operands.len() - 1]).unwrap_or(0.0) as f32;
                            let r = (1.0 - c) * (1.0 - k);
                            let g = (1.0 - m) * (1.0 - k);
                            let b = (1.0 - y) * (1.0 - k);
                            cur_color = (r, g, b);
                        }
                    }
                    "Tc" => {
                        if !operands.is_empty() {
                            cur_char_spacing = token_as_f64(&operands[operands.len() - 1]).unwrap_or(0.0);
                        }
                    }
                    "Tw" => {
                        if !operands.is_empty() {
                            cur_word_spacing = token_as_f64(&operands[operands.len() - 1]).unwrap_or(0.0);
                        }
                    }
                    "TL" => {
                        if !operands.is_empty() {
                            cur_leading = token_as_f64(&operands[operands.len() - 1]).unwrap_or(14.0);
                        }
                    }
                    "Td" => {
                        if operands.len() >= 2 {
                            let dx = token_as_f64(&operands[operands.len() - 2]).unwrap_or(0.0);
                            let dy = token_as_f64(&operands[operands.len() - 1]).unwrap_or(0.0);
                            lm_x += dx;
                            lm_y += dy;
                            tm_x = lm_x;
                            tm_y = lm_y;
                        }
                    }
                    "TD" => {
                        if operands.len() >= 2 {
                            let dx = token_as_f64(&operands[operands.len() - 2]).unwrap_or(0.0);
                            let dy = token_as_f64(&operands[operands.len() - 1]).unwrap_or(0.0);
                            cur_leading = -dy;
                            lm_x += dx;
                            lm_y += dy;
                            tm_x = lm_x;
                            tm_y = lm_y;
                        }
                    }
                    "Tm" => {
                        if operands.len() >= 6 {
                            let e = token_as_f64(&operands[operands.len() - 2]).unwrap_or(0.0);
                            let f = token_as_f64(&operands[operands.len() - 1]).unwrap_or(0.0);
                            lm_x = e;
                            lm_y = f;
                            tm_x = e;
                            tm_y = f;
                        }
                    }
                    "T*" => {
                        lm_y -= cur_leading;
                        tm_x = lm_x;
                        tm_y = lm_y;
                    }
                    "Tj" => {
                        if !operands.is_empty() {
                            if let Some(text) = token_as_text(&operands[operands.len() - 1]) {
                                if !text.is_empty() {
                                    spans.push(TextSpan {
                                        text: text.clone(),
                                        x: tm_x,
                                        y: tm_y,
                                        font_name: cur_font.clone(),
                                        font_size: cur_font_size,
                                        color: cur_color,
                                        char_spacing: cur_char_spacing,
                                        word_spacing: cur_word_spacing,
                                    });
                                    tm_x += text.len() as f64 * cur_font_size * 0.52 + cur_char_spacing * text.len() as f64;
                                }
                            }
                        }
                    }
                    "'" => {
                        lm_y -= cur_leading;
                        tm_x = lm_x;
                        tm_y = lm_y;
                        if !operands.is_empty() {
                            if let Some(text) = token_as_text(&operands[operands.len() - 1]) {
                                if !text.is_empty() {
                                    spans.push(TextSpan {
                                        text: text.clone(),
                                        x: tm_x,
                                        y: tm_y,
                                        font_name: cur_font.clone(),
                                        font_size: cur_font_size,
                                        color: cur_color,
                                        char_spacing: cur_char_spacing,
                                        word_spacing: cur_word_spacing,
                                    });
                                    tm_x += text.len() as f64 * cur_font_size * 0.52;
                                }
                            }
                        }
                    }
                    "\"" => {
                        if operands.len() >= 3 {
                            cur_word_spacing = token_as_f64(&operands[operands.len() - 3]).unwrap_or(0.0);
                            cur_char_spacing = token_as_f64(&operands[operands.len() - 2]).unwrap_or(0.0);
                            lm_y -= cur_leading;
                            tm_x = lm_x;
                            tm_y = lm_y;
                            if let Some(text) = token_as_text(&operands[operands.len() - 1]) {
                                if !text.is_empty() {
                                    spans.push(TextSpan {
                                        text: text.clone(),
                                        x: tm_x,
                                        y: tm_y,
                                        font_name: cur_font.clone(),
                                        font_size: cur_font_size,
                                        color: cur_color,
                                        char_spacing: cur_char_spacing,
                                        word_spacing: cur_word_spacing,
                                    });
                                    tm_x += text.len() as f64 * cur_font_size * 0.52;
                                }
                            }
                        }
                    }
                    "TJ" => {
                        let mut combined_text = String::new();
                        for op in operands.iter().rev() {
                            if let Some(t) = token_as_text(op) {
                                combined_text.insert_str(0, &t);
                            } else if matches!(op, Token::ArrayOpen) {
                                break;
                            }
                        }
                        if !combined_text.is_empty() {
                            spans.push(TextSpan {
                                text: combined_text.clone(),
                                x: tm_x,
                                y: tm_y,
                                font_name: cur_font.clone(),
                                font_size: cur_font_size,
                                color: cur_color,
                                char_spacing: cur_char_spacing,
                                word_spacing: cur_word_spacing,
                            });
                            tm_x += combined_text.len() as f64 * cur_font_size * 0.52;
                        }
                    }
                    _ => {}
                }
                operands.clear();
            }
            other => {
                operands.push(other);
            }
        }
    }

    Ok(spans)
}

/// Replaces text occurrences in-place within a page's content stream, optionally updating font, size, and color.
///
/// Returns the number of replacements made.
/// Conforms to ISO 32000-1 Clause 9. Traceability: FR-04.1, FR-04.3.
pub fn replace_text_span_in_page(
    doc: &mut PdfDocument,
    page_index: usize,
    target_text: &str,
    new_text: &str,
    new_font_name: Option<&str>,
    new_font_size: Option<f64>,
    new_color: Option<(f32, f32, f32)>,
) -> Result<usize> {
    if target_text.is_empty() {
        return Ok(0);
    }

    let page = doc
        .get_page(page_index)
        .ok_or_else(|| PdfError::InvalidStructure(format!("Page {} out of range", page_index)))?;

    let contents_ids = page.contents_ids;
    let mut total_replacements = 0;

    let escaped_new = escape_pdf_string(new_text);
    let mut font_resource_to_add: Option<String> = None;

    for content_id in contents_ids {
        if let Some(Object::Stream(ref mut stream)) = doc.objects.get_mut(&content_id) {
            let decoded = stream.decode()?;
            let (new_bytes, count, used_font) = replace_text_in_stream_bytes(
                &decoded,
                target_text,
                &escaped_new,
                new_font_name,
                new_font_size,
                new_color,
            );
            if count > 0 {
                total_replacements += count;
                stream.raw_data = new_bytes;
                let _ = stream.compress_flate();
                stream.update_length();
                if let Some(f) = used_font {
                    font_resource_to_add = Some(f);
                }
            }
        }
    }

    if let Some(font_name) = font_resource_to_add {
        let page_ids = doc.page_ids();
        if let Some(&pid) = page_ids.get(page_index) {
            if let Some(page_obj) = doc.objects.get_mut(&pid) {
                if let Some(page_dict) = page_obj.as_dict_mut() {
                    ensure_font_registered(page_dict, &font_name);
                }
            }
        }
    }

    Ok(total_replacements)
}

fn replace_text_in_stream_bytes(
    data: &[u8],
    target_text: &str,
    escaped_new: &str,
    new_font_name: Option<&str>,
    new_font_size: Option<f64>,
    new_color: Option<(f32, f32, f32)>,
) -> (Vec<u8>, usize, Option<String>) {
    let mut out = Vec::with_capacity(data.len() + 64);
    let mut count = 0;
    let mut i = 0;
    let mut used_font_name: Option<String> = None;

    while i < data.len() {
        let b = data[i];

        if b == b'%' {
            out.push(b);
            i += 1;
            while i < data.len() {
                let c = data[i];
                out.push(c);
                i += 1;
                if c == b'\r' || c == b'\n' {
                    break;
                }
            }
        } else if b == b'(' {
            let start = i;
            i += 1;
            let mut depth = 1;
            let mut escaped = false;
            while i < data.len() && depth > 0 {
                let c = data[i];
                if escaped {
                    escaped = false;
                } else if c == b'\\' {
                    escaped = true;
                } else if c == b'(' {
                    depth += 1;
                } else if c == b')' {
                    depth -= 1;
                }
                i += 1;
            }

            let end = i;
            let raw_inner = if end > start + 2 {
                &data[start + 1..end - 1]
            } else {
                &[]
            };

            let decoded_str = decode_pdf_string(raw_inner);
            if decoded_str.contains(target_text) {
                count += 1;
                let replaced_str = decoded_str.replace(target_text, escaped_new);

                if let Some(font_name) = new_font_name {
                    let sz = new_font_size.unwrap_or(12.0);
                    let font_cmd = format!("\n/{} {:.2} Tf\n", font_name, sz);
                    out.extend_from_slice(font_cmd.as_bytes());
                    used_font_name = Some(font_name.to_string());
                } else if let Some(sz) = new_font_size {
                    let font_cmd = format!("\n/F1 {:.2} Tf\n", sz);
                    out.extend_from_slice(font_cmd.as_bytes());
                }

                if let Some((r, g, b)) = new_color {
                    let color_cmd = format!("{:.3} {:.3} {:.3} rg\n", r, g, b);
                    out.extend_from_slice(color_cmd.as_bytes());
                }

                let escaped_final = escape_pdf_string(&replaced_str);
                out.push(b'(');
                out.extend_from_slice(escaped_final.as_bytes());
                out.push(b')');
            } else {
                out.extend_from_slice(&data[start..end]);
            }
        } else {
            out.push(b);
            i += 1;
        }
    }

    (out, count, used_font_name)
}

/// Inserts a multi-line formatted text block with word-wrapping, paragraph alignment (Left, Center, Right, Justify),
/// character spacing, and optional underline/strikethrough path decoration onto the specified page.
///
/// Conforms to ISO 32000-1 Clause 9 (Text) & Clause 8.4 (Path Construction). Traceability: FR-04.2, FR-04.3, FR-04.4.
pub fn insert_formatted_text_block(
    doc: &mut PdfDocument,
    page_index: usize,
    text: &str,
    options: &FormattedTextBlockOptions,
) -> Result<()> {
    let page_ids = doc.page_ids();
    let page_id = *page_ids
        .get(page_index)
        .ok_or_else(|| PdfError::InvalidStructure(format!("Page {} out of range", page_index)))?;

    let block_w = options.rect[2].max(20.0);
    let avg_char_w = options.font_size * 0.52;
    let max_chars = ((block_w / avg_char_w).floor() as usize).max(5);

    let mut wrapped_lines: Vec<(String, bool)> = Vec::new();
    for raw_para in text.lines() {
        if raw_para.trim().is_empty() {
            wrapped_lines.push((String::new(), true));
            continue;
        }

        let words: Vec<&str> = raw_para.split_whitespace().collect();
        let mut cur_line = String::new();

        for (w_idx, word) in words.iter().enumerate() {
            let is_last_word = w_idx == words.len() - 1;
            if cur_line.is_empty() {
                cur_line.push_str(word);
            } else if cur_line.len() + 1 + word.len() <= max_chars {
                cur_line.push(' ');
                cur_line.push_str(word);
            } else {
                wrapped_lines.push((cur_line, false));
                cur_line = word.to_string();
            }

            if is_last_word && !cur_line.is_empty() {
                wrapped_lines.push((cur_line.clone(), true));
            }
        }
    }

    if wrapped_lines.is_empty() {
        return Ok(());
    }

    let mut content = String::from("\nq\n");
    let font_key = match options.font_family.as_str() {
        "Helvetica-Bold" => "F_HB",
        "Helvetica-Oblique" => "F_HO",
        "Times-Roman" => "F_TR",
        "Times-Bold" => "F_TB",
        "Courier" => "F_CR",
        "Courier-Bold" => "F_CB",
        _ => "F1",
    };

    content.push_str(&format!(
        "BT\n/{} {:.2} Tf\n{:.3} {:.3} {:.3} rg\n{:.2} Tc\n",
        font_key, options.font_size, options.color.0, options.color.1, options.color.2, options.char_spacing
    ));

    let top_y = options.rect[1] + options.rect[3] - options.font_size;
    let mut line_decorations: Vec<(f64, f64, f64)> = Vec::new();

    for (line_idx, (line_text, is_para_end)) in wrapped_lines.iter().enumerate() {
        if line_text.is_empty() {
            continue;
        }
        let line_y = top_y - (line_idx as f64 * options.line_spacing);
        if line_y < options.rect[1] {
            break;
        }

        let line_len = line_text.chars().count();
        let natural_w = line_len as f64 * avg_char_w + options.char_spacing * line_len as f64;

        let (line_x, word_spacing) = match options.alignment {
            TextAlignment::Left => (options.rect[0], 0.0),
            TextAlignment::Center => {
                let off = (block_w - natural_w).max(0.0) / 2.0;
                (options.rect[0] + off, 0.0)
            }
            TextAlignment::Right => {
                let off = (block_w - natural_w).max(0.0);
                (options.rect[0] + off, 0.0)
            }
            TextAlignment::Justify => {
                if *is_para_end {
                    (options.rect[0], 0.0)
                } else {
                    let spaces = line_text.chars().filter(|c| *c == ' ').count();
                    if spaces > 0 && block_w > natural_w {
                        let extra_per_space = (block_w - natural_w) / spaces as f64;
                        (options.rect[0], extra_per_space.min(options.font_size * 0.8))
                    } else {
                        (options.rect[0], 0.0)
                    }
                }
            }
        };

        let escaped = escape_pdf_string(line_text);
        if word_spacing > 0.0 {
            content.push_str(&format!("{:.2} Tw\n", word_spacing));
        } else {
            content.push_str("0.00 Tw\n");
        }

        content.push_str(&format!(
            "1 0 0 1 {:.2} {:.2} Tm\n({}) Tj\n",
            line_x, line_y, escaped
        ));

        let effective_w = natural_w.min(block_w);
        line_decorations.push((line_x, line_y, effective_w));
    }

    content.push_str("ET\n");

    if options.underline || options.strikethrough {
        content.push_str(&format!(
            "0.75 w\n{:.3} {:.3} {:.3} RG\n",
            options.color.0, options.color.1, options.color.2
        ));

        for &(lx, ly, lw) in &line_decorations {
            if options.underline {
                let uy = ly - 2.0;
                content.push_str(&format!("{:.2} {:.2} m {:.2} {:.2} l S\n", lx, uy, lx + lw, uy));
            }
            if options.strikethrough {
                let sy = ly + options.font_size * 0.32;
                content.push_str(&format!("{:.2} {:.2} m {:.2} {:.2} l S\n", lx, sy, lx + lw, sy));
            }
        }
    }

    content.push_str("Q\n");

    let stream_id = doc.next_object_id();
    let mut stream = PdfStream::with_plain_data(PdfDictionary::new(), content.into_bytes());
    let _ = stream.compress_flate();
    doc.objects.insert(stream_id, Object::Stream(stream));

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

    ensure_font_registered(page_dict, font_key);

    Ok(())
}

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

    let escaped = escape_pdf_string(text);

    let instruction = format!(
        "\nq\nBT\n/F1 {:.2} Tf\n{:.3} {:.3} {:.3} rg\n{:.2} {:.2} Td\n({}) Tj\nET\nQ\n",
        font_size, color.0, color.1, color.2, x, y, escaped
    );

    let stream_id = doc.next_object_id();
    let mut stream = PdfStream::with_plain_data(PdfDictionary::new(), instruction.into_bytes());
    let _ = stream.compress_flate();
    doc.objects.insert(stream_id, Object::Stream(stream));

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

    ensure_font_registered(page_dict, "F1");

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

/// Reads an image from disk using the `image` crate (supporting PNG, JPEG, BMP, TIFF, GIF, WebP)
/// and inserts it onto the specified page as an XObject with dimensions (width, height) at coordinates (x, y).
///
/// Conforms to ISO 32000-1 Clause 8.9 (Images). Traceability: FR-05.1.
pub fn insert_image_file(
    doc: &mut PdfDocument,
    page_index: usize,
    image_path: &std::path::Path,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
) -> Result<()> {
    let img = image::open(image_path).map_err(|e| {
        PdfError::InvalidStructure(format!(
            "Failed to open image '{}': {}",
            image_path.display(),
            e
        ))
    })?;

    let rgb = img.to_rgb8();
    let img_w = rgb.width();
    let img_h = rgb.height();

    let display_w = if width > 0.0 { width } else { img_w as f64 };
    let display_h = if height > 0.0 { height } else { img_h as f64 };

    insert_raw_image(
        doc,
        page_index,
        rgb.into_raw(),
        img_w,
        img_h,
        x,
        y,
        display_w,
        display_h,
    )
}

/// Ensures that a specified font key is registered in the page's `/Resources /Font` dictionary.
pub fn ensure_font_registered(page_dict: &mut PdfDictionary, font_key: &str) {
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
            let base_font = match font_key {
                "F_HB" | "Helvetica-Bold" => "Helvetica-Bold",
                "F_HO" | "Helvetica-Oblique" => "Helvetica-Oblique",
                "F_TR" | "Times-Roman" => "Times-Roman",
                "F_TB" | "Times-Bold" => "Times-Bold",
                "F_CR" | "Courier" => "Courier",
                "F_CB" | "Courier-Bold" => "Courier-Bold",
                "Symbol" => "Symbol",
                "ZapfDingbats" => "ZapfDingbats",
                _ => "Helvetica",
            };

            if !fonts.contains_key(font_key) {
                let mut f_dict = PdfDictionary::new();
                f_dict.insert("Type", PdfName::new("Font"));
                f_dict.insert("Subtype", PdfName::new("Type1"));
                f_dict.insert("BaseFont", PdfName::new(base_font));
                fonts.insert(font_key.to_string(), f_dict);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_append_text_and_raw_image() {
        let mut doc = PdfDocument::new();
        doc.add_blank_page(612.0, 792.0);

        append_text_block(
            &mut doc,
            0,
            "Interactive Content Insertion",
            72.0,
            700.0,
            14.0,
            (0.1, 0.2, 0.8),
        )
        .expect("append text");

        let raw_pixels = vec![255u8; 10 * 10 * 3];
        insert_raw_image(
            &mut doc,
            0,
            raw_pixels,
            10,
            10,
            100.0,
            500.0,
            100.0,
            100.0,
        )
        .expect("insert raw image");

        let content = doc.get_page_content(0).expect("get content");
        assert!(!content.is_empty());
        assert!(String::from_utf8_lossy(&content).contains("Interactive Content Insertion"));
    }

    #[test]
    fn test_extract_and_replace_text_spans() {
        let mut doc = PdfDocument::new();
        doc.add_blank_page(612.0, 792.0);

        append_text_block(
            &mut doc,
            0,
            "Initial Document Title",
            100.0,
            700.0,
            16.0,
            (0.0, 0.0, 0.0),
        )
        .expect("append text");

        let spans = extract_page_text_spans(&doc, 0).expect("extract spans");
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].text, "Initial Document Title");
        assert_eq!(spans[0].x, 100.0);
        assert_eq!(spans[0].y, 700.0);
        assert_eq!(spans[0].font_size, 16.0);

        let replaced = replace_text_span_in_page(
            &mut doc,
            0,
            "Initial Document Title",
            "Updated Product Specification",
            Some("Helvetica-Bold"),
            Some(18.0),
            Some((0.2, 0.4, 0.8)),
        )
        .expect("replace text span");

        assert_eq!(replaced, 1);

        let updated_spans = extract_page_text_spans(&doc, 0).expect("extract updated spans");
        assert_eq!(updated_spans.len(), 1);
        assert_eq!(updated_spans[0].text, "Updated Product Specification");
    }

    #[test]
    fn test_formatted_text_block_alignment_and_reflow() {
        let mut doc = PdfDocument::new();
        doc.add_blank_page(612.0, 792.0);

        let options = FormattedTextBlockOptions {
            rect: [54.0, 400.0, 300.0, 200.0],
            font_family: "Helvetica".to_string(),
            font_size: 12.0,
            alignment: TextAlignment::Justify,
            line_spacing: 16.0,
            char_spacing: 0.5,
            color: (0.1, 0.1, 0.1),
            underline: true,
            strikethrough: false,
        };

        let sample_para = "RavPDF delivers high-performance PDF manipulation conforming strictly to ISO 32000-1 and ISO 32000-2 specifications with memory safety.";
        insert_formatted_text_block(&mut doc, 0, sample_para, &options).expect("insert formatted block");

        let content = doc.get_page_content(0).expect("get content");
        let content_str = String::from_utf8_lossy(&content);
        assert!(content_str.contains("RavPDF"));
        assert!(content_str.contains("Tc")); // char spacing
        assert!(content_str.contains("m")); // path construction for underline
        assert!(content_str.contains("l S"));
    }

    #[test]
    fn test_font_substitution_suggestions() {
        let arial_sub = suggest_font_substitute("Arial");
        assert_eq!(arial_sub.substitute_font, "Helvetica");
        assert!(!arial_sub.is_exact_match);

        let times_sub = suggest_font_substitute("Times New Roman");
        assert_eq!(times_sub.substitute_font, "Times-Roman");
        assert!(!times_sub.is_exact_match);

        let courier_sub = suggest_font_substitute("Courier");
        assert_eq!(courier_sub.substitute_font, "Courier");
        assert!(courier_sub.is_exact_match);
    }
}
