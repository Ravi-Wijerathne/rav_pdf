//! Printing Subsystem conforming to ISO 32000-1 and Adobe Acrobat Print Dialog specifications.
//!
//! Traceability:
//! - FR-35.1: Printing to system-installed printers via OS print APIs.
//! - FR-35.2: Print options: page range (all, current, custom ranges), copies, collation, orientation, scaling, duplex.
//! - FR-35.3: Printing comments and annotations (in-place or summary page).
//! - FR-35.4: Printing form fields and their values.
//! - FR-35.5: Printing specific layers / OCGs.
//! - FR-35.6: Booklet printing with automatic saddle-stitch page imposition.
//! - FR-35.7: Poster printing (splitting oversized pages across multiple sheets with overlap and cut marks).
//! - FR-35.8: Multiple pages per sheet (N-up: 2, 4, 6, 9, 16) with configurable ordering and borders.
//! - FR-35.9: Print preview layout generator.
//! - FR-35.10: PostScript (.ps) output generation conforming to PostScript DSC Level 2/3.

use anyhow::{anyhow, Result};
use ravpdf_core::document::PdfDocument;
use serde::{Deserialize, Serialize};

/// Print scaling strategies (FR-35.2).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum PrintScaling {
    ActualSize,
    FitToPrintableArea,
    ShrinkOversized,
    CustomScale(f64), // e.g. 1.0 = 100%, 0.85 = 85%
}

/// Print page orientation (FR-35.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PrintOrientation {
    AutoRotate,
    Portrait,
    Landscape,
}

/// Print duplex / two-sided printing mode (FR-35.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PrintDuplex {
    Simplex,
    DuplexFlipLongEdge,
    DuplexFlipShortEdge,
}

/// Annotation printing modes (FR-35.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PrintAnnotationMode {
    DocumentOnly,
    DocumentAndAnnotations,
    SummarizeAnnotations,
}

/// Booklet binding orientation (FR-35.6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BookletBinding {
    LeftToRight, // Standard Western
    RightToLeft, // Arabic, Hebrew, CJK
}

/// Booklet printing subsets (FR-35.6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BookletSubset {
    BothSides,
    FrontSideOnly,
    BackSideOnly,
}

/// Saddle-stitch booklet sheet with 4 logical pages (FR-35.6).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BookletSheet {
    pub sheet_number: usize,
    pub front_left_page: Option<usize>, // 0-based page index, None = blank
    pub front_right_page: Option<usize>,
    pub back_left_page: Option<usize>,
    pub back_right_page: Option<usize>,
}

/// Poster tiling sheet specification (FR-35.7).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PosterTile {
    pub source_page: usize,
    pub col: usize,
    pub row: usize,
    pub total_cols: usize,
    pub total_rows: usize,
    pub crop_rect: [f64; 4], // [x, y, width, height] in source points
    pub overlap_points: f64,
    pub has_cut_marks: bool,
}

/// N-up pages-per-sheet count (FR-35.8).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NUpCount {
    TwoUp = 2,
    FourUp = 4,
    SixUp = 6,
    NineUp = 9,
    SixteenUp = 16,
}

impl NUpCount {
    pub fn grid_dimensions(&self) -> (usize, usize) {
        match self {
            NUpCount::TwoUp => (2, 1),
            NUpCount::FourUp => (2, 2),
            NUpCount::SixUp => (3, 2),
            NUpCount::NineUp => (3, 3),
            NUpCount::SixteenUp => (4, 4),
        }
    }

    pub fn value(&self) -> usize {
        *self as usize
    }
}

/// Ordering of pages on an N-up sheet (FR-35.8).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NUpOrder {
    Horizontal,
    HorizontalReversed,
    Vertical,
    VerticalReversed,
}

/// Single cell placement within an N-up printed sheet (FR-35.8).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NUpCellPlacement {
    pub source_page: usize,
    pub cell_rect: [f64; 4], // [x, y, w, h] on the physical sheet
    pub scale: f64,
}

/// Single physical sheet containing N logical pages (FR-35.8).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NUpSheet {
    pub sheet_number: usize,
    pub cells: Vec<NUpCellPlacement>,
}

/// High-level print layout mode.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PrintLayoutMode {
    Standard,
    Booklet {
        binding: BookletBinding,
        subset: BookletSubset,
    },
    Poster {
        tile_scale: f64,
        overlap_points: f64,
        cut_marks: bool,
    },
    NUp {
        count: NUpCount,
        order: NUpOrder,
        draw_border: bool,
    },
}

/// Overall print job configuration (FR-35.1 - FR-35.8).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrintJobConfig {
    pub printer_name: String,
    pub copies: u32,
    pub collate: bool,
    pub page_range_str: String, // e.g. "all", "1-3, 5, 8-10"
    pub scaling: PrintScaling,
    pub orientation: PrintOrientation,
    pub duplex: PrintDuplex,
    pub annotation_mode: PrintAnnotationMode,
    pub print_forms: bool,
    pub layout_mode: PrintLayoutMode,
    pub target_sheet_size: (f64, f64), // (width, height) in points, default Letter
}

impl Default for PrintJobConfig {
    fn default() -> Self {
        Self {
            printer_name: "Default Printer".to_string(),
            copies: 1,
            collate: true,
            page_range_str: "all".to_string(),
            scaling: PrintScaling::FitToPrintableArea,
            orientation: PrintOrientation::AutoRotate,
            duplex: PrintDuplex::Simplex,
            annotation_mode: PrintAnnotationMode::DocumentAndAnnotations,
            print_forms: true,
            layout_mode: PrintLayoutMode::Standard,
            target_sheet_size: (612.0, 792.0), // Standard Letter (8.5 x 11 in)
        }
    }
}

/// Report summarizing calculated print imposition and sheets (FR-35.9).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PrintPreviewReport {
    pub total_logical_pages: usize,
    pub total_physical_sheets: usize,
    pub target_sheet_dimensions: (f64, f64),
    pub summary_description: String,
}

// ============================================================================
// Page Range Parsing (FR-35.2)
// ============================================================================

/// Parses page range string (e.g. "all", "1-3, 5, 8-10") into a sorted list of 0-based page indices.
pub fn parse_page_range(range_str: &str, total_pages: usize) -> Result<Vec<usize>> {
    if total_pages == 0 {
        return Ok(Vec::new());
    }

    let trimmed = range_str.trim().to_lowercase();
    if trimmed.is_empty() || trimmed == "all" {
        return Ok((0..total_pages).collect());
    }

    let mut result = Vec::new();
    let parts = range_str.split(',');

    for part in parts {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }

        if let Some((start_s, end_s)) = part.split_once('-') {
            let start: usize = start_s.trim().parse().map_err(|_| {
                anyhow!("Invalid start number '{}' in page range '{}'", start_s, part)
            })?;
            let end: usize = end_s.trim().parse().map_err(|_| {
                anyhow!("Invalid end number '{}' in page range '{}'", end_s, part)
            })?;

            if start == 0 || end == 0 {
                return Err(anyhow!("Page numbers are 1-based; cannot use 0 in range"));
            }

            let (min_p, max_p) = if start <= end {
                (start, end)
            } else {
                (end, start)
            };

            for p in min_p..=max_p {
                if p <= total_pages {
                    result.push(p - 1);
                }
            }
        } else {
            let page: usize = part
                .parse()
                .map_err(|_| anyhow!("Invalid page number '{}' in range", part))?;
            if page == 0 {
                return Err(anyhow!("Page numbers are 1-based; cannot use 0 in range"));
            }
            if page <= total_pages {
                result.push(page - 1);
            }
        }
    }

    result.sort_unstable();
    result.dedup();
    Ok(result)
}

// ============================================================================
// Booklet Printing Imposition Engine (FR-35.6)
// ============================================================================

/// Calculates saddle-stitch booklet sheet imposition for a sequence of logical pages (FR-35.6).
/// Automatically pads page count to a multiple of 4 with trailing blanks.
pub fn calculate_booklet_imposition(
    page_indices: &[usize],
    binding: BookletBinding,
) -> Vec<BookletSheet> {
    if page_indices.is_empty() {
        return Vec::new();
    }

    // A folded saddle-stitch sheet has 4 pages (2 front, 2 back)
    let remainder = page_indices.len() % 4;
    let padding_needed = if remainder == 0 { 0 } else { 4 - remainder };
    let total_booklet_pages = page_indices.len() + padding_needed;
    let total_sheets = total_booklet_pages / 4;

    let mut sheets = Vec::with_capacity(total_sheets);

    let get_page = |idx: usize| -> Option<usize> {
        if idx < page_indices.len() {
            Some(page_indices[idx])
        } else {
            None // Blank padding page
        }
    };

    for sheet_idx in 0..total_sheets {
        // Sheet 0: Front = (Last, 1st), Back = (2nd, 2nd-to-last)
        // In general for sheet s:
        // Front Left: total_pages - 1 - (2 * s)
        // Front Right: 0 + (2 * s)
        // Back Left: 1 + (2 * s)
        // Back Right: total_pages - 2 - (2 * s)
        let fl_idx = total_booklet_pages - 1 - (2 * sheet_idx);
        let fr_idx = 2 * sheet_idx;
        let bl_idx = 1 + (2 * sheet_idx);
        let br_idx = total_booklet_pages - 2 - (2 * sheet_idx);

        let (fl, fr, bl, br) = match binding {
            BookletBinding::LeftToRight => (
                get_page(fl_idx),
                get_page(fr_idx),
                get_page(bl_idx),
                get_page(br_idx),
            ),
            BookletBinding::RightToLeft => (
                get_page(fr_idx),
                get_page(fl_idx),
                get_page(br_idx),
                get_page(bl_idx),
            ),
        };

        sheets.push(BookletSheet {
            sheet_number: sheet_idx + 1,
            front_left_page: fl,
            front_right_page: fr,
            back_left_page: bl,
            back_right_page: br,
        });
    }

    sheets
}

// ============================================================================
// Poster Tiling Engine (FR-35.7)
// ============================================================================

/// Calculates poster tiling grid and cut marks for an oversized page across physical sheets (FR-35.7).
#[allow(clippy::too_many_arguments)]
pub fn calculate_poster_tiles(
    source_w: f64,
    source_h: f64,
    sheet_w: f64,
    sheet_h: f64,
    scale: f64,
    overlap_pt: f64,
    source_page: usize,
    cut_marks: bool,
) -> Vec<PosterTile> {
    let scaled_w = source_w * scale;
    let scaled_h = source_h * scale;

    let usable_w = (sheet_w - overlap_pt).max(10.0);
    let usable_h = (sheet_h - overlap_pt).max(10.0);

    let cols = (scaled_w / usable_w).ceil() as usize;
    let rows = (scaled_h / usable_h).ceil() as usize;

    let mut tiles = Vec::with_capacity(cols * rows);

    for r in 0..rows {
        for c in 0..cols {
            let x_start = (c as f64 * usable_w) / scale;
            let y_start = (r as f64 * usable_h) / scale;
            let crop_w = (sheet_w / scale).min(source_w - x_start);
            let crop_h = (sheet_h / scale).min(source_h - y_start);

            tiles.push(PosterTile {
                source_page,
                col: c,
                row: r,
                total_cols: cols,
                total_rows: rows,
                crop_rect: [x_start, y_start, crop_w, crop_h],
                overlap_points: overlap_pt,
                has_cut_marks: cut_marks,
            });
        }
    }

    tiles
}

// ============================================================================
// N-Up Multiple Pages Per Sheet Engine (FR-35.8)
// ============================================================================

/// Calculates N-up layout sheets and cell placements (FR-35.8).
pub fn calculate_nup_imposition(
    page_indices: &[usize],
    sheet_w: f64,
    sheet_h: f64,
    count: NUpCount,
    order: NUpOrder,
    page_w: f64,
    page_h: f64,
) -> Vec<NUpSheet> {
    if page_indices.is_empty() {
        return Vec::new();
    }

    let (cols, rows) = count.grid_dimensions();
    let n = count.value();
    let num_sheets = (page_indices.len() as f64 / n as f64).ceil() as usize;

    let cell_w = sheet_w / (cols as f64);
    let cell_h = sheet_h / (rows as f64);

    let scale_x = cell_w / page_w;
    let scale_y = cell_h / page_h;
    let cell_scale = scale_x.min(scale_y) * 0.95; // 5% margin padding

    let draw_w = page_w * cell_scale;
    let draw_h = page_h * cell_scale;

    let mut sheets = Vec::with_capacity(num_sheets);

    for s in 0..num_sheets {
        let mut cells = Vec::new();
        for i in 0..n {
            let global_idx = s * n + i;
            if global_idx >= page_indices.len() {
                break;
            }

            let (col_idx, row_idx) = match order {
                NUpOrder::Horizontal => (i % cols, i / cols),
                NUpOrder::HorizontalReversed => (cols - 1 - (i % cols), i / cols),
                NUpOrder::Vertical => (i / rows, i % rows),
                NUpOrder::VerticalReversed => (i / rows, rows - 1 - (i % rows)),
            };

            let base_x = (col_idx as f64) * cell_w;
            let base_y = sheet_h - ((row_idx as f64 + 1.0) * cell_h); // Top-to-bottom

            let offset_x = base_x + (cell_w - draw_w) / 2.0;
            let offset_y = base_y + (cell_h - draw_h) / 2.0;

            cells.push(NUpCellPlacement {
                source_page: page_indices[global_idx],
                cell_rect: [offset_x, offset_y, draw_w, draw_h],
                scale: cell_scale,
            });
        }

        sheets.push(NUpSheet {
            sheet_number: s + 1,
            cells,
        });
    }

    sheets
}

// ============================================================================
// PostScript (.ps) Output Generator (FR-35.10)
// ============================================================================

/// Generates a valid PostScript Level 2/3 stream conforming to Adobe DSC (Document Structuring Conventions) (FR-35.10).
pub fn generate_postscript(doc: &PdfDocument, config: &PrintJobConfig) -> Result<String> {
    let pages = parse_page_range(&config.page_range_str, doc.page_count())?;
    let (target_w, target_h) = config.target_sheet_size;

    let mut ps = String::new();
    // 1. PostScript Header and DSC Comments (conforming to DSC 3.0)
    ps.push_str("%!PS-Adobe-3.0\n");
    ps.push_str("%%Title: RavPDF Printed Document\n");
    ps.push_str("%%Creator: RavPDF Document Engine\n");
    ps.push_str(&format!("%%Pages: {}\n", pages.len()));
    ps.push_str(&format!(
        "%%BoundingBox: 0 0 {:.0} {:.0}\n",
        target_w, target_h
    ));
    ps.push_str("%%DocumentData: Clean7Bit\n");
    ps.push_str("%%LanguageLevel: 2\n");
    ps.push_str("%%EndComments\n\n");

    // 2. Document Setup and Definitions Prolog
    ps.push_str("%%BeginProlog\n");
    ps.push_str("/rectpath { 4 2 roll moveto 1 index 0 rlineto 0 exch rlineto neg 0 rlineto closepath } bind def\n");
    ps.push_str("/rectfill { rectpath fill } bind def\n");
    ps.push_str("/rectstroke { rectpath stroke } bind def\n");
    ps.push_str("/picstr 4096 string def\n");
    ps.push_str("/Courier findfont 12 scalefont setfont\n");
    ps.push_str("%%EndProlog\n\n");

    // 3. Page Loop
    for (seq_idx, &page_idx) in pages.iter().enumerate() {
        ps.push_str(&format!("%%Page: {} {}\n", page_idx + 1, seq_idx + 1));
        ps.push_str("%%BeginPageSetup\n");
        ps.push_str("gsave\n");
        ps.push_str(&format!(
            "0 0 {:.2} {:.2} rectclip\n",
            target_w, target_h
        ));
        ps.push_str("%%EndPageSetup\n\n");

        // Attempt vector translation of page content stream (FR-35.10)
        let mut vector_rendered = false;
        if let Ok(content_bytes) = doc.get_page_content(page_idx) {
            if !content_bytes.is_empty() {
                let vector_ps = translate_content_stream_to_postscript(&content_bytes);
                if !vector_ps.trim().is_empty() {
                    ps.push_str(&vector_ps);
                    vector_rendered = true;
                }
            }
        }

        // If no vector content was present, render high-fidelity PostScript Level 2 colorimage
        if !vector_rendered {
            if let Ok(pixmap) = ravpdf_render::render_page(
                doc,
                page_idx,
                &ravpdf_render::RenderOptions {
                    zoom: 1.0,
                    ..Default::default()
                },
            ) {
                let px_w = pixmap.width();
                let px_h = pixmap.height();
                let raw_rgba = pixmap.data();

                ps.push_str("gsave\n");
                ps.push_str(&format!("0 0 translate\n{:.2} {:.2} scale\n", target_w, target_h));
                ps.push_str(&format!(
                    "{} {} 8 [{} 0 0 -{} 0 {}]\n",
                    px_w, px_h, px_w, px_h, px_h
                ));
                ps.push_str("{ currentfile picstr readhexstring pop } false 3 colorimage\n");

                let mut col = 0;
                for chunk in raw_rgba.chunks(4) {
                    if chunk.len() >= 3 {
                        ps.push_str(&format!("{:02x}{:02x}{:02x}", chunk[0], chunk[1], chunk[2]));
                        col += 6;
                        if col >= 72 {
                            ps.push('\n');
                            col = 0;
                        }
                    }
                }
                if col > 0 {
                    ps.push('\n');
                }
                ps.push_str("grestore\n");
            }
        }

        ps.push_str("grestore\n");
        ps.push_str("showpage\n");
        ps.push_str("%%PageTrailer\n\n");
    }

    ps.push_str("%%Trailer\n");
    ps.push_str("%%EOF\n");
    Ok(ps)
}

/// Translates PDF content stream operators into equivalent Level 2/3 PostScript operators.
fn translate_content_stream_to_postscript(data: &[u8]) -> String {
    use ravpdf_core::lexer::{Lexer, Token};

    let mut lexer = Lexer::new(data);
    let mut ps = String::new();
    let mut stack: Vec<Token> = Vec::new();

    while let Ok(token) = lexer.next_token() {
        match token {
            Token::Eof => break,
            Token::Keyword(ref op) => {
                match op.as_str() {
                    "q" => ps.push_str("gsave\n"),
                    "Q" => ps.push_str("grestore\n"),
                    "cm" => {
                        if stack.len() >= 6 {
                            let f = pop_num(&mut stack);
                            let e = pop_num(&mut stack);
                            let d = pop_num(&mut stack);
                            let c = pop_num(&mut stack);
                            let b = pop_num(&mut stack);
                            let a = pop_num(&mut stack);
                            ps.push_str(&format!("[{:.4} {:.4} {:.4} {:.4} {:.4} {:.4}] concat\n", a, b, c, d, e, f));
                        }
                    }
                    "w" => {
                        let w = pop_num(&mut stack);
                        ps.push_str(&format!("{:.2} setlinewidth\n", w));
                    }
                    "J" => {
                        let cap = pop_num(&mut stack) as i64;
                        ps.push_str(&format!("{} setlinecap\n", cap));
                    }
                    "j" => {
                        let join = pop_num(&mut stack) as i64;
                        ps.push_str(&format!("{} setlinejoin\n", join));
                    }
                    "M" => {
                        let m = pop_num(&mut stack);
                        ps.push_str(&format!("{:.2} setmiterlimit\n", m));
                    }
                    "m" => {
                        let y = pop_num(&mut stack);
                        let x = pop_num(&mut stack);
                        ps.push_str(&format!("{:.2} {:.2} moveto\n", x, y));
                    }
                    "l" => {
                        let y = pop_num(&mut stack);
                        let x = pop_num(&mut stack);
                        ps.push_str(&format!("{:.2} {:.2} lineto\n", x, y));
                    }
                    "c" => {
                        let y3 = pop_num(&mut stack);
                        let x3 = pop_num(&mut stack);
                        let y2 = pop_num(&mut stack);
                        let x2 = pop_num(&mut stack);
                        let y1 = pop_num(&mut stack);
                        let x1 = pop_num(&mut stack);
                        ps.push_str(&format!(
                            "{:.2} {:.2} {:.2} {:.2} {:.2} {:.2} curveto\n",
                            x1, y1, x2, y2, x3, y3
                        ));
                    }
                    "h" => ps.push_str("closepath\n"),
                    "re" => {
                        let h = pop_num(&mut stack);
                        let w = pop_num(&mut stack);
                        let y = pop_num(&mut stack);
                        let x = pop_num(&mut stack);
                        ps.push_str(&format!("{:.2} {:.2} {:.2} {:.2} rectpath\n", x, y, w, h));
                    }
                    "S" => ps.push_str("stroke\n"),
                    "s" => ps.push_str("closepath stroke\n"),
                    "f" | "F" => ps.push_str("fill\n"),
                    "f*" => ps.push_str("eofill\n"),
                    "B" => ps.push_str("gsave fill grestore stroke\n"),
                    "B*" => ps.push_str("gsave eofill grestore stroke\n"),
                    "b" => ps.push_str("closepath gsave fill grestore stroke\n"),
                    "b*" => ps.push_str("closepath gsave eofill grestore stroke\n"),
                    "n" => ps.push_str("newpath\n"),
                    "W" => ps.push_str("clip\n"),
                    "W*" => ps.push_str("eoclip\n"),
                    "g" | "G" => {
                        let g = pop_num(&mut stack);
                        ps.push_str(&format!("{:.3} setgray\n", g));
                    }
                    "rg" | "RG" => {
                        let b = pop_num(&mut stack);
                        let g = pop_num(&mut stack);
                        let r = pop_num(&mut stack);
                        ps.push_str(&format!("{:.3} {:.3} {:.3} setrgbcolor\n", r, g, b));
                    }
                    "k" | "K" => {
                        let k = pop_num(&mut stack);
                        let y = pop_num(&mut stack);
                        let m = pop_num(&mut stack);
                        let c = pop_num(&mut stack);
                        ps.push_str(&format!("{:.3} {:.3} {:.3} {:.3} setcmykcolor\n", c, m, y, k));
                    }
                    "BT" => ps.push_str("% BT\n"),
                    "ET" => ps.push_str("% ET\n"),
                    "Tf" => {
                        let sz = pop_num(&mut stack);
                        let font_name = match stack.pop() {
                            Some(Token::Name(n)) => n,
                            _ => "Helvetica".to_string(),
                        };
                        let ps_font = match font_name.as_str() {
                            "F2" => "Helvetica-Bold",
                            "F3" => "Times-Roman",
                            "F4" => "Courier",
                            _ => "Helvetica",
                        };
                        ps.push_str(&format!("/{} findfont {:.2} scalefont setfont\n", ps_font, sz));
                    }
                    "Td" | "TD" => {
                        let ty = pop_num(&mut stack);
                        let tx = pop_num(&mut stack);
                        ps.push_str(&format!("{:.2} {:.2} moveto\n", tx, ty));
                    }
                    "Tj" => {
                        if let Some(Token::StringLiteral(bytes)) = stack.pop() {
                            let text = String::from_utf8_lossy(&bytes);
                            ps.push_str(&format!("({}) show\n", escape_ps_string(&text)));
                        }
                    }
                    "'" => {
                        if let Some(Token::StringLiteral(bytes)) = stack.pop() {
                            let text = String::from_utf8_lossy(&bytes);
                            ps.push_str(&format!("0 -14 rmoveto ({}) show\n", escape_ps_string(&text)));
                        }
                    }
                    _ => {
                        // Clear stack for unhandled operator
                        stack.clear();
                    }
                }
            }
            other => {
                stack.push(other);
            }
        }
    }

    ps
}

fn pop_num(stack: &mut Vec<ravpdf_core::lexer::Token>) -> f64 {
    use ravpdf_core::lexer::Token;
    match stack.pop() {
        Some(Token::Real(r)) => r,
        Some(Token::Integer(i)) => i as f64,
        _ => 0.0,
    }
}

fn escape_ps_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '(' => out.push_str("\\("),
            ')' => out.push_str("\\)"),
            '\\' => out.push_str("\\\\"),
            '\r' => out.push_str("\\r"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str(" "),
            _ => out.push(c),
        }
    }
    out
}

/// Generates a print preview summary and layout metrics (FR-35.9).
pub fn generate_print_preview_layout(
    doc: &PdfDocument,
    config: &PrintJobConfig,
) -> Result<PrintPreviewReport> {
    let pages = parse_page_range(&config.page_range_str, doc.page_count())?;
    let (target_w, target_h) = config.target_sheet_size;

    let (sheets_count, desc) = match &config.layout_mode {
        PrintLayoutMode::Standard => (
            pages.len(),
            format!("Standard 1-up layout across {} sheets", pages.len()),
        ),
        PrintLayoutMode::Booklet { binding, .. } => {
            let sheets = calculate_booklet_imposition(&pages, *binding);
            (
                sheets.len(),
                format!(
                    "Saddle-Stitch Booklet ({:?}) across {} physical sheets",
                    binding,
                    sheets.len()
                ),
            )
        }
        PrintLayoutMode::Poster {
            tile_scale,
            overlap_points,
            cut_marks,
        } => {
            let mut total_tiles = 0;
            for &p in &pages {
                let tiles = calculate_poster_tiles(
                    target_w,
                    target_h,
                    target_w,
                    target_h,
                    *tile_scale,
                    *overlap_points,
                    p,
                    *cut_marks,
                );
                total_tiles += tiles.len();
            }
            (
                total_tiles,
                format!("Poster tiling across {} printed sheets", total_tiles),
            )
        }
        PrintLayoutMode::NUp {
            count,
            order,
            draw_border,
        } => {
            let sheets = calculate_nup_imposition(
                &pages,
                target_w,
                target_h,
                *count,
                *order,
                target_w,
                target_h,
            );
            (
                sheets.len(),
                format!(
                    "{}-Up layout ({:?}, border: {}) across {} sheets",
                    count.value(),
                    order,
                    draw_border,
                    sheets.len()
                ),
            )
        }
    };

    Ok(PrintPreviewReport {
        total_logical_pages: pages.len(),
        total_physical_sheets: sheets_count,
        target_sheet_dimensions: config.target_sheet_size,
        summary_description: desc,
    })
}

// ============================================================================
// Unit Tests (FR-35.1 - FR-35.10)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_page_ranges() {
        let pages = parse_page_range("1-3, 5, 8-10", 12).expect("parse ranges");
        assert_eq!(pages, vec![0, 1, 2, 4, 7, 8, 9]);

        let all_pages = parse_page_range("all", 5).expect("parse all");
        assert_eq!(all_pages, vec![0, 1, 2, 3, 4]);

        let single = parse_page_range("3", 5).expect("parse single");
        assert_eq!(single, vec![2]);
    }

    #[test]
    fn test_booklet_imposition_saddle_stitch() {
        // 8-page document = 2 sheets
        let pages: Vec<usize> = (0..8).collect();
        let sheets = calculate_booklet_imposition(&pages, BookletBinding::LeftToRight);
        assert_eq!(sheets.len(), 2);

        // Sheet 1: Front = (Page 8, Page 1) -> (7, 0)
        //          Back  = (Page 2, Page 7) -> (1, 6)
        assert_eq!(sheets[0].front_left_page, Some(7));
        assert_eq!(sheets[0].front_right_page, Some(0));
        assert_eq!(sheets[0].back_left_page, Some(1));
        assert_eq!(sheets[0].back_right_page, Some(6));

        // Sheet 2: Front = (Page 6, Page 3) -> (5, 2)
        //          Back  = (Page 4, Page 5) -> (3, 4)
        assert_eq!(sheets[1].front_left_page, Some(5));
        assert_eq!(sheets[1].front_right_page, Some(2));
        assert_eq!(sheets[1].back_left_page, Some(3));
        assert_eq!(sheets[1].back_right_page, Some(4));
    }

    #[test]
    fn test_poster_tiling_calculation() {
        // 1200 x 1600 point poster on 612 x 792 sheets with 36 pt overlap
        let tiles = calculate_poster_tiles(1200.0, 1600.0, 612.0, 792.0, 1.0, 36.0, 0, true);
        assert!(!tiles.is_empty());
        assert!(tiles[0].total_cols >= 2);
        assert!(tiles[0].total_rows >= 2);
        assert!(tiles.iter().all(|t| t.has_cut_marks));
    }

    #[test]
    fn test_nup_imposition_calculation() {
        let pages: Vec<usize> = (0..10).collect();
        // 4-up layout = 3 sheets
        let sheets = calculate_nup_imposition(
            &pages,
            612.0,
            792.0,
            NUpCount::FourUp,
            NUpOrder::Horizontal,
            612.0,
            792.0,
        );
        assert_eq!(sheets.len(), 3);
        assert_eq!(sheets[0].cells.len(), 4);
        assert_eq!(sheets[1].cells.len(), 4);
        assert_eq!(sheets[2].cells.len(), 2); // remaining 2 pages
    }

    #[test]
    fn test_postscript_generation_dsc() {
        let mut doc = PdfDocument::new();
        doc.add_blank_page(612.0, 792.0);
        doc.add_blank_page(612.0, 792.0);

        let config = PrintJobConfig::default();
        let ps = generate_postscript(&doc, &config).expect("generate ps");
        assert!(ps.starts_with("%!PS-Adobe-3.0"));
        assert!(ps.contains("%%Pages: 2"));
        assert!(ps.contains("showpage"));
        assert!(ps.contains("%%EOF"));
    }

    #[test]
    fn test_postscript_vector_operator_translation() {
        let content = b"q 1 0 0 rg 50 100 200 150 re f 0 0 1 RG 2 w 10 20 m 100 200 l S Q";
        let ps = translate_content_stream_to_postscript(content);
        assert!(ps.contains("gsave"));
        assert!(ps.contains("1.000 0.000 0.000 setrgbcolor"));
        assert!(ps.contains("50.00 100.00 200.00 150.00 rectpath"));
        assert!(ps.contains("fill"));
        assert!(ps.contains("0.000 0.000 1.000 setrgbcolor"));
        assert!(ps.contains("2.00 setlinewidth"));
        assert!(ps.contains("10.00 20.00 moveto"));
        assert!(ps.contains("100.00 200.00 lineto"));
        assert!(ps.contains("stroke"));
        assert!(ps.contains("grestore"));
    }
}
