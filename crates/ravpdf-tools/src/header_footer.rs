//! Headers and Footers implementation conforming to ISO 32000-1.
//!
//! Traceability: FR-18.1.1 - FR-18.1.5.

use chrono::Utc;
use ravpdf_core::document::PdfDocument;
use ravpdf_core::error::{PdfError, Result};
use ravpdf_core::object::{Object, PdfDictionary, PdfName, PdfStream};

/// Configuration for adding headers and footers to document pages.
#[derive(Debug, Clone)]
pub struct HeaderFooterConfig {
    pub left_header: Option<String>,
    pub center_header: Option<String>,
    pub right_header: Option<String>,
    pub left_footer: Option<String>,
    pub center_footer: Option<String>,
    pub right_footer: Option<String>,
    pub font_size: f64,
    pub color: (f32, f32, f32),
    pub top_margin: f64,
    pub bottom_margin: f64,
    pub side_margin: f64,
}

impl Default for HeaderFooterConfig {
    fn default() -> Self {
        Self {
            left_header: None,
            center_header: None,
            right_header: None,
            left_footer: None,
            center_footer: Some("Page <<Page>> of <<TotalPages>>".into()),
            right_footer: None,
            font_size: 9.0,
            color: (0.3, 0.3, 0.3), // Subdued dark gray
            top_margin: 30.0,
            bottom_margin: 25.0,
            side_margin: 36.0,
        }
    }
}

/// Applies headers and footers to all pages in the document.
pub fn apply_headers_and_footers(
    doc: &mut PdfDocument,
    config: &HeaderFooterConfig,
    title: Option<&str>,
    author: Option<&str>,
) -> Result<()> {
    let total_pages = doc.page_count();
    let date_str = Utc::now().format("%Y-%m-%d").to_string();
    let title_str = title.unwrap_or("Document");
    let author_str = author.unwrap_or("");

    for i in 0..total_pages {
        let page_num = (i + 1).to_string();
        let total_str = total_pages.to_string();

        let page = match doc.get_page(i) {
            Some(p) => p,
            None => continue,
        };

        let width = (page.crop_box[2] - page.crop_box[0]).abs();
        let height = (page.crop_box[3] - page.crop_box[1]).abs();

        let replace_tokens = |tmpl: &str| -> String {
            tmpl.replace("<<Page>>", &page_num)
                .replace("<<TotalPages>>", &total_str)
                .replace("<<Date>>", &date_str)
                .replace("<<Title>>", title_str)
                .replace("<<Author>>", author_str)
        };

        let mut commands = Vec::new();
        commands.push(format!(
            "q\nBT\n/F_HF {:.2} Tf\n{:.3} {:.3} {:.3} rg\n",
            config.font_size, config.color.0, config.color.1, config.color.2
        ));

        // Top Header
        let header_y = height - config.top_margin;
        if let Some(ref left) = config.left_header {
            let text = replace_tokens(left);
            commands.push(format!("{:.2} {:.2} Td ({}) Tj\n", config.side_margin, header_y, escape_pdf_text(&text)));
        }
        if let Some(ref center) = config.center_header {
            let text = replace_tokens(center);
            let est_w = (text.len() as f64) * config.font_size * 0.55;
            let cx = (width - est_w) * 0.5;
            commands.push(format!("{:.2} {:.2} Td ({}) Tj\n", cx, header_y, escape_pdf_text(&text)));
        }
        if let Some(ref right) = config.right_header {
            let text = replace_tokens(right);
            let est_w = (text.len() as f64) * config.font_size * 0.55;
            let rx = width - config.side_margin - est_w;
            commands.push(format!("{:.2} {:.2} Td ({}) Tj\n", rx, header_y, escape_pdf_text(&text)));
        }

        // Bottom Footer
        let footer_y = config.bottom_margin;
        if let Some(ref left) = config.left_footer {
            let text = replace_tokens(left);
            commands.push(format!("{:.2} {:.2} Td ({}) Tj\n", config.side_margin, footer_y, escape_pdf_text(&text)));
        }
        if let Some(ref center) = config.center_footer {
            let text = replace_tokens(center);
            let est_w = (text.len() as f64) * config.font_size * 0.55;
            let cx = (width - est_w) * 0.5;
            commands.push(format!("{:.2} {:.2} Td ({}) Tj\n", cx, footer_y, escape_pdf_text(&text)));
        }
        if let Some(ref right) = config.right_footer {
            let text = replace_tokens(right);
            let est_w = (text.len() as f64) * config.font_size * 0.55;
            let rx = width - config.side_margin - est_w;
            commands.push(format!("{:.2} {:.2} Td ({}) Tj\n", rx, footer_y, escape_pdf_text(&text)));
        }

        commands.push("ET\nQ\n".into());
        let stream_bytes = commands.concat().into_bytes();

        let stream_id = doc.next_object_id();
        let mut stream = PdfStream::with_plain_data(PdfDictionary::new(), stream_bytes);
        let _ = stream.compress_flate();
        doc.objects.insert(stream_id, Object::Stream(stream));

        let page_id = doc.page_ids()[i];
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

        // Ensure font in Resources
        let resources = page_dict
            .map
            .entry(PdfName::new("Resources"))
            .or_insert_with(|| Object::Dictionary(PdfDictionary::new()));
        if let Object::Dictionary(res) = resources {
            let font_dict = res
                .map
                .entry(PdfName::new("Font"))
                .or_insert_with(|| Object::Dictionary(PdfDictionary::new()));
            if let Object::Dictionary(fonts) = font_dict {
                if !fonts.contains_key("F_HF") {
                    let mut font_entry = PdfDictionary::new();
                    font_entry.insert("Type", PdfName::new("Font"));
                    font_entry.insert("Subtype", PdfName::new("Type1"));
                    font_entry.insert("BaseFont", PdfName::new("Helvetica"));
                    fonts.insert("F_HF", font_entry);
                }
            }
        }
    }

    Ok(())
}

fn escape_pdf_text(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        match c {
            '(' => out.push_str("\\("),
            ')' => out.push_str("\\)"),
            '\\' => out.push_str("\\\\"),
            '\r' => out.push_str("\\r"),
            '\n' => out.push_str("\\n"),
            _ => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_apply_headers_and_footers() {
        let mut doc = PdfDocument::new();
        doc.add_blank_page(612.0, 792.0);
        doc.add_blank_page(612.0, 792.0);

        let mut config = HeaderFooterConfig::default();
        config.center_header = Some("CONFIDENTIAL - <<Title>>".into());
        config.center_footer = Some("Page <<Page>> of <<TotalPages>>".into());

        apply_headers_and_footers(&mut doc, &config, Some("Project Plan"), None).expect("apply");

        let p1_content = doc.get_page_content(0).expect("content p1");
        let p1_str = String::from_utf8_lossy(&p1_content);
        assert!(p1_str.contains("Page 1 of 2"));
        assert!(p1_str.contains("CONFIDENTIAL - Project Plan"));
    }
}
