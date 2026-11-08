//! Microsoft Office Document Interchange Engine conforming to ISO/IEC 29500 (OpenXML).
//!
//! Traceability:
//! - FR-03.1: Create PDF from Office documents: Microsoft Word (.docx), Microsoft Excel (.xlsx), Microsoft PowerPoint (.pptx).
//! - FR-03.2: Export PDF to editable formats: Microsoft Word (.docx), Microsoft Excel (.xlsx), Microsoft PowerPoint (.pptx).

use anyhow::{anyhow, Context, Result};
use quick_xml::events::Event;
use quick_xml::reader::Reader;
use ravpdf_core::document::PdfDocument;
use ravpdf_core::object::{Object, PdfDictionary, PdfName, PdfStream};
use std::collections::BTreeMap;
use std::io::{Cursor, Read, Write};
use zip::write::SimpleFileOptions;
use zip::{ZipArchive, ZipWriter};

/// Configuration options for Office-to-PDF conversion (FR-03.1).
#[derive(Debug, Clone)]
pub struct OfficeImportOptions {
    /// Target page width in PDF points (default 612.0 for Letter).
    pub page_width: f64,
    /// Target page height in PDF points (default 792.0 for Letter).
    pub page_height: f64,
    /// Page margin in PDF points (default 54.0 = 0.75 in).
    pub margin: f64,
    /// Default body font size in points (default 11.0).
    pub font_size: f64,
    /// Default line spacing multiplier (default 1.25).
    pub line_spacing: f64,
}

impl Default for OfficeImportOptions {
    fn default() -> Self {
        Self {
            page_width: 612.0,
            page_height: 792.0,
            margin: 54.0,
            font_size: 11.0,
            line_spacing: 1.25,
        }
    }
}

// ============================================================================
// 1. Microsoft Word (.docx) Import & Export (FR-03.1, FR-03.2)
// ============================================================================

/// Represents a parsed Word paragraph run with basic style annotations.
#[derive(Debug, Clone, Default)]
struct DocxRun {
    text: String,
    bold: bool,
    italic: bool,
    size_pt: Option<f64>,
}

/// Represents a parsed Word paragraph.
#[derive(Debug, Clone, Default)]
struct DocxParagraph {
    runs: Vec<DocxRun>,
    heading_level: Option<u8>, // 1 for Heading 1, 2 for Heading 2, etc.
}

/// Represents a parsed Word table cell.
#[derive(Debug, Clone, Default)]
struct DocxCell {
    paragraphs: Vec<DocxParagraph>,
}

/// Represents a parsed Word table.
#[derive(Debug, Clone, Default)]
struct DocxTable {
    rows: Vec<Vec<DocxCell>>,
}

/// High-level element in a parsed Word document.
#[derive(Debug, Clone)]
enum DocxElement {
    Paragraph(DocxParagraph),
    Table(DocxTable),
}

/// Imports a Microsoft Word (.docx) file from raw bytes into a formatted `PdfDocument` (FR-03.1).
pub fn import_docx_from_bytes(bytes: &[u8], options: &OfficeImportOptions) -> Result<PdfDocument> {
    let mut archive = ZipArchive::new(Cursor::new(bytes))
        .context("Failed to open DOCX archive: invalid ZIP file structure")?;

    let mut doc_xml = String::new();
    {
        let mut entry = archive
            .by_name("word/document.xml")
            .context("Invalid DOCX format: missing 'word/document.xml'")?;
        entry.read_to_string(&mut doc_xml)?;
    }

    let elements = parse_docx_xml(&doc_xml)?;
    render_docx_elements_to_pdf(&elements, options)
}

/// Parses OpenXML `word/document.xml` into structured document elements.
fn parse_docx_xml(xml_content: &str) -> Result<Vec<DocxElement>> {
    let mut reader = Reader::from_str(xml_content);
    reader.config_mut().trim_text(true);

    let mut elements = Vec::new();
    let mut in_table = false;
    let mut current_table = DocxTable::default();
    let mut current_row: Vec<DocxCell> = Vec::new();
    let mut current_cell = DocxCell::default();

    let mut _in_p = false;
    let mut current_p = DocxParagraph::default();
    let mut in_r = false;
    let mut current_r = DocxRun::default();
    let mut in_t = false;
    let mut current_text = String::new();

    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let name = e.name();
                match name.as_ref() {
                    b"w:tbl" => {
                        in_table = true;
                        current_table = DocxTable::default();
                    }
                    b"w:tr" => {
                        current_row = Vec::new();
                    }
                    b"w:tc" => {
                        current_cell = DocxCell::default();
                    }
                    b"w:p" => {
                        _in_p = true;
                        current_p = DocxParagraph::default();
                    }
                    b"w:pStyle" => {
                        for attr in e.attributes().flatten() {
                            if attr.key.as_ref() == b"w:val" {
                                let val = String::from_utf8_lossy(&attr.value);
                                if val.contains("Heading1") || val.contains("1") {
                                    current_p.heading_level = Some(1);
                                } else if val.contains("Heading2") || val.contains("2") {
                                    current_p.heading_level = Some(2);
                                } else if val.contains("Title") {
                                    current_p.heading_level = Some(1);
                                }
                            }
                        }
                    }
                    b"w:r" => {
                        in_r = true;
                        current_r = DocxRun::default();
                    }
                    b"w:b" => {
                        if in_r {
                            current_r.bold = true;
                        }
                    }
                    b"w:i" => {
                        if in_r {
                            current_r.italic = true;
                        }
                    }
                    b"w:sz" => {
                        for attr in e.attributes().flatten() {
                            if attr.key.as_ref() == b"w:val" {
                                if let Ok(half_pts) = std::str::from_utf8(&attr.value).unwrap_or("0").parse::<f64>() {
                                    current_r.size_pt = Some(half_pts / 2.0);
                                }
                            }
                        }
                    }
                    b"w:t" => {
                        in_t = true;
                        current_text.clear();
                    }
                    _ => {}
                }
            }
            Ok(Event::Text(ref e)) => {
                if in_t {
                    let decoded = e.unescape().unwrap_or_default();
                    current_text.push_str(&decoded);
                }
            }
            Ok(Event::End(ref e)) => {
                let name = e.name();
                match name.as_ref() {
                    b"w:t" => {
                        in_t = false;
                        if in_r {
                            current_r.text.push_str(&current_text);
                        }
                        current_text.clear();
                    }
                    b"w:r" => {
                        in_r = false;
                        if !current_r.text.is_empty() {
                            current_p.runs.push(current_r.clone());
                        }
                        current_r = DocxRun::default();
                    }
                    b"w:p" => {
                        _in_p = false;
                        if in_table {
                            current_cell.paragraphs.push(current_p.clone());
                        } else {
                            elements.push(DocxElement::Paragraph(current_p.clone()));
                        }
                        current_p = DocxParagraph::default();
                    }
                    b"w:tc" => {
                        current_row.push(current_cell.clone());
                        current_cell = DocxCell::default();
                    }
                    b"w:tr" => {
                        current_table.rows.push(current_row.clone());
                        current_row = Vec::new();
                    }
                    b"w:tbl" => {
                        in_table = false;
                        elements.push(DocxElement::Table(current_table.clone()));
                        current_table = DocxTable::default();
                    }
                    _ => {}
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(anyhow!("XML error parsing DOCX: {:?}", e)),
            _ => {}
        }
        buf.clear();
    }

    Ok(elements)
}

/// Renders parsed Word elements into PDF pages.
fn render_docx_elements_to_pdf(
    elements: &[DocxElement],
    options: &OfficeImportOptions,
) -> Result<PdfDocument> {
    let mut doc = PdfDocument::new();
    let page_w = options.page_width;
    let page_h = options.page_height;
    let margin = options.margin;
    let content_w = page_w - 2.0 * margin;

    let mut current_page_id = doc.add_blank_page(page_w, page_h);
    let mut current_stream = String::new();
    current_stream.push_str("q\n");

    let mut cursor_y = page_h - margin;

    let flush_page = |doc: &mut PdfDocument, page_id: ravpdf_core::object::ObjectId, stream_content: &mut String| {
        stream_content.push_str("Q\n");
        let stream_id = doc.next_object_id();
        let s = PdfStream::with_plain_data(PdfDictionary::new(), stream_content.as_bytes().to_vec());
        doc.objects.insert(stream_id, Object::Stream(s));

        if let Some(Object::Dictionary(ref mut page_dict)) = doc.objects.get_mut(&page_id) {
            page_dict.insert("Contents", Object::Reference(stream_id));
            attach_standard_fonts(page_dict);
        }
        stream_content.clear();
        stream_content.push_str("q\n");
    };

    for elem in elements {
        match elem {
            DocxElement::Paragraph(p) => {
                let text: String = p.runs.iter().map(|r| r.text.as_str()).collect();
                if text.trim().is_empty() {
                    cursor_y -= options.font_size * 0.75;
                    continue;
                }

                let (font_name, font_sz) = match p.heading_level {
                    Some(1) => ("/F2", 18.0),
                    Some(2) => ("/F2", 14.0),
                    _ => {
                        let has_bold = p.runs.iter().any(|r| r.bold);
                        let name = if has_bold { "/F2" } else { "/F1" };
                        let sz = p.runs.first().and_then(|r| r.size_pt).unwrap_or(options.font_size);
                        (name, sz)
                    }
                };

                let line_height = font_sz * options.line_spacing;
                let words: Vec<&str> = text.split_whitespace().collect();
                let mut line_buf = String::new();

                for w in words {
                    let test_line = if line_buf.is_empty() {
                        w.to_string()
                    } else {
                        format!("{} {}", line_buf, w)
                    };
                    let approx_width = (test_line.len() as f64) * font_sz * 0.52;

                    if approx_width > content_w && !line_buf.is_empty() {
                        if cursor_y - line_height < margin {
                            flush_page(&mut doc, current_page_id, &mut current_stream);
                            current_page_id = doc.add_blank_page(page_w, page_h);
                            cursor_y = page_h - margin;
                        }

                        current_stream.push_str(&format!(
                            "BT\n{} {:.2} Tf\n{:.2} {:.2} Td\n({}) Tj\nET\n",
                            font_name, font_sz, margin, cursor_y, escape_pdf_str(&line_buf)
                        ));
                        cursor_y -= line_height;
                        line_buf = w.to_string();
                    } else {
                        line_buf = test_line;
                    }
                }

                if !line_buf.is_empty() {
                    if cursor_y - line_height < margin {
                        flush_page(&mut doc, current_page_id, &mut current_stream);
                        current_page_id = doc.add_blank_page(page_w, page_h);
                        cursor_y = page_h - margin;
                    }

                    current_stream.push_str(&format!(
                        "BT\n{} {:.2} Tf\n{:.2} {:.2} Td\n({}) Tj\nET\n",
                        font_name, font_sz, margin, cursor_y, escape_pdf_str(&line_buf)
                    ));
                    cursor_y -= line_height + 4.0;
                }
            }
            DocxElement::Table(tbl) => {
                let num_cols = tbl.rows.iter().map(|r| r.len()).max().unwrap_or(1).max(1);
                let col_w = content_w / (num_cols as f64);
                let row_h = options.font_size * 2.0;

                for row in &tbl.rows {
                    if cursor_y - row_h < margin {
                        flush_page(&mut doc, current_page_id, &mut current_stream);
                        current_page_id = doc.add_blank_page(page_w, page_h);
                        cursor_y = page_h - margin;
                    }

                    for (c_idx, cell) in row.iter().enumerate() {
                        let cell_x = margin + (c_idx as f64) * col_w;
                        let cell_y = cursor_y - row_h;

                        // Draw cell border
                        current_stream.push_str(&format!(
                            "0.8 0.8 0.8 RG\n0.5 w\n{:.2} {:.2} {:.2} {:.2} re\nS\n0 0 0 RG\n",
                            cell_x, cell_y, col_w, row_h
                        ));

                        let cell_text: String = cell
                            .paragraphs
                            .iter()
                            .flat_map(|p| p.runs.iter().map(|r| r.text.as_str()))
                            .collect::<Vec<_>>()
                            .join(" ");

                        if !cell_text.trim().is_empty() {
                            let text_y = cell_y + (row_h - options.font_size) / 2.0;
                            current_stream.push_str(&format!(
                                "BT\n/F1 {:.2} Tf\n{:.2} {:.2} Td\n({}) Tj\nET\n",
                                options.font_size, cell_x + 4.0, text_y, escape_pdf_str(cell_text.trim())
                            ));
                        }
                    }
                    cursor_y -= row_h;
                }
                cursor_y -= 8.0;
            }
        }
    }

    flush_page(&mut doc, current_page_id, &mut current_stream);
    Ok(doc)
}

/// Exports a `PdfDocument` to Microsoft Word (.docx) OpenXML binary format (FR-03.2).
pub fn export_pdf_to_docx(doc: &PdfDocument) -> Result<Vec<u8>> {
    let mut zip_buffer = Cursor::new(Vec::new());
    let mut zip = ZipWriter::new(&mut zip_buffer);
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    // 1. [Content_Types].xml
    zip.start_file("[Content_Types].xml", options)?;
    zip.write_all(br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>
</Types>"#)?;

    // 2. _rels/.rels
    zip.start_file("_rels/.rels", options)?;
    zip.write_all(br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/>
</Relationships>"#)?;

    // 3. word/document.xml
    zip.start_file("word/document.xml", options)?;
    let mut doc_xml = String::new();
    doc_xml.push_str(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
"#);

    let page_count = doc.page_count();
    for p_idx in 0..page_count {
        let text = crate::search::extract_page_text(doc, p_idx).unwrap_or_default();
        for line in text.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            doc_xml.push_str("    <w:p>\n      <w:r>\n        <w:t>");
            doc_xml.push_str(&quick_xml::escape::escape(trimmed));
            doc_xml.push_str("</w:t>\n      </w:r>\n    </w:p>\n");
        }
        if p_idx + 1 < page_count {
            // Page break
            doc_xml.push_str("    <w:p><w:r><w:br w:type=\"page\"/></w:r></w:p>\n");
        }
    }

    doc_xml.push_str(r#"    <w:sectPr>
      <w:pgSz w:w="12240" w:h="15840"/>
      <w:pgMar w:top="1440" w:right="1440" w:bottom="1440" w:left="1440"/>
    </w:sectPr>
  </w:body>
</w:document>"#);

    zip.write_all(doc_xml.as_bytes())?;
    zip.finish()?;

    Ok(zip_buffer.into_inner())
}

// ============================================================================
// 2. Microsoft Excel (.xlsx) Import & Export (FR-03.1, FR-03.2)
// ============================================================================

/// Cell coordinate (row, col) with string value.
#[derive(Debug, Clone)]
struct SheetCell {
    col: usize,
    val: String,
}

/// Imports a Microsoft Excel (.xlsx) file into a formatted PDF document (FR-03.1).
pub fn import_xlsx_from_bytes(bytes: &[u8], options: &OfficeImportOptions) -> Result<PdfDocument> {
    let mut archive = ZipArchive::new(Cursor::new(bytes))
        .context("Failed to open XLSX archive: invalid ZIP file structure")?;

    // 1. Read shared strings if present
    let mut shared_strings = Vec::new();
    if let Ok(mut entry) = archive.by_name("xl/sharedStrings.xml") {
        let mut sst_xml = String::new();
        entry.read_to_string(&mut sst_xml)?;
        shared_strings = parse_xlsx_shared_strings(&sst_xml)?;
    }

    // 2. Read first worksheet: xl/worksheets/sheet1.xml
    let mut sheet_xml = String::new();
    {
        let mut entry = archive
            .by_name("xl/worksheets/sheet1.xml")
            .context("Invalid XLSX format: missing 'xl/worksheets/sheet1.xml'")?;
        entry.read_to_string(&mut sheet_xml)?;
    }

    let rows = parse_xlsx_worksheet(&sheet_xml, &shared_strings)?;
    render_xlsx_grid_to_pdf(&rows, options)
}

fn parse_xlsx_shared_strings(xml: &str) -> Result<Vec<String>> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut strings = Vec::new();
    let mut in_t = false;
    let mut current = String::new();
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) if e.name().as_ref() == b"t" => {
                in_t = true;
                current.clear();
            }
            Ok(Event::Text(ref e)) if in_t => {
                current.push_str(&e.unescape().unwrap_or_default());
            }
            Ok(Event::End(ref e)) if e.name().as_ref() == b"t" => {
                in_t = false;
                strings.push(current.clone());
                current.clear();
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(anyhow!("Error reading shared strings: {:?}", e)),
            _ => {}
        }
        buf.clear();
    }
    Ok(strings)
}

fn parse_xlsx_worksheet(xml: &str, shared_strings: &[String]) -> Result<BTreeMap<usize, Vec<SheetCell>>> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut rows: BTreeMap<usize, Vec<SheetCell>> = BTreeMap::new();
    let mut current_row = 0;
    let mut current_col = 0;
    let mut is_shared_string = false;
    let mut in_v = false;
    let mut cell_val = String::new();
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                match e.name().as_ref() {
                    b"row" => {
                        for attr in e.attributes().flatten() {
                            if attr.key.as_ref() == b"r" {
                                if let Ok(r_num) = std::str::from_utf8(&attr.value).unwrap_or("0").parse::<usize>() {
                                    current_row = r_num;
                                }
                            }
                        }
                    }
                    b"c" => {
                        is_shared_string = false;
                        for attr in e.attributes().flatten() {
                            if attr.key.as_ref() == b"r" {
                                let ref_str = String::from_utf8_lossy(&attr.value);
                                current_col = parse_col_letters(&ref_str);
                            } else if attr.key.as_ref() == b"t" && attr.value.as_ref() == b"s" {
                                is_shared_string = true;
                            }
                        }
                    }
                    b"v" => {
                        in_v = true;
                        cell_val.clear();
                    }
                    _ => {}
                }
            }
            Ok(Event::Text(ref e)) if in_v => {
                cell_val.push_str(&e.unescape().unwrap_or_default());
            }
            Ok(Event::End(ref e)) => {
                match e.name().as_ref() {
                    b"v" => {
                        in_v = false;
                        let final_str = if is_shared_string {
                            if let Ok(idx) = cell_val.parse::<usize>() {
                                shared_strings.get(idx).cloned().unwrap_or(cell_val.clone())
                            } else {
                                cell_val.clone()
                            }
                        } else {
                            cell_val.clone()
                        };

                        rows.entry(current_row).or_default().push(SheetCell {
                            col: current_col,
                            val: final_str,
                        });
                        cell_val.clear();
                    }
                    _ => {}
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(anyhow!("Error reading worksheet: {:?}", e)),
            _ => {}
        }
        buf.clear();
    }

    Ok(rows)
}

fn parse_col_letters(cell_ref: &str) -> usize {
    let mut col = 0;
    for ch in cell_ref.chars() {
        if ch.is_ascii_alphabetic() {
            col = col * 26 + (ch.to_ascii_uppercase() as usize - 'A' as usize + 1);
        } else {
            break;
        }
    }
    col.saturating_sub(1)
}

fn render_xlsx_grid_to_pdf(
    rows: &BTreeMap<usize, Vec<SheetCell>>,
    options: &OfficeImportOptions,
) -> Result<PdfDocument> {
    let mut doc = PdfDocument::new();
    // Excel is best rendered in landscape (792 x 612)
    let page_w = options.page_height;
    let page_h = options.page_width;
    let margin = 36.0;
    let content_w = page_w - 2.0 * margin;

    let mut current_page_id = doc.add_blank_page(page_w, page_h);
    let mut current_stream = String::new();
    current_stream.push_str("q\n");

    let max_col = rows
        .values()
        .flat_map(|cells| cells.iter().map(|c| c.col))
        .max()
        .unwrap_or(0)
        + 1;
    let num_cols = max_col.max(4).min(10); // Clamp to printable columns
    let col_w = content_w / (num_cols as f64);
    let row_h = 20.0;
    let mut cursor_y = page_h - margin;

    let flush_page = |doc: &mut PdfDocument, page_id: ravpdf_core::object::ObjectId, stream_content: &mut String| {
        stream_content.push_str("Q\n");
        let stream_id = doc.next_object_id();
        let s = PdfStream::with_plain_data(PdfDictionary::new(), stream_content.as_bytes().to_vec());
        doc.objects.insert(stream_id, Object::Stream(s));

        if let Some(Object::Dictionary(ref mut page_dict)) = doc.objects.get_mut(&page_id) {
            page_dict.insert("Contents", Object::Reference(stream_id));
            attach_standard_fonts(page_dict);
        }
        stream_content.clear();
        stream_content.push_str("q\n");
    };

    for (row_idx, cells) in rows {
        if cursor_y - row_h < margin {
            flush_page(&mut doc, current_page_id, &mut current_stream);
            current_page_id = doc.add_blank_page(page_w, page_h);
            cursor_y = page_h - margin;
        }

        let is_header = *row_idx == 1;

        // Draw cells
        for c in 0..num_cols {
            let x = margin + (c as f64) * col_w;
            let y = cursor_y - row_h;

            if is_header {
                // Header background fill
                current_stream.push_str(&format!(
                    "0.92 0.95 0.98 rg\n{:.2} {:.2} {:.2} {:.2} re\nf\n",
                    x, y, col_w, row_h
                ));
            }

            // Cell border
            current_stream.push_str(&format!(
                "0.75 0.75 0.75 RG\n0.5 w\n{:.2} {:.2} {:.2} {:.2} re\nS\n0 0 0 rg\n",
                x, y, col_w, row_h
            ));

            if let Some(cell) = cells.iter().find(|cell| cell.col == c) {
                let font_name = if is_header { "/F2" } else { "/F1" };
                current_stream.push_str(&format!(
                    "BT\n{} 9.0 Tf\n{:.2} {:.2} Td\n({}) Tj\nET\n",
                    font_name, x + 4.0, y + 6.0, escape_pdf_str(&cell.val)
                ));
            }
        }
        cursor_y -= row_h;
    }

    flush_page(&mut doc, current_page_id, &mut current_stream);
    Ok(doc)
}

/// Exports a `PdfDocument` to Microsoft Excel (.xlsx) OpenXML binary format (FR-03.2).
pub fn export_pdf_to_xlsx(doc: &PdfDocument) -> Result<Vec<u8>> {
    let mut zip_buffer = Cursor::new(Vec::new());
    let mut zip = ZipWriter::new(&mut zip_buffer);
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    // 1. [Content_Types].xml
    zip.start_file("[Content_Types].xml", options)?;
    zip.write_all(br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/>
  <Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>
</Types>"#)?;

    // 2. _rels/.rels
    zip.start_file("_rels/.rels", options)?;
    zip.write_all(br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/>
</Relationships>"#)?;

    // 3. xl/_rels/workbook.xml.rels
    zip.start_file("xl/_rels/workbook.xml.rels", options)?;
    zip.write_all(br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/>
</Relationships>"#)?;

    // 4. xl/workbook.xml
    zip.start_file("xl/workbook.xml", options)?;
    zip.write_all(br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <sheets>
    <sheet name="Sheet1" sheetId="1" r:id="rId1" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"/>
  </sheets>
</workbook>"#)?;

    // 5. xl/worksheets/sheet1.xml
    zip.start_file("xl/worksheets/sheet1.xml", options)?;
    let mut sheet_xml = String::new();
    sheet_xml.push_str(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <sheetData>
"#);

    let page_count = doc.page_count();
    let mut row_counter = 1;

    for p_idx in 0..page_count {
        let text = crate::search::extract_page_text(doc, p_idx).unwrap_or_default();
        for line in text.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            // Split line columns by multiple whitespace or tabs
            let cols: Vec<&str> = trimmed.split("   ").map(str::trim).filter(|s| !s.is_empty()).collect();
            let cols = if cols.is_empty() { vec![trimmed] } else { cols };

            sheet_xml.push_str(&format!("    <row r=\"{}\">\n", row_counter));
            for (c_idx, col_val) in cols.iter().enumerate() {
                let col_letter = (b'A' + (c_idx as u8 % 26)) as char;
                let cell_ref = format!("{}{}", col_letter, row_counter);
                sheet_xml.push_str(&format!(
                    "      <c r=\"{}\" t=\"inlineStr\"><is><t>{}</t></is></c>\n",
                    cell_ref, quick_xml::escape::escape(*col_val)
                ));
            }
            sheet_xml.push_str("    </row>\n");
            row_counter += 1;
        }
    }

    sheet_xml.push_str(r#"  </sheetData>
</worksheet>"#);

    zip.write_all(sheet_xml.as_bytes())?;
    zip.finish()?;

    Ok(zip_buffer.into_inner())
}

// ============================================================================
// 3. Microsoft PowerPoint (.pptx) Import & Export (FR-03.1, FR-03.2)
// ============================================================================

/// Represents a parsed PowerPoint slide.
#[derive(Debug, Clone, Default)]
struct PptxSlide {
    title: Option<String>,
    paragraphs: Vec<String>,
}

/// Imports a Microsoft PowerPoint (.pptx) file into a formatted PDF slide presentation (FR-03.1).
pub fn import_pptx_from_bytes(bytes: &[u8], _options: &OfficeImportOptions) -> Result<PdfDocument> {
    let mut archive = ZipArchive::new(Cursor::new(bytes))
        .context("Failed to open PPTX archive: invalid ZIP file structure")?;

    let mut slide_names = Vec::new();
    for i in 0..archive.len() {
        let file = archive.by_index(i)?;
        let name = file.name().to_string();
        if name.starts_with("ppt/slides/slide") && name.ends_with(".xml") {
            slide_names.push(name);
        }
    }
    // Sort slides numerically: slide1.xml, slide2.xml...
    slide_names.sort_by_key(|name| {
        name.trim_start_matches("ppt/slides/slide")
            .trim_end_matches(".xml")
            .parse::<usize>()
            .unwrap_or(0)
    });

    let mut slides = Vec::new();
    for name in &slide_names {
        let mut entry = archive.by_name(name)?;
        let mut xml = String::new();
        entry.read_to_string(&mut xml)?;
        let slide = parse_pptx_slide(&xml)?;
        slides.push(slide);
    }

    render_pptx_slides_to_pdf(&slides)
}

fn parse_pptx_slide(xml: &str) -> Result<PptxSlide> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut slide = PptxSlide::default();
    let mut _in_p = false;
    let mut in_t = false;
    let mut current_p = String::new();
    let mut is_title_shape = false;
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                match e.name().as_ref() {
                    b"p:sp" => {
                        is_title_shape = false;
                    }
                    b"p:ph" => {
                        for attr in e.attributes().flatten() {
                            if attr.key.as_ref() == b"type" && (attr.value.as_ref() == b"title" || attr.value.as_ref() == b"ctrTitle") {
                                is_title_shape = true;
                            }
                        }
                    }
                    b"a:p" => {
                        _in_p = true;
                        current_p.clear();
                    }
                    b"a:t" => {
                        in_t = true;
                    }
                    _ => {}
                }
            }
            Ok(Event::Text(ref e)) if in_t => {
                current_p.push_str(&e.unescape().unwrap_or_default());
            }
            Ok(Event::End(ref e)) => {
                match e.name().as_ref() {
                    b"a:t" => {
                        in_t = false;
                    }
                    b"a:p" => {
                        _in_p = false;
                        let text = current_p.trim().to_string();
                        if !text.is_empty() {
                            if is_title_shape && slide.title.is_none() {
                                slide.title = Some(text);
                            } else {
                                slide.paragraphs.push(text);
                            }
                        }
                        current_p.clear();
                    }
                    _ => {}
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(anyhow!("Error parsing PPTX slide: {:?}", e)),
            _ => {}
        }
        buf.clear();
    }

    Ok(slide)
}

fn render_pptx_slides_to_pdf(slides: &[PptxSlide]) -> Result<PdfDocument> {
    let mut doc = PdfDocument::new();
    // 16:9 widescreen presentation dimensions: 960 x 540 pt
    let slide_w = 960.0;
    let slide_h = 540.0;
    let margin = 50.0;

    for slide in slides {
        let page_id = doc.add_blank_page(slide_w, slide_h);
        let mut stream = String::new();
        stream.push_str("q\n");

        // Subtle gradient / border background
        stream.push_str("0.96 0.97 0.98 rg\n0 0 960 540 re\nf\n");
        stream.push_str("0.2 0.3 0.6 rg\n0 520 960 20 re\nf\n"); // Top decorative bar

        let mut cursor_y = slide_h - margin - 30.0;

        // Slide title
        if let Some(ref title) = slide.title {
            stream.push_str(&format!(
                "BT\n0.1 0.2 0.4 rg\n/F2 26.0 Tf\n{:.2} {:.2} Td\n({}) Tj\nET\n",
                margin, cursor_y, escape_pdf_str(title)
            ));
            cursor_y -= 45.0;
        }

        // Slide bullet points
        stream.push_str("0 0 0 rg\n");
        for p in &slide.paragraphs {
            stream.push_str(&format!(
                "BT\n/F1 16.0 Tf\n{:.2} {:.2} Td\n(-  {}) Tj\nET\n",
                margin + 20.0, cursor_y, escape_pdf_str(p)
            ));
            cursor_y -= 28.0;
            if cursor_y < margin {
                break;
            }
        }

        stream.push_str("Q\n");
        let stream_id = doc.next_object_id();
        let s = PdfStream::with_plain_data(PdfDictionary::new(), stream.into_bytes());
        doc.objects.insert(stream_id, Object::Stream(s));

        if let Some(Object::Dictionary(ref mut page_dict)) = doc.objects.get_mut(&page_id) {
            page_dict.insert("Contents", Object::Reference(stream_id));
            attach_standard_fonts(page_dict);
        }
    }

    Ok(doc)
}

/// Exports a `PdfDocument` to Microsoft PowerPoint (.pptx) OpenXML binary format (FR-03.2).
pub fn export_pdf_to_pptx(doc: &PdfDocument) -> Result<Vec<u8>> {
    let mut zip_buffer = Cursor::new(Vec::new());
    let mut zip = ZipWriter::new(&mut zip_buffer);
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    let page_count = doc.page_count().max(1);

    // 1. [Content_Types].xml
    zip.start_file("[Content_Types].xml", options)?;
    let mut ct = String::new();
    ct.push_str(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/ppt/presentation.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml"/>
"#);
    for i in 1..=page_count {
        ct.push_str(&format!(
            "  <Override PartName=\"/ppt/slides/slide{}.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.presentationml.slide+xml\"/>\n",
            i
        ));
    }
    ct.push_str("</Types>");
    zip.write_all(ct.as_bytes())?;

    // 2. _rels/.rels
    zip.start_file("_rels/.rels", options)?;
    zip.write_all(br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="ppt/presentation.xml"/>
</Relationships>"#)?;

    // 3. ppt/_rels/presentation.xml.rels
    zip.start_file("ppt/_rels/presentation.xml.rels", options)?;
    let mut pres_rels = String::new();
    pres_rels.push_str(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
"#);
    for i in 1..=page_count {
        pres_rels.push_str(&format!(
            "  <Relationship Id=\"rId{}\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide\" Target=\"slides/slide{}.xml\"/>\n",
            i, i
        ));
    }
    pres_rels.push_str("</Relationships>");
    zip.write_all(pres_rels.as_bytes())?;

    // 4. ppt/presentation.xml
    zip.start_file("ppt/presentation.xml", options)?;
    let mut pres = String::new();
    pres.push_str(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:presentation xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <p:sldSz cx="12192000" cy="6858000"/>
  <p:sldIdLst>
"#);
    for i in 1..=page_count {
        pres.push_str(&format!(
            "    <p:sldId id=\"{}\" r:id=\"rId{}\"/>\n",
            255 + i, i
        ));
    }
    pres.push_str("  </p:sldIdLst>\n</p:presentation>");
    zip.write_all(pres.as_bytes())?;

    // 5. Slides
    for p_idx in 0..page_count {
        zip.start_file(format!("ppt/slides/slide{}.xml", p_idx + 1), options)?;
        let text = crate::search::extract_page_text(doc, p_idx).unwrap_or_default();
        let lines: Vec<&str> = text.lines().map(str::trim).filter(|s| !s.is_empty()).collect();
        let title = lines.first().copied().unwrap_or("Slide Title");
        let body_lines = if lines.len() > 1 { &lines[1..] } else { &[] };

        let mut slide_xml = String::new();
        slide_xml.push_str(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:sld xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">
  <p:cSld>
    <p:spTree>
      <p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr>
      <p:grpSpPr/>
      <p:sp>
        <p:nvSpPr><p:cNvPr id="2" name="Title"/><p:cNvSpPr><a:spLocks noGrp="1"/></p:cNvSpPr><p:nvPr><p:ph type="title"/></p:nvPr></p:nvSpPr>
        <p:spPr/>
        <p:txBody>
          <a:bodyPr/>
          <a:p><a:r><a:t>"#);
        slide_xml.push_str(&quick_xml::escape::escape(title));
        slide_xml.push_str(r#"</a:t></a:r></a:p>
        </p:txBody>
      </p:sp>
      <p:sp>
        <p:nvSpPr><p:cNvPr id="3" name="Content"/><p:cNvSpPr><a:spLocks noGrp="1"/></p:cNvSpPr><p:nvPr><p:ph idx="1"/></p:nvPr></p:nvSpPr>
        <p:spPr/>
        <p:txBody>
          <a:bodyPr/>
"#);
        for bl in body_lines {
            slide_xml.push_str("          <a:p><a:r><a:t>");
            slide_xml.push_str(&quick_xml::escape::escape(*bl));
            slide_xml.push_str("</a:t></a:r></a:p>\n");
        }
        slide_xml.push_str(r#"        </p:txBody>
      </p:sp>
    </p:spTree>
  </p:cSld>
</p:sld>"#);
        zip.write_all(slide_xml.as_bytes())?;
    }

    zip.finish()?;
    Ok(zip_buffer.into_inner())
}

// ============================================================================
// Helper Utilities
// ============================================================================

fn attach_standard_fonts(page_dict: &mut PdfDictionary) {
    let resources = page_dict
        .map
        .entry(PdfName::new("Resources"))
        .or_insert_with(|| Object::Dictionary(PdfDictionary::new()));

    if let Object::Dictionary(ref mut res_dict) = resources {
        let fonts = res_dict
            .map
            .entry(PdfName::new("Font"))
            .or_insert_with(|| Object::Dictionary(PdfDictionary::new()));

        if let Object::Dictionary(ref mut font_map) = fonts {
            if !font_map.contains_key("F1") {
                let mut f1 = PdfDictionary::new();
                f1.insert("Type", PdfName::new("Font"));
                f1.insert("Subtype", PdfName::new("Type1"));
                f1.insert("BaseFont", PdfName::new("Helvetica"));
                font_map.insert("F1", f1);
            }
            if !font_map.contains_key("F2") {
                let mut f2 = PdfDictionary::new();
                f2.insert("Type", PdfName::new("Font"));
                f2.insert("Subtype", PdfName::new("Type1"));
                f2.insert("BaseFont", PdfName::new("Helvetica-Bold"));
                font_map.insert("F2", f2);
            }
        }
    }
}

fn escape_pdf_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '(' => out.push_str("\\("),
            ')' => out.push_str("\\)"),
            '\\' => out.push_str("\\\\"),
            '\r' => out.push_str("\\r"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("    "),
            _ => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_docx_export_and_import_roundtrip() {
        let mut original_doc = PdfDocument::new();
        let p_id = original_doc.add_blank_page(612.0, 792.0);

        let stream_content = "BT\n/F1 12.0 Tf\n72 700 Td\n(Quarterly Financial Report) Tj\nET\n";
        let s_id = original_doc.next_object_id();
        let s = PdfStream::with_plain_data(PdfDictionary::new(), stream_content.as_bytes().to_vec());
        original_doc.objects.insert(s_id, Object::Stream(s));
        if let Some(Object::Dictionary(ref mut d)) = original_doc.objects.get_mut(&p_id) {
            d.insert("Contents", Object::Reference(s_id));
            attach_standard_fonts(d);
        }

        // Export to DOCX
        let docx_bytes = export_pdf_to_docx(&original_doc).expect("export docx");
        assert!(!docx_bytes.is_empty());

        // Verify ZIP entries
        let mut zip = ZipArchive::new(Cursor::new(&docx_bytes)).expect("open zip");
        assert!(zip.by_name("word/document.xml").is_ok());

        // Import back to PDF
        let imported_doc = import_docx_from_bytes(&docx_bytes, &OfficeImportOptions::default()).expect("import docx");
        assert_eq!(imported_doc.page_count(), 1);
    }

    #[test]
    fn test_xlsx_export_and_import_roundtrip() {
        let mut original_doc = PdfDocument::new();
        let p_id = original_doc.add_blank_page(612.0, 792.0);

        let stream_content = "BT\n/F1 12.0 Tf\n72 700 Td\n(Revenue   Expense   Profit) Tj\nET\n";
        let s_id = original_doc.next_object_id();
        let s = PdfStream::with_plain_data(PdfDictionary::new(), stream_content.as_bytes().to_vec());
        original_doc.objects.insert(s_id, Object::Stream(s));
        if let Some(Object::Dictionary(ref mut d)) = original_doc.objects.get_mut(&p_id) {
            d.insert("Contents", Object::Reference(s_id));
            attach_standard_fonts(d);
        }

        // Export to XLSX
        let xlsx_bytes = export_pdf_to_xlsx(&original_doc).expect("export xlsx");
        assert!(!xlsx_bytes.is_empty());

        // Verify ZIP entries
        let mut zip = ZipArchive::new(Cursor::new(&xlsx_bytes)).expect("open zip");
        assert!(zip.by_name("xl/worksheets/sheet1.xml").is_ok());

        // Import back to PDF
        let imported_doc = import_xlsx_from_bytes(&xlsx_bytes, &OfficeImportOptions::default()).expect("import xlsx");
        assert_eq!(imported_doc.page_count(), 1);
    }

    #[test]
    fn test_pptx_export_and_import_roundtrip() {
        let mut original_doc = PdfDocument::new();
        let p_id = original_doc.add_blank_page(612.0, 792.0);

        let stream_content = "BT\n/F1 12.0 Tf\n72 700 Td\n(Presentation Title) Tj\nET\n";
        let s_id = original_doc.next_object_id();
        let s = PdfStream::with_plain_data(PdfDictionary::new(), stream_content.as_bytes().to_vec());
        original_doc.objects.insert(s_id, Object::Stream(s));
        if let Some(Object::Dictionary(ref mut d)) = original_doc.objects.get_mut(&p_id) {
            d.insert("Contents", Object::Reference(s_id));
            attach_standard_fonts(d);
        }

        // Export to PPTX
        let pptx_bytes = export_pdf_to_pptx(&original_doc).expect("export pptx");
        assert!(!pptx_bytes.is_empty());

        // Verify ZIP entries
        let mut zip = ZipArchive::new(Cursor::new(&pptx_bytes)).expect("open zip");
        assert!(zip.by_name("ppt/slides/slide1.xml").is_ok());

        // Import back to PDF
        let imported_doc = import_pptx_from_bytes(&pptx_bytes, &OfficeImportOptions::default()).expect("import pptx");
        assert_eq!(imported_doc.page_count(), 1);
    }
}

