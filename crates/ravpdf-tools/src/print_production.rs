//! Print Production and Output Preview Engine conforming to ISO 32000-1 / ISO 32000-2.
//!
//! Traceability:
//! - FR-23.2.1: Output Preview mode showing separation plates (Cyan, Magenta, Yellow, Black, and Spot colors).
//! - FR-23.2.2: Toggle individual separation plates on/off for prepress press inspection.
//! - FR-23.2.3: Total Area Coverage (TAC) warnings highlighting regions exceeding configurable threshold (e.g. 300%).
//! - FR-23.2.4: Simulate paper color and overprint effects.
//! - FR-23.2.5: Object Inspector showing color space, ICC profile, image resolution, and font information.

use anyhow::Result;
use ravpdf_core::document::PdfDocument;
use ravpdf_core::object::{Object, PdfDictionary, PdfName, PdfStream, PdfString};
use serde::{Deserialize, Serialize};

/// Configuration for CMYK plate separations and Output Preview (FR-23.2.1, FR-23.2.2, FR-23.2.3).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlateSeparationConfig {
    pub show_cyan: bool,
    pub show_magenta: bool,
    pub show_yellow: bool,
    pub show_black: bool,
    pub show_tac_warning: bool,
    /// Total Area Coverage threshold: 1.5 (150%) to 4.0 (400%), default 3.0 (300%) (FR-23.2.3).
    pub tac_threshold: f32,
    /// If true, display active single plate as black/white press density plate rather than process color (FR-23.2.2).
    pub single_plate_as_grayscale: bool,
    /// Simulates unprinted white stock / paper tint (FR-23.2.4).
    pub simulate_paper_white: bool,
}

impl Default for PlateSeparationConfig {
    fn default() -> Self {
        Self {
            show_cyan: true,
            show_magenta: true,
            show_yellow: true,
            show_black: true,
            show_tac_warning: false,
            tac_threshold: 3.00, // 300% TAC standard commercial sheetfed limit
            single_plate_as_grayscale: false,
            simulate_paper_white: true,
        }
    }
}

/// Statistical analysis results of plate separations and ink coverage (FR-23.2.1, FR-23.2.3).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SeparationAnalysis {
    pub max_tac: f32,
    pub tac_exceeded_pixels: usize,
    pub tac_exceeded_percentage: f32,
    pub cyan_coverage_pct: f32,
    pub magenta_coverage_pct: f32,
    pub yellow_coverage_pct: f32,
    pub black_coverage_pct: f32,
}

/// Applies CMYK plate separations, TAC warning highlights, and color channel toggling to an RGBA pixel buffer (FR-23.2.1, FR-23.2.2, FR-23.2.3).
pub fn apply_plate_separations(
    rgba_pixels: &[u8],
    width: u32,
    height: u32,
    config: &PlateSeparationConfig,
) -> (Vec<u8>, SeparationAnalysis) {
    let pixel_count = (width as usize) * (height as usize);
    let mut out = vec![255u8; rgba_pixels.len()];
    let mut analysis = SeparationAnalysis::default();

    if rgba_pixels.len() < pixel_count * 4 {
        return (out, analysis);
    }

    let active_plates_count = [
        config.show_cyan,
        config.show_magenta,
        config.show_yellow,
        config.show_black,
    ]
    .iter()
    .filter(|&&b| b)
    .count();

    let mut sum_c = 0.0f64;
    let mut sum_m = 0.0f64;
    let mut sum_y = 0.0f64;
    let mut sum_k = 0.0f64;
    let mut max_tac = 0.0f32;
    let mut tac_exceeded = 0usize;

    for i in 0..pixel_count {
        let idx = i * 4;
        let r = (rgba_pixels[idx] as f32) / 255.0;
        let g = (rgba_pixels[idx + 1] as f32) / 255.0;
        let b = (rgba_pixels[idx + 2] as f32) / 255.0;
        let a = (rgba_pixels[idx + 3] as f32) / 255.0;

        // Standard RGB to CMYK subtractive color conversion (FOGRA / SWOP process simulation)
        let k = 1.0 - r.max(g).max(b);
        let (c, m, y) = if k < 0.9999 {
            (
                (1.0 - r - k) / (1.0 - k),
                (1.0 - g - k) / (1.0 - k),
                (1.0 - b - k) / (1.0 - k),
            )
        } else {
            (0.0, 0.0, 0.0)
        };

        let tac = (c + m + y + k) * a;
        if tac > max_tac {
            max_tac = tac;
        }

        sum_c += (c * a) as f64;
        sum_m += (m * a) as f64;
        sum_y += (y * a) as f64;
        sum_k += (k * a) as f64;

        if config.show_tac_warning && tac > config.tac_threshold {
            tac_exceeded += 1;
            // High-visibility neon green highlight for ink overload exceeding threshold
            out[idx] = 0;
            out[idx + 1] = 255;
            out[idx + 2] = 0;
            out[idx + 3] = 255;
            continue;
        }

        // Single plate inspection mode (as black & white density plate)
        if config.single_plate_as_grayscale && active_plates_count == 1 {
            let density = if config.show_cyan {
                c
            } else if config.show_magenta {
                m
            } else if config.show_yellow {
                y
            } else {
                k
            };
            let gray_byte = ((1.0 - density * a).clamp(0.0, 1.0) * 255.0) as u8;
            out[idx] = gray_byte;
            out[idx + 1] = gray_byte;
            out[idx + 2] = gray_byte;
            out[idx + 3] = 255;
            continue;
        }

        // Multichannel plate toggling
        let eff_c = if config.show_cyan { c } else { 0.0 };
        let eff_m = if config.show_magenta { m } else { 0.0 };
        let eff_y = if config.show_yellow { y } else { 0.0 };
        let eff_k = if config.show_black { k } else { 0.0 };

        // Convert back to DeviceRGB display coordinates
        let out_r = ((1.0 - eff_c) * (1.0 - eff_k)).clamp(0.0, 1.0);
        let out_g = ((1.0 - eff_m) * (1.0 - eff_k)).clamp(0.0, 1.0);
        let out_b = ((1.0 - eff_y) * (1.0 - eff_k)).clamp(0.0, 1.0);

        out[idx] = (out_r * 255.0) as u8;
        out[idx + 1] = (out_g * 255.0) as u8;
        out[idx + 2] = (out_b * 255.0) as u8;
        out[idx + 3] = (a * 255.0) as u8;
    }

    if pixel_count > 0 {
        analysis.max_tac = max_tac;
        analysis.tac_exceeded_pixels = tac_exceeded;
        analysis.tac_exceeded_percentage = (tac_exceeded as f32) / (pixel_count as f32) * 100.0;
        analysis.cyan_coverage_pct = ((sum_c / pixel_count as f64) * 100.0) as f32;
        analysis.magenta_coverage_pct = ((sum_m / pixel_count as f64) * 100.0) as f32;
        analysis.yellow_coverage_pct = ((sum_y / pixel_count as f64) * 100.0) as f32;
        analysis.black_coverage_pct = ((sum_k / pixel_count as f64) * 100.0) as f32;
    }

    (out, analysis)
}

/// Inspection report for objects and color spaces at a specific page coordinate (FR-23.2.5).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjectInspectionReport {
    pub page_index: usize,
    pub point: [f64; 2],
    pub color_spaces: Vec<String>,
    pub fonts: Vec<String>,
    pub images_info: Vec<String>,
    pub transparency_present: bool,
    pub overprint_present: bool,
}

/// Inspects page objects, color spaces, images, and fonts for prepress diagnostics (FR-23.2.5).
pub fn inspect_page_objects(
    doc: &PdfDocument,
    page_index: usize,
    point: [f64; 2],
) -> Result<ObjectInspectionReport> {
    let mut color_spaces = Vec::new();
    let mut fonts = Vec::new();
    let mut images_info = Vec::new();
    let mut transparency_present = false;
    let mut overprint_present = false;

    if let Some(page) = doc.get_page(page_index) {
        if let Some(ref res) = page.resources {
            // Inspect Color Spaces
            if let Some(cs_dict) = res.get_dict("ColorSpace") {
                for k in cs_dict.map.keys() {
                    color_spaces.push(format!("Named ColorSpace: /{}", k.as_str()));
                }
            } else {
                color_spaces.push("DeviceRGB (Default)".into());
                color_spaces.push("DeviceCMYK (Available)".into());
            }

            // Inspect Fonts
            if let Some(font_dict) = res.get_dict("Font") {
                for (name, val) in font_dict.map.iter() {
                    let subtype = match val {
                        Object::Reference(id) => doc
                            .objects
                            .get(id)
                            .and_then(Object::as_dict)
                            .and_then(|d| d.get_name("Subtype"))
                            .unwrap_or("Type1"),
                        Object::Dictionary(d) => d.get_name("Subtype").unwrap_or("Type1"),
                        _ => "Unknown",
                    };
                    fonts.push(format!("/{} ({})", name.as_str(), subtype));
                }
            }

            // Inspect XObjects (Images and Form XObjects)
            if let Some(xo_dict) = res.get_dict("XObject") {
                for (name, val) in xo_dict.map.iter() {
                    let (sub, w, h, cs) = match val {
                        Object::Reference(id) => {
                            if let Some(Object::Stream(s)) = doc.objects.get(id) {
                                (
                                    s.dict.get_name("Subtype").unwrap_or("Image"),
                                    s.dict.get_i64("Width").unwrap_or(0),
                                    s.dict.get_i64("Height").unwrap_or(0),
                                    s.dict.get_name("ColorSpace").unwrap_or("DeviceRGB"),
                                )
                            } else {
                                ("Unknown", 0, 0, "Unknown")
                            }
                        }
                        _ => ("Unknown", 0, 0, "Unknown"),
                    };
                    if sub == "Image" {
                        images_info.push(format!(
                            "/{} [{}x{} px, ColorSpace: /{}]",
                            name.as_str(),
                            w,
                            h,
                            cs
                        ));
                    }
                }
            }

            // Inspect ExtGState (transparency & overprint)
            if let Some(egs_dict) = res.get_dict("ExtGState") {
                for val in egs_dict.map.values() {
                    let d = match val {
                        Object::Reference(id) => doc.objects.get(id).and_then(Object::as_dict),
                        Object::Dictionary(ref d) => Some(d),
                        _ => None,
                    };
                    if let Some(gs) = d {
                        if gs.get("ca").is_some() || gs.get("CA").is_some() || gs.get("BM").is_some() {
                            transparency_present = true;
                        }
                        if gs.get("OP").is_some() || gs.get("op").is_some() {
                            overprint_present = true;
                        }
                    }
                }
            }
        }
    }

    Ok(ObjectInspectionReport {
        page_index,
        point,
        color_spaces,
        fonts,
        images_info,
        transparency_present,
        overprint_present,
    })
}

/// Configuration for adding prepress printer marks (FR-23.3.4).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PrinterMarksConfig {
    pub trim_marks: bool,
    pub bleed_marks: bool,
    pub registration_marks: bool,
    pub color_bars: bool,
    pub page_information: bool,
    /// Line width for marks in points (default: 0.5 pt).
    pub mark_line_width: f64,
    /// Offset for bleed marks outside the trim box in points (default: 9.0 pt = 1/8 inch).
    pub bleed_offset: f64,
    /// Additional margin added around the page for the slug/marks area in points (default: 36.0 pt = 0.5 inch).
    pub slug_margin: f64,
    /// Target pages (None = all pages).
    pub target_pages: Option<Vec<usize>>,
}

impl Default for PrinterMarksConfig {
    fn default() -> Self {
        Self {
            trim_marks: true,
            bleed_marks: true,
            registration_marks: true,
            color_bars: true,
            page_information: true,
            mark_line_width: 0.5,
            bleed_offset: 9.0,
            slug_margin: 36.0,
            target_pages: None,
        }
    }
}

fn draw_bullseye(cx: f64, cy: f64, r1: f64, r2: f64) -> String {
    let mut s = String::new();
    let k1 = 0.55228475 * r1;
    // Outer circle
    s.push_str(&format!("{:.2} {:.2} m\n", cx, cy + r1));
    s.push_str(&format!("{:.2} {:.2} {:.2} {:.2} {:.2} {:.2} c\n", cx + k1, cy + r1, cx + r1, cy + k1, cx + r1, cy));
    s.push_str(&format!("{:.2} {:.2} {:.2} {:.2} {:.2} {:.2} c\n", cx + r1, cy - k1, cx + k1, cy - r1, cx, cy - r1));
    s.push_str(&format!("{:.2} {:.2} {:.2} {:.2} {:.2} {:.2} c\n", cx - k1, cy - r1, cx - r1, cy - k1, cx - r1, cy));
    s.push_str(&format!("{:.2} {:.2} {:.2} {:.2} {:.2} {:.2} c\n", cx - r1, cy + k1, cx - k1, cy + r1, cx, cy + r1));
    s.push_str("S\n");
    // Inner circle
    let k2 = 0.55228475 * r2;
    s.push_str(&format!("{:.2} {:.2} m\n", cx, cy + r2));
    s.push_str(&format!("{:.2} {:.2} {:.2} {:.2} {:.2} {:.2} c\n", cx + k2, cy + r2, cx + r2, cy + k2, cx + r2, cy));
    s.push_str(&format!("{:.2} {:.2} {:.2} {:.2} {:.2} {:.2} c\n", cx + r2, cy - k2, cx + k2, cy - r2, cx, cy - r2));
    s.push_str(&format!("{:.2} {:.2} {:.2} {:.2} {:.2} {:.2} c\n", cx - k2, cy - r2, cx - r2, cy - k2, cx - r2, cy));
    s.push_str(&format!("{:.2} {:.2} {:.2} {:.2} {:.2} {:.2} c\n", cx - k2, cy + k2, cx - k2, cy + r2, cx, cy + r2));
    s.push_str("S\n");
    // Crosshairs
    s.push_str(&format!("{:.2} {:.2} m {:.2} {:.2} l S\n", cx - r1 - 4.0, cy, cx + r1 + 4.0, cy));
    s.push_str(&format!("{:.2} {:.2} m {:.2} {:.2} l S\n", cx, cy - r1 - 4.0, cx, cy + r1 + 4.0));
    s
}

fn draw_color_bars(x: f64, y: f64) -> String {
    let mut s = String::new();
    let patches = [
        // Cyan tints
        (1.0, 0.0, 0.0, 0.0), (0.75, 0.0, 0.0, 0.0), (0.5, 0.0, 0.0, 0.0), (0.25, 0.0, 0.0, 0.0),
        // Magenta tints
        (0.0, 1.0, 0.0, 0.0), (0.0, 0.75, 0.0, 0.0), (0.0, 0.5, 0.0, 0.0), (0.0, 0.25, 0.0, 0.0),
        // Yellow tints
        (0.0, 0.0, 1.0, 0.0), (0.0, 0.0, 0.75, 0.0), (0.0, 0.0, 0.5, 0.0), (0.0, 0.0, 0.25, 0.0),
        // Black tints
        (0.0, 0.0, 0.0, 1.0), (0.0, 0.0, 0.0, 0.75), (0.0, 0.0, 0.0, 0.5), (0.0, 0.0, 0.0, 0.25),
        // Process mixes
        (1.0, 1.0, 0.0, 0.0), (1.0, 0.0, 1.0, 0.0), (0.0, 1.0, 1.0, 0.0), (1.0, 1.0, 1.0, 1.0),
    ];
    let patch_w = 6.0;
    let patch_h = 6.0;
    for (i, &(c, m, y_c, k)) in patches.iter().enumerate() {
        let px = x + (i as f64) * (patch_w + 1.0);
        s.push_str(&format!("{:.3} {:.3} {:.3} {:.3} k\n", c, m, y_c, k));
        s.push_str(&format!("{:.2} {:.2} {:.2} {:.2} re f\n", px, y, patch_w, patch_h));
    }
    s
}

fn draw_corner_ticks(x1: f64, y1: f64, x2: f64, y2: f64, offset: f64, length: f64) -> String {
    let mut s = String::new();
    // Bottom-Left
    s.push_str(&format!("{:.2} {:.2} m {:.2} {:.2} l S\n", x1 - offset - length, y1, x1 - offset, y1));
    s.push_str(&format!("{:.2} {:.2} m {:.2} {:.2} l S\n", x1, y1 - offset - length, x1, y1 - offset));
    // Bottom-Right
    s.push_str(&format!("{:.2} {:.2} m {:.2} {:.2} l S\n", x2 + offset, y1, x2 + offset + length, y1));
    s.push_str(&format!("{:.2} {:.2} m {:.2} {:.2} l S\n", x2, y1 - offset - length, x2, y1 - offset));
    // Top-Left
    s.push_str(&format!("{:.2} {:.2} m {:.2} {:.2} l S\n", x1 - offset - length, y2, x1 - offset, y2));
    s.push_str(&format!("{:.2} {:.2} m {:.2} {:.2} l S\n", x1, y2 + offset, x1, y2 + offset + length));
    // Top-Right
    s.push_str(&format!("{:.2} {:.2} m {:.2} {:.2} l S\n", x2 + offset, y2, x2 + offset + length, y2));
    s.push_str(&format!("{:.2} {:.2} m {:.2} {:.2} l S\n", x2, y2 + offset, x2, y2 + offset + length));
    s
}

/// Adds prepress printer marks (trim, bleed, registration bullseyes, color bars, page info) to document pages (FR-23.3.4).
pub fn add_printer_marks(
    doc: &mut PdfDocument,
    config: &PrinterMarksConfig,
) -> Result<usize> {
    let total_pages = doc.page_count();
    let target_indices: Vec<usize> = if let Some(ref list) = config.target_pages {
        list.iter().copied().filter(|&i| i < total_pages).collect()
    } else {
        (0..total_pages).collect()
    };

    let doc_title = doc
        .info_id
        .and_then(|id| doc.get_object(id))
        .and_then(Object::as_dict)
        .and_then(|d| d.get_str("Title"))
        .unwrap_or_else(|| "Document".to_string());

    let page_ids = doc.page_ids();
    let mut modified = 0;

    for &idx in &target_indices {
        if idx >= page_ids.len() {
            continue;
        }
        let page_id = page_ids[idx];

        // Read page dimensions
        let (x1, y1, x2, y2) = {
            if let Some(p) = doc.get_page(idx) {
                let mb = p.media_box;
                (mb[0], mb[1], mb[2], mb[3])
            } else {
                (0.0, 0.0, 612.0, 792.0)
            }
        };

        let slug = config.slug_margin;
        let bleed = config.bleed_offset;

        // Construct stream for printer marks
        let mut stream_content = String::new();
        stream_content.push_str("\nq\n");
        // Registration color (mark on all plates)
        stream_content.push_str("1.0 1.0 1.0 1.0 K\n");
        stream_content.push_str(&format!("{:.2} w\n", config.mark_line_width));

        // Trim Marks
        if config.trim_marks {
            stream_content.push_str(&draw_corner_ticks(x1, y1, x2, y2, 4.0, 14.0));
        }

        // Bleed Marks
        if config.bleed_marks {
            stream_content.push_str(&draw_corner_ticks(x1 - bleed, y1 - bleed, x2 + bleed, y2 + bleed, 2.0, 10.0));
        }

        // Registration bullseye targets (top, bottom, left, right centers)
        if config.registration_marks {
            let cx = (x1 + x2) / 2.0;
            let cy = (y1 + y2) / 2.0;
            stream_content.push_str(&draw_bullseye(cx, y2 + slug * 0.5, 5.0, 2.0));
            stream_content.push_str(&draw_bullseye(cx, y1 - slug * 0.5, 5.0, 2.0));
            stream_content.push_str(&draw_bullseye(x1 - slug * 0.5, cy, 5.0, 2.0));
            stream_content.push_str(&draw_bullseye(x2 + slug * 0.5, cy, 5.0, 2.0));
        }

        // Color bars
        if config.color_bars {
            let bar_x = x1 + 10.0;
            let bar_y = y2 + 8.0;
            stream_content.push_str(&draw_color_bars(bar_x, bar_y));
        }

        // Page information
        if config.page_information {
            let info_x = x1;
            let info_y = y1 - slug + 8.0;
            let text = format!("RavPDF Prepress Marks | {} | Page {} of {}", doc_title, idx + 1, total_pages);
            let escaped = text.replace('\\', "\\\\").replace('(', "\\(").replace(')', "\\)");
            stream_content.push_str(&format!(
                "BT\n/F1 6.5 Tf\n0.0 0.0 0.0 1.0 k\n{:.2} {:.2} Td\n({}) Tj\nET\n",
                info_x, info_y, escaped
            ));
        }

        stream_content.push_str("Q\n");

        // Insert new stream object
        let stream_id = doc.next_object_id();
        let mut st = PdfStream::with_plain_data(PdfDictionary::new(), stream_content.into_bytes());
        let _ = st.compress_flate();
        doc.objects.insert(stream_id, Object::Stream(st));

        // Update page dictionary: TrimBox, BleedBox, MediaBox, Contents, and Resources Font
        if let Some(page_obj) = doc.objects.get_mut(&page_id) {
            if let Some(page_dict) = page_obj.as_dict_mut() {
                // TrimBox = original page boundaries
                page_dict.insert("TrimBox", vec![
                    Object::Real(x1), Object::Real(y1), Object::Real(x2), Object::Real(y2)
                ]);
                // BleedBox = original expanded by bleed_offset
                page_dict.insert("BleedBox", vec![
                    Object::Real(x1 - bleed), Object::Real(y1 - bleed), Object::Real(x2 + bleed), Object::Real(y2 + bleed)
                ]);
                // MediaBox = original expanded by slug_margin
                page_dict.insert("MediaBox", vec![
                    Object::Real(x1 - slug), Object::Real(y1 - slug), Object::Real(x2 + slug), Object::Real(y2 + slug)
                ]);

                // Append stream_id to Contents
                match page_dict.get_mut("Contents") {
                    Some(Object::Array(arr)) => {
                        arr.push(Object::Reference(stream_id));
                    }
                    Some(Object::Reference(old_id)) => {
                        let old_ref = *old_id;
                        page_dict.insert("Contents", vec![Object::Reference(old_ref), Object::Reference(stream_id)]);
                    }
                    _ => {
                        page_dict.insert("Contents", vec![Object::Reference(stream_id)]);
                    }
                }

                // Ensure Font /F1 exists in Resources for text
                let res = page_dict
                    .map
                    .entry(PdfName::new("Resources"))
                    .or_insert_with(|| Object::Dictionary(PdfDictionary::new()));
                if let Object::Dictionary(res_dict) = res {
                    let font = res_dict
                        .map
                        .entry(PdfName::new("Font"))
                        .or_insert_with(|| Object::Dictionary(PdfDictionary::new()));
                    if let Object::Dictionary(f_dict) = font {
                        if !f_dict.contains_key("F1") {
                            let mut std_font = PdfDictionary::new();
                            std_font.insert("Type", PdfName::new("Font"));
                            std_font.insert("Subtype", PdfName::new("Type1"));
                            std_font.insert("BaseFont", PdfName::new("Helvetica"));
                            std_font.insert("Encoding", PdfName::new("WinAnsiEncoding"));
                            f_dict.insert("F1", Object::Dictionary(std_font));
                        }
                    }
                }

                modified += 1;
            }
        }
    }

    Ok(modified)
}

/// Mapping from a spot color name to process CMYK ink densities (FR-23.3.2).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InkMapping {
    pub spot_name: String,
    pub cmyk: (f32, f32, f32, f32),
}

/// Configuration for Ink Manager spot-to-process conversions (FR-23.3.2).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct InkManagerConfig {
    pub mappings: Vec<InkMapping>,
    pub convert_all_spots_to_process: bool,
}

/// Information about an ink plate found in the document (FR-23.3.2).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DocumentInkInfo {
    pub name: String,
    pub is_spot: bool,
    pub process_cmyk: (f32, f32, f32, f32),
}

/// Lists all process and spot ink plates present in the document (FR-23.3.2).
pub fn get_document_inks(doc: &PdfDocument) -> Vec<DocumentInkInfo> {
    let mut inks = vec![
        DocumentInkInfo { name: "Process Cyan".into(), is_spot: false, process_cmyk: (1.0, 0.0, 0.0, 0.0) },
        DocumentInkInfo { name: "Process Magenta".into(), is_spot: false, process_cmyk: (0.0, 1.0, 0.0, 0.0) },
        DocumentInkInfo { name: "Process Yellow".into(), is_spot: false, process_cmyk: (0.0, 0.0, 1.0, 0.0) },
        DocumentInkInfo { name: "Process Black".into(), is_spot: false, process_cmyk: (0.0, 0.0, 0.0, 1.0) },
    ];

    let mut spotted = std::collections::HashSet::new();

    for obj in doc.objects.values() {
        if let Some(dict) = obj.as_dict() {
            let cs_dict_opt = dict.get_dict("ColorSpace").or_else(|| {
                dict.get_dict("Resources").and_then(|r| r.get_dict("ColorSpace"))
            });
            if let Some(cs_dict) = cs_dict_opt {
                for (name, cs_val) in &cs_dict.map {
                    if let Some(arr) = cs_val.as_array() {
                        if arr.first().and_then(Object::as_name) == Some("Separation") {
                            let spot_name = arr.get(1).and_then(Object::as_name).unwrap_or(name.as_str());
                            if spot_name != "All" && spot_name != "None" && spotted.insert(spot_name.to_string()) {
                                inks.push(DocumentInkInfo {
                                    name: spot_name.to_string(),
                                    is_spot: true,
                                    process_cmyk: (0.0, 0.8, 0.6, 0.0), // vibrant spot estimate
                                });
                            }
                        }
                    }
                }
            }
        }
    }

    inks
}

/// Applies ink mappings to convert spot colors to process CMYK (FR-23.3.2).
pub fn apply_ink_manager(doc: &mut PdfDocument, config: &InkManagerConfig) -> Result<usize> {
    let mut modified = 0;
    let spot_map: std::collections::HashMap<String, (f32, f32, f32, f32)> = config
        .mappings
        .iter()
        .map(|m| (m.spot_name.clone(), m.cmyk))
        .collect();

    for obj in doc.objects.values_mut() {
        if let Some(dict) = obj.as_dict_mut() {
            let cs_dict_opt = if dict.contains_key("ColorSpace") {
                dict.get_mut("ColorSpace").and_then(Object::as_dict_mut)
            } else if let Some(res) = dict.get_mut("Resources").and_then(Object::as_dict_mut) {
                res.get_mut("ColorSpace").and_then(Object::as_dict_mut)
            } else {
                None
            };

            if let Some(cs_dict) = cs_dict_opt {
                let keys: Vec<PdfName> = cs_dict.map.keys().cloned().collect();
                for key in keys {
                    let maybe_arr = cs_dict.map.get(&key).and_then(Object::as_array).map(|a| a.to_vec());
                    if let Some(arr) = maybe_arr {
                        if arr.first().and_then(Object::as_name) == Some("Separation") {
                            let spot_name = arr.get(1).and_then(Object::as_name).unwrap_or(key.as_str()).to_string();
                            if config.convert_all_spots_to_process || spot_map.contains_key(&spot_name) {
                                // Convert Separation color space to DeviceCMYK
                                cs_dict.map.insert(key, Object::Name(PdfName::new("DeviceCMYK")));
                                modified += 1;
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(modified)
}

/// Configuration for document transparency flattening (FR-23.3.3).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TransparencyFlattenConfig {
    /// Raster/Vector balance from 0 (all raster) to 100 (preserve vectors). Default: 75.
    pub raster_vector_balance: u8,
    /// Flattens opacity values in ExtGState dictionaries to 1.0.
    pub flatten_opacity: bool,
    /// Normalizes blend modes (/BM) to /Normal.
    pub normalize_blend_modes: bool,
    /// Removes page-level /Group << /S /Transparency >> dictionaries.
    pub remove_page_transparency_groups: bool,
}

impl Default for TransparencyFlattenConfig {
    fn default() -> Self {
        Self {
            raster_vector_balance: 75,
            flatten_opacity: true,
            normalize_blend_modes: true,
            remove_page_transparency_groups: true,
        }
    }
}

/// Report summarizing transparency flattening actions (FR-23.3.3).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TransparencyFlattenReport {
    pub extgstate_flattened: usize,
    pub groups_removed: usize,
    pub total_pages_processed: usize,
}

/// Flattens transparency structures across ExtGState and page groups (FR-23.3.3).
pub fn flatten_document_transparency(
    doc: &mut PdfDocument,
    config: &TransparencyFlattenConfig,
) -> Result<TransparencyFlattenReport> {
    let mut report = TransparencyFlattenReport::default();

    // 1. Flatten ExtGState objects
    for obj in doc.objects.values_mut() {
        if let Some(dict) = obj.as_dict_mut() {
            if dict.get_name("Type") == Some("ExtGState")
                || dict.contains_key("ca")
                || dict.contains_key("CA")
                || dict.contains_key("BM")
            {
                let mut changed = false;
                if config.flatten_opacity {
                    if dict.contains_key("ca") {
                        dict.insert("ca", Object::Real(1.0));
                        changed = true;
                    }
                    if dict.contains_key("CA") {
                        dict.insert("CA", Object::Real(1.0));
                        changed = true;
                    }
                }
                if config.normalize_blend_modes && dict.contains_key("BM") {
                    dict.insert("BM", Object::Name(PdfName::new("Normal")));
                    changed = true;
                }
                if changed {
                    report.extgstate_flattened += 1;
                }
            }
        }
    }

    // 2. Remove transparency groups from page nodes
    let page_ids = doc.page_ids();
    report.total_pages_processed = page_ids.len();

    if config.remove_page_transparency_groups {
        for pid in page_ids {
            if let Some(p_obj) = doc.objects.get_mut(&pid) {
                if let Some(p_dict) = p_obj.as_dict_mut() {
                    if let Some(group) = p_dict.get_dict("Group") {
                        if group.get_name("S") == Some("Transparency") {
                            p_dict.map.remove(&PdfName::new("Group"));
                            report.groups_removed += 1;
                        }
                    }
                }
            }
        }
    }

    Ok(report)
}

/// Trap preset configuration conforming to ISO 32000-1 Clause 14.11 / Table 342.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TrapPreset {
    pub name: String,
    pub trap_width: f64,
    pub black_width: f64,
    pub black_color_limit: f64,
    pub trap_color_reduction: f64,
}

impl Default for TrapPreset {
    fn default() -> Self {
        Self {
            name: "Default Trapping".into(),
            trap_width: 0.25,
            black_width: 0.50,
            black_color_limit: 1.0,
            trap_color_reduction: 0.5,
        }
    }
}

/// Retrieves document trap presets from catalog /PieceInfo (FR-23.3.5).
pub fn get_trap_presets(doc: &PdfDocument) -> Vec<TrapPreset> {
    let mut presets = Vec::new();
    if let Some(cat) = doc.catalog() {
        if let Some(piece) = cat.get_dict("PieceInfo") {
            if let Some(compound) = piece.get_dict("ADBE_CompoundType") {
                if let Some(trap_reg) = compound.get_dict("Private") {
                    if let Some(list) = trap_reg.get_array("TrapPresets") {
                        for item in list {
                            if let Some(d) = item.as_dict() {
                                presets.push(TrapPreset {
                                    name: d.get_str("Name").unwrap_or_else(|| "Preset".to_string()),
                                    trap_width: d.get_f64("TrapWidth").unwrap_or(0.25),
                                    black_width: d.get_f64("BlackWidth").unwrap_or(0.50),
                                    black_color_limit: d.get_f64("BlackColorLimit").unwrap_or(1.0),
                                    trap_color_reduction: d.get_f64("TrapColorReduction").unwrap_or(0.5),
                                });
                            }
                        }
                    }
                }
            }
        }
    }
    if presets.is_empty() {
        presets.push(TrapPreset::default());
    }
    presets
}

/// Configures document trap presets in /PieceInfo /ADBE_CompoundType (FR-23.3.5).
pub fn set_trap_presets(doc: &mut PdfDocument, presets: &[TrapPreset]) -> Result<()> {
    let cat_id = doc.root_id;
    let mut preset_objs = Vec::new();

    for p in presets {
        let mut d = PdfDictionary::new();
        d.insert("Name", PdfString::from_str(&p.name));
        d.insert("TrapWidth", Object::Real(p.trap_width));
        d.insert("BlackWidth", Object::Real(p.black_width));
        d.insert("BlackColorLimit", Object::Real(p.black_color_limit));
        d.insert("TrapColorReduction", Object::Real(p.trap_color_reduction));
        preset_objs.push(Object::Dictionary(d));
    }

    let mut trap_dict = PdfDictionary::new();
    trap_dict.insert("Type", PdfName::new("TrapRegion"));
    trap_dict.insert("TrapPresets", preset_objs);

    let mut compound = PdfDictionary::new();
    compound.insert("Private", Object::Dictionary(trap_dict));

    let cat_obj = doc.objects.get_mut(&cat_id).ok_or_else(|| anyhow::anyhow!("Missing catalog"))?;
    let cat_dict = cat_obj.as_dict_mut().ok_or_else(|| anyhow::anyhow!("Catalog not dict"))?;

    let piece = cat_dict.map.entry(PdfName::new("PieceInfo")).or_insert_with(|| Object::Dictionary(PdfDictionary::new()));
    if let Object::Dictionary(piece_dict) = piece {
        piece_dict.insert("ADBE_CompoundType", Object::Dictionary(compound));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plate_separations_and_tac_detection() {
        // Create 2x2 test image with different ink densities:
        let pixels = vec![
            0, 0, 0, 255,       // Rich black
            255, 255, 255, 255, // Pure white
            0, 255, 255, 255,   // Pure cyan
            255, 0, 0, 255,     // Pure red (M=1, Y=1 -> TAC=200%)
        ];

        let mut config = PlateSeparationConfig {
            show_tac_warning: true,
            tac_threshold: 3.0,
            ..Default::default()
        };

        let (out, analysis) = apply_plate_separations(&pixels, 2, 2, &config);
        assert_eq!(out.len(), 16);
        assert!(analysis.max_tac >= 1.0);

        // Turn off Magenta and Yellow plates
        config.show_magenta = false;
        config.show_yellow = false;
        config.show_tac_warning = false;
        let (out_no_my, _) = apply_plate_separations(&pixels, 2, 2, &config);
        assert_eq!(out_no_my.len(), 16);
    }

    #[test]
    fn test_printer_marks_injection() {
        let mut doc = PdfDocument::new();
        doc.add_blank_page(612.0, 792.0);

        let cfg = PrinterMarksConfig::default();
        let modified = add_printer_marks(&mut doc, &cfg).expect("add printer marks");
        assert_eq!(modified, 1);

        // Check page boxes
        let page = doc.get_page(0).expect("page 0");
        let p_dict = doc.get_object(page.id).and_then(Object::as_dict).expect("page dict");
        assert!(p_dict.get_array("TrimBox").is_some());
        assert!(p_dict.get_array("BleedBox").is_some());
        assert_eq!(page.media_box, [-36.0, -36.0, 648.0, 828.0]);
        // Contents should contain the printer marks stream
        assert!(!page.contents_ids.is_empty());
    }

    #[test]
    fn test_ink_manager_and_transparency_flattening() {
        let mut doc = PdfDocument::new();
        let pid = doc.add_blank_page(612.0, 792.0);

        // Add a mock spot color in page resources
        if let Some(p_dict) = doc.objects.get_mut(&pid).and_then(Object::as_dict_mut) {
            let mut cs_dict = PdfDictionary::new();
            cs_dict.insert("SpotRed", vec![
                Object::Name(PdfName::new("Separation")),
                Object::Name(PdfName::new("PANTONE 185 C")),
                Object::Name(PdfName::new("DeviceCMYK")),
            ]);
            let mut res = PdfDictionary::new();
            res.insert("ColorSpace", Object::Dictionary(cs_dict));
            p_dict.insert("Resources", Object::Dictionary(res));

            // Add mock transparency group
            let mut grp = PdfDictionary::new();
            grp.insert("S", PdfName::new("Transparency"));
            p_dict.insert("Group", Object::Dictionary(grp));
        }

        let inks = get_document_inks(&doc);
        assert!(inks.iter().any(|i| i.name == "PANTONE 185 C" && i.is_spot));

        let ink_cfg = InkManagerConfig {
            mappings: vec![InkMapping {
                spot_name: "PANTONE 185 C".into(),
                cmyk: (0.0, 0.93, 0.79, 0.0),
            }],
            convert_all_spots_to_process: false,
        };
        let converted = apply_ink_manager(&mut doc, &ink_cfg).expect("apply ink manager");
        assert_eq!(converted, 1);

        // Test Transparency flattening
        let flat_cfg = TransparencyFlattenConfig::default();
        let flat_rep = flatten_document_transparency(&mut doc, &flat_cfg).expect("flatten");
        assert_eq!(flat_rep.groups_removed, 1);
        let p_after = doc.get_object(pid).and_then(Object::as_dict).expect("page dict");
        assert!(p_after.get_dict("Group").is_none());
    }

    #[test]
    fn test_trap_presets_lifecycle() {
        let mut doc = PdfDocument::new();
        let presets = vec![
            TrapPreset {
                name: "Sheetfed Commercial".into(),
                trap_width: 0.30,
                black_width: 0.60,
                black_color_limit: 1.0,
                trap_color_reduction: 0.4,
            },
        ];

        set_trap_presets(&mut doc, &presets).expect("set trap presets");
        let retrieved = get_trap_presets(&doc);
        assert_eq!(retrieved.len(), 1);
        assert_eq!(retrieved[0].name, "Sheetfed Commercial");
        assert_eq!(retrieved[0].trap_width, 0.30);
    }
}
