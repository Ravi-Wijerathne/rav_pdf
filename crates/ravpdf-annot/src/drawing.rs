//! Drawing and Shape Annotations conforming to ISO 32000-1 Section 12.5.6.
//!
//! Traceability:
//! - FR-09.1: Freehand drawing (pencil/ink tool) with configurable stroke width, color, and opacity.
//! - FR-09.2: Eraser tool removing pencil/ink strokes.
//! - FR-09.3: Lines with configurable start/end arrowheads, width, color, dash pattern.
//! - FR-09.4: Arrows (single and double-headed).
//! - FR-09.5: Rectangles and squares with fill, border color, width, style.
//! - FR-09.6: Ellipses and circles with fill and border.
//! - FR-09.7: Polygons (multi-point closed shapes).
//! - FR-09.8: Polylines (multi-segment open lines).
//! - FR-09.9: Cloud/cloudy border annotations.
//! - FR-09.10: Connected line tools with automatic node snapping.
//! - FR-09.11: Opacity for all drawing annotations.
//! - FR-09.12: Locking drawing annotations.

use ravpdf_core::document::PdfDocument;
use ravpdf_core::error::{PdfError, Result};
use ravpdf_core::object::{Object, ObjectId, PdfDictionary, PdfName, PdfStream, PdfString};

/// Line ending styles conforming to ISO 32000-1 Table 176.
///
/// Traceability: FR-09.3, FR-09.4.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LineEndingStyle {
    #[default]
    None,
    Square,
    Circle,
    Diamond,
    OpenArrow,
    ClosedArrow,
    Butt,
    ROpenArrow,
    RClosedArrow,
    Slash,
}

impl LineEndingStyle {
    pub fn to_pdf_name(&self) -> &'static str {
        match self {
            LineEndingStyle::None => "None",
            LineEndingStyle::Square => "Square",
            LineEndingStyle::Circle => "Circle",
            LineEndingStyle::Diamond => "Diamond",
            LineEndingStyle::OpenArrow => "OpenArrow",
            LineEndingStyle::ClosedArrow => "ClosedArrow",
            LineEndingStyle::Butt => "Butt",
            LineEndingStyle::ROpenArrow => "ROpenArrow",
            LineEndingStyle::RClosedArrow => "RClosedArrow",
            LineEndingStyle::Slash => "Slash",
        }
    }

    pub fn from_pdf_name(name: &str) -> Self {
        match name {
            "Square" => LineEndingStyle::Square,
            "Circle" => LineEndingStyle::Circle,
            "Diamond" => LineEndingStyle::Diamond,
            "OpenArrow" => LineEndingStyle::OpenArrow,
            "ClosedArrow" => LineEndingStyle::ClosedArrow,
            "Butt" => LineEndingStyle::Butt,
            "ROpenArrow" => LineEndingStyle::ROpenArrow,
            "RClosedArrow" => LineEndingStyle::RClosedArrow,
            "Slash" => LineEndingStyle::Slash,
            _ => LineEndingStyle::None,
        }
    }
}

/// Border dash style for shapes and lines.
///
/// Traceability: FR-09.3, FR-09.5.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum BorderStyle {
    #[default]
    Solid,
    Dashed(Vec<f64>),
    Beveled,
    Inset,
    Underline,
}

/// Border effect conforming to ISO 32000-1 Section 12.5.6.7 Table 176 (/BE dictionary).
///
/// Traceability: FR-09.9.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum BorderEffect {
    #[default]
    None,
    /// Cloudy / Scalloped border with given intensity (1.0 or 2.0).
    Cloudy { intensity: f64 },
}

/// Types of geometric vector shapes.
///
/// Traceability: FR-09.3, FR-09.4, FR-09.5, FR-09.6, FR-09.7, FR-09.8.
#[derive(Debug, Clone, PartialEq)]
pub enum ShapeType {
    /// Rectangle or Square with coordinates `[x, y, w, h]` (FR-09.5).
    Rectangle,
    /// Ellipse or Circle within bounding box `[x, y, w, h]` (FR-09.6).
    Circle,
    /// Single or Double-headed Line / Arrow with start/end points (FR-09.3, FR-09.4).
    Line {
        start: [f64; 2],
        end: [f64; 2],
        start_style: LineEndingStyle,
        end_style: LineEndingStyle,
    },
    /// Closed polygon with 3 or more vertices (FR-09.7).
    Polygon { vertices: Vec<[f64; 2]> },
    /// Open polyline with 2 or more vertices (FR-09.8).
    PolyLine {
        vertices: Vec<[f64; 2]>,
        start_style: LineEndingStyle,
        end_style: LineEndingStyle,
    },
}

/// Configuration options for creating shape annotations.
///
/// Traceability: FR-09.3 - FR-09.9, FR-09.11, FR-09.12.
#[derive(Debug, Clone)]
pub struct ShapeAnnotationConfig {
    pub shape_type: ShapeType,
    pub rect: [f64; 4],
    pub stroke_color: Option<(f32, f32, f32)>,
    pub fill_color: Option<(f32, f32, f32)>,
    pub stroke_width: f64,
    pub border_style: BorderStyle,
    pub border_effect: BorderEffect,
    pub opacity: f32,
    pub author: Option<String>,
    pub locked: bool,
    pub popup_note: Option<String>,
}

impl Default for ShapeAnnotationConfig {
    fn default() -> Self {
        Self {
            shape_type: ShapeType::Rectangle,
            rect: [100.0, 100.0, 200.0, 100.0],
            stroke_color: Some((0.15, 0.15, 0.15)),
            fill_color: None,
            stroke_width: 2.0,
            border_style: BorderStyle::Solid,
            border_effect: BorderEffect::None,
            opacity: 1.0,
            author: None,
            locked: false,
            popup_note: None,
        }
    }
}

/// Configuration options for freehand ink drawings.
///
/// Traceability: FR-09.1, FR-09.11, FR-09.12.
#[derive(Debug, Clone)]
pub struct InkAnnotationConfig {
    /// List of strokes, where each stroke is a sequence of `[x, y]` points in PDF space.
    pub strokes: Vec<Vec<[f64; 2]>>,
    pub stroke_width: f64,
    pub color: (f32, f32, f32),
    pub opacity: f32,
    /// Apply smoothing algorithm to strokes.
    pub smooth: bool,
    pub author: Option<String>,
    pub locked: bool,
    pub popup_note: Option<String>,
}

impl Default for InkAnnotationConfig {
    fn default() -> Self {
        Self {
            strokes: Vec::new(),
            stroke_width: 2.0,
            color: (0.1, 0.4, 0.8),
            opacity: 1.0,
            smooth: true,
            author: None,
            locked: false,
            popup_note: None,
        }
    }
}

/// Creates a new freehand ink annotation conforming to ISO 32000-1 Clause 12.5.6.13.
///
/// Traceability: FR-09.1, FR-09.11, FR-09.12.
pub fn add_ink_annotation(
    doc: &mut PdfDocument,
    page_index: usize,
    config: &InkAnnotationConfig,
) -> Result<ObjectId> {
    if config.strokes.is_empty() {
        return Err(PdfError::InvalidStructure(
            "Ink strokes list is empty".into(),
        ));
    }

    let page_ids = doc.page_ids();
    let page_id = *page_ids
        .get(page_index)
        .ok_or_else(|| PdfError::InvalidStructure(format!("Page {} out of range", page_index)))?;

    // Compute bounding box from all points
    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut max_y = f64::NEG_INFINITY;

    for stroke in &config.strokes {
        for &[x, y] in stroke {
            if x < min_x {
                min_x = x;
            }
            if y < min_y {
                min_y = y;
            }
            if x > max_x {
                max_x = x;
            }
            if y > max_y {
                max_y = y;
            }
        }
    }

    let pad = (config.stroke_width * 2.0).max(4.0);
    let rect = [min_x - pad, min_y - pad, max_x + pad, max_y + pad];

    // Build ISO 32000-1 Clause 12.5.5 Appearance Stream (/AP /N)
    let ap_stream_bytes = generate_ink_appearance_stream(config, &rect);
    let ap_obj_id = doc.next_object_id();
    let mut ap_dict = PdfDictionary::new();
    ap_dict.insert("Type", PdfName::new("XObject"));
    ap_dict.insert("Subtype", PdfName::new("Form"));
    ap_dict.insert("FormType", Object::Integer(1));
    let w = (rect[2] - rect[0]).abs();
    let h = (rect[3] - rect[1]).abs();
    ap_dict.insert(
        "BBox",
        vec![
            Object::Real(0.0),
            Object::Real(0.0),
            Object::Real(w),
            Object::Real(h),
        ],
    );

    let mut resources = PdfDictionary::new();
    let gs_name = "GS0";
    let mut ext_gstate = PdfDictionary::new();
    let mut gs_entry = PdfDictionary::new();
    gs_entry.insert("ca", Object::Real(config.opacity as f64));
    gs_entry.insert("CA", Object::Real(config.opacity as f64));
    ext_gstate.insert(gs_name, Object::Dictionary(gs_entry));
    resources.insert("ExtGState", Object::Dictionary(ext_gstate));
    ap_dict.insert("Resources", Object::Dictionary(resources));

    let mut ap_stream = PdfStream::with_plain_data(ap_dict, ap_stream_bytes);
    let _ = ap_stream.compress_flate();
    doc.objects.insert(ap_obj_id, Object::Stream(ap_stream));

    // Build annotation dictionary
    let annot_id = doc.next_object_id();
    let mut annot_dict = PdfDictionary::new();
    annot_dict.insert("Type", PdfName::new("Annot"));
    annot_dict.insert("Subtype", PdfName::new("Ink"));
    annot_dict.insert(
        "Rect",
        vec![
            Object::Real(rect[0]),
            Object::Real(rect[1]),
            Object::Real(rect[2]),
            Object::Real(rect[3]),
        ],
    );

    // /InkList: array of arrays of coordinates
    let mut ink_list_arr = Vec::new();
    for stroke in &config.strokes {
        let mut stroke_arr = Vec::new();
        for &[x, y] in stroke {
            stroke_arr.push(Object::Real(x));
            stroke_arr.push(Object::Real(y));
        }
        ink_list_arr.push(Object::Array(stroke_arr));
    }
    annot_dict.insert("InkList", Object::Array(ink_list_arr));

    // Color and border style
    annot_dict.insert(
        "C",
        vec![
            Object::Real(config.color.0 as f64),
            Object::Real(config.color.1 as f64),
            Object::Real(config.color.2 as f64),
        ],
    );

    let mut bs_dict = PdfDictionary::new();
    bs_dict.insert("W", Object::Real(config.stroke_width));
    bs_dict.insert("S", PdfName::new("S"));
    annot_dict.insert("BS", Object::Dictionary(bs_dict));

    if (config.opacity - 1.0).abs() > 1e-4 {
        annot_dict.insert("CA", Object::Real(config.opacity as f64));
    }

    if let Some(ref author) = config.author {
        annot_dict.insert("T", PdfString::from_str(author));
    }
    if let Some(ref note) = config.popup_note {
        annot_dict.insert("Contents", PdfString::from_str(note));
    }

    // Annotation Flags: Bit 3 (4) = Print; Bit 8 (128) = Locked (FR-09.12)
    let mut flags: i64 = 4;
    if config.locked {
        flags |= 128;
    }
    annot_dict.insert("F", Object::Integer(flags));

    // Register /AP
    let mut ap_ref_dict = PdfDictionary::new();
    ap_ref_dict.insert("N", Object::Reference(ap_obj_id));
    annot_dict.insert("AP", Object::Dictionary(ap_ref_dict));

    doc.objects.insert(annot_id, Object::Dictionary(annot_dict));

    // Attach to page
    attach_annot_to_page(doc, page_id, annot_id)?;

    Ok(annot_id)
}

/// Erases ink strokes intersecting an eraser circle with radius at `erase_point`.
///
/// Conforms to ISO 32000-1 ink stroke management.
/// Returns Ok(true) if strokes were modified or removed, Ok(false) if unaffected.
///
/// Traceability: FR-09.2.
pub fn erase_ink_strokes(
    doc: &mut PdfDocument,
    page_index: usize,
    annot_id: ObjectId,
    erase_point: [f64; 2],
    radius: f64,
) -> Result<bool> {
    let annot_dict = match doc.objects.get_mut(&annot_id) {
        Some(Object::Dictionary(d)) => d,
        _ => {
            return Err(PdfError::ObjectNotFound(
                annot_id.number,
                annot_id.generation,
            ))
        }
    };

    if annot_dict.get_name("Subtype") != Some("Ink") {
        return Ok(false);
    }

    let ink_list = match annot_dict.get("InkList") {
        Some(Object::Array(arr)) => arr.clone(),
        _ => return Ok(false),
    };

    let mut remaining_strokes: Vec<Vec<[f64; 2]>> = Vec::new();
    let mut any_erased = false;

    for stroke_obj in ink_list {
        if let Object::Array(coords) = stroke_obj {
            let mut stroke_pts = Vec::new();
            let mut i = 0;
            while i + 1 < coords.len() {
                if let (Some(x), Some(y)) = (coords[i].as_f64(), coords[i + 1].as_f64()) {
                    stroke_pts.push([x, y]);
                }
                i += 2;
            }

            // Check if eraser intersects any line segment of this stroke
            let mut stroke_hit = false;
            for j in 0..stroke_pts.len() {
                let p = stroke_pts[j];
                let d = ((p[0] - erase_point[0]).powi(2) + (p[1] - erase_point[1]).powi(2)).sqrt();
                if d <= radius {
                    stroke_hit = true;
                    break;
                }
                if j + 1 < stroke_pts.len() {
                    let next_p = stroke_pts[j + 1];
                    let seg_d = point_to_segment_distance(erase_point, p, next_p);
                    if seg_d <= radius {
                        stroke_hit = true;
                        break;
                    }
                }
            }

            if stroke_hit {
                any_erased = true;
            } else {
                remaining_strokes.push(stroke_pts);
            }
        }
    }

    if !any_erased {
        return Ok(false);
    }

    let page_ids = doc.page_ids();
    let page_id = *page_ids
        .get(page_index)
        .ok_or_else(|| PdfError::InvalidStructure(format!("Page {} out of range", page_index)))?;

    if remaining_strokes.is_empty() {
        // All strokes erased, remove the entire annotation
        remove_annot_from_page(doc, page_id, annot_id)?;
        doc.objects.remove(&annot_id);
        return Ok(true);
    }

    // Update remaining strokes in annotation dictionary
    let stroke_width = doc
        .objects
        .get(&annot_id)
        .and_then(Object::as_dict)
        .and_then(|d| d.get_dict("BS"))
        .and_then(|bs| bs.get_f64("W"))
        .unwrap_or(2.0);

    let color = doc
        .objects
        .get(&annot_id)
        .and_then(Object::as_dict)
        .and_then(|d| d.get_array("C"))
        .map(|c| {
            (
                c.first().and_then(Object::as_f64).unwrap_or(0.0) as f32,
                c.get(1).and_then(Object::as_f64).unwrap_or(0.0) as f32,
                c.get(2).and_then(Object::as_f64).unwrap_or(0.0) as f32,
            )
        })
        .unwrap_or((0.0, 0.0, 0.0));

    let opacity = doc
        .objects
        .get(&annot_id)
        .and_then(Object::as_dict)
        .and_then(|d| d.get_f64("CA"))
        .unwrap_or(1.0) as f32;

    let config = InkAnnotationConfig {
        strokes: remaining_strokes.clone(),
        stroke_width,
        color,
        opacity,
        smooth: true,
        author: None,
        locked: false,
        popup_note: None,
    };

    // Recompute bounding box
    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut max_y = f64::NEG_INFINITY;

    for stroke in &remaining_strokes {
        for &[x, y] in stroke {
            if x < min_x {
                min_x = x;
            }
            if y < min_y {
                min_y = y;
            }
            if x > max_x {
                max_x = x;
            }
            if y > max_y {
                max_y = y;
            }
        }
    }
    let pad = (stroke_width * 2.0).max(4.0);
    let rect = [min_x - pad, min_y - pad, max_x + pad, max_y + pad];

    // Regenerate appearance stream
    let ap_bytes = generate_ink_appearance_stream(&config, &rect);

    if let Some(Object::Dictionary(d)) = doc.objects.get_mut(&annot_id) {
        d.insert(
            "Rect",
            vec![
                Object::Real(rect[0]),
                Object::Real(rect[1]),
                Object::Real(rect[2]),
                Object::Real(rect[3]),
            ],
        );

        let mut ink_list_arr = Vec::new();
        for stroke in &remaining_strokes {
            let mut stroke_arr = Vec::new();
            for &[x, y] in stroke {
                stroke_arr.push(Object::Real(x));
                stroke_arr.push(Object::Real(y));
            }
            ink_list_arr.push(Object::Array(stroke_arr));
        }
        d.insert("InkList", Object::Array(ink_list_arr));

        if let Some(ap_dict) = d.get_dict("AP") {
            if let Some(Object::Reference(ap_id)) = ap_dict.get("N") {
                let ap_id = *ap_id;
                if let Some(Object::Stream(s)) = doc.objects.get_mut(&ap_id) {
                    *s = PdfStream::with_plain_data(s.dict.clone(), ap_bytes);
                    let _ = s.compress_flate();
                    let w = (rect[2] - rect[0]).abs();
                    let h = (rect[3] - rect[1]).abs();
                    s.dict.insert(
                        "BBox",
                        vec![
                            Object::Real(0.0),
                            Object::Real(0.0),
                            Object::Real(w),
                            Object::Real(h),
                        ],
                    );
                }
            }
        }
    }

    Ok(true)
}

/// Creates a new shape annotation (Line, Arrow, Rectangle, Circle, Polygon, Polyline, Cloudy).
///
/// Conforms strictly to ISO 32000-1 Section 12.5.6.
///
/// Traceability: FR-09.3 - FR-09.9, FR-09.11, FR-09.12.
pub fn add_shape_annotation(
    doc: &mut PdfDocument,
    page_index: usize,
    config: &ShapeAnnotationConfig,
) -> Result<ObjectId> {
    let page_ids = doc.page_ids();
    let page_id = *page_ids
        .get(page_index)
        .ok_or_else(|| PdfError::InvalidStructure(format!("Page {} out of range", page_index)))?;

    let (subtype, effective_rect) = match &config.shape_type {
        ShapeType::Rectangle => ("Square", config.rect),
        ShapeType::Circle => ("Circle", config.rect),
        ShapeType::Line { start, end, .. } => {
            let x1 = start[0].min(end[0]) - config.stroke_width * 4.0;
            let y1 = start[1].min(end[1]) - config.stroke_width * 4.0;
            let x2 = start[0].max(end[0]) + config.stroke_width * 4.0;
            let y2 = start[1].max(end[1]) + config.stroke_width * 4.0;
            ("Line", [x1, y1, x2, y2])
        }
        ShapeType::Polygon { vertices } => {
            let (x1, y1, x2, y2) = bounds_of_points(vertices, config.stroke_width * 2.0);
            ("Polygon", [x1, y1, x2, y2])
        }
        ShapeType::PolyLine { vertices, .. } => {
            let (x1, y1, x2, y2) = bounds_of_points(vertices, config.stroke_width * 4.0);
            ("PolyLine", [x1, y1, x2, y2])
        }
    };

    // Build ISO appearance stream
    let ap_bytes = generate_shape_appearance_stream(config, &effective_rect);
    let ap_obj_id = doc.next_object_id();
    let mut ap_dict = PdfDictionary::new();
    ap_dict.insert("Type", PdfName::new("XObject"));
    ap_dict.insert("Subtype", PdfName::new("Form"));
    ap_dict.insert("FormType", Object::Integer(1));
    let w = (effective_rect[2] - effective_rect[0]).abs();
    let h = (effective_rect[3] - effective_rect[1]).abs();
    ap_dict.insert(
        "BBox",
        vec![
            Object::Real(0.0),
            Object::Real(0.0),
            Object::Real(w),
            Object::Real(h),
        ],
    );

    let mut resources = PdfDictionary::new();
    let gs_name = "GS0";
    let mut ext_gstate = PdfDictionary::new();
    let mut gs_entry = PdfDictionary::new();
    gs_entry.insert("ca", Object::Real(config.opacity as f64));
    gs_entry.insert("CA", Object::Real(config.opacity as f64));
    ext_gstate.insert(gs_name, Object::Dictionary(gs_entry));
    resources.insert("ExtGState", Object::Dictionary(ext_gstate));
    ap_dict.insert("Resources", Object::Dictionary(resources));

    let mut ap_stream = PdfStream::with_plain_data(ap_dict, ap_bytes);
    let _ = ap_stream.compress_flate();
    doc.objects.insert(ap_obj_id, Object::Stream(ap_stream));

    // Build annotation dictionary
    let annot_id = doc.next_object_id();
    let mut annot_dict = PdfDictionary::new();
    annot_dict.insert("Type", PdfName::new("Annot"));
    annot_dict.insert("Subtype", PdfName::new(subtype));
    annot_dict.insert(
        "Rect",
        vec![
            Object::Real(effective_rect[0]),
            Object::Real(effective_rect[1]),
            Object::Real(effective_rect[2]),
            Object::Real(effective_rect[3]),
        ],
    );

    // Stroke Color /C
    if let Some((r, g, b)) = config.stroke_color {
        annot_dict.insert(
            "C",
            vec![
                Object::Real(r as f64),
                Object::Real(g as f64),
                Object::Real(b as f64),
            ],
        );
    }

    // Interior Color /IC
    if let Some((r, g, b)) = config.fill_color {
        annot_dict.insert(
            "IC",
            vec![
                Object::Real(r as f64),
                Object::Real(g as f64),
                Object::Real(b as f64),
            ],
        );
    }

    // Border style /BS
    let mut bs_dict = PdfDictionary::new();
    bs_dict.insert("W", Object::Real(config.stroke_width));
    match &config.border_style {
        BorderStyle::Solid => {
            bs_dict.insert("S", PdfName::new("S"));
        }
        BorderStyle::Dashed(dash) => {
            bs_dict.insert("S", PdfName::new("D"));
            let d_arr = dash.iter().map(|&v| Object::Real(v)).collect();
            bs_dict.insert("D", Object::Array(d_arr));
        }
        BorderStyle::Beveled => {
            bs_dict.insert("S", PdfName::new("B"));
        }
        BorderStyle::Inset => {
            bs_dict.insert("S", PdfName::new("I"));
        }
        BorderStyle::Underline => {
            bs_dict.insert("S", PdfName::new("U"));
        }
    }
    annot_dict.insert("BS", Object::Dictionary(bs_dict));

    // Border effect /BE (Cloudy border FR-09.9)
    if let BorderEffect::Cloudy { intensity } = config.border_effect {
        let mut be_dict = PdfDictionary::new();
        be_dict.insert("S", PdfName::new("C"));
        be_dict.insert("I", Object::Real(intensity));
        annot_dict.insert("BE", Object::Dictionary(be_dict));
    }

    // Subtype-specific keys
    match &config.shape_type {
        ShapeType::Line {
            start,
            end,
            start_style,
            end_style,
        } => {
            annot_dict.insert(
                "L",
                vec![
                    Object::Real(start[0]),
                    Object::Real(start[1]),
                    Object::Real(end[0]),
                    Object::Real(end[1]),
                ],
            );
            annot_dict.insert(
                "LE",
                vec![
                    Object::Name(PdfName::new(start_style.to_pdf_name())),
                    Object::Name(PdfName::new(end_style.to_pdf_name())),
                ],
            );
        }
        ShapeType::Polygon { vertices } => {
            let mut vert_arr = Vec::new();
            for pt in vertices {
                vert_arr.push(Object::Real(pt[0]));
                vert_arr.push(Object::Real(pt[1]));
            }
            annot_dict.insert("Vertices", Object::Array(vert_arr));
        }
        ShapeType::PolyLine {
            vertices,
            start_style,
            end_style,
        } => {
            let mut vert_arr = Vec::new();
            for pt in vertices {
                vert_arr.push(Object::Real(pt[0]));
                vert_arr.push(Object::Real(pt[1]));
            }
            annot_dict.insert("Vertices", Object::Array(vert_arr));
            annot_dict.insert(
                "LE",
                vec![
                    Object::Name(PdfName::new(start_style.to_pdf_name())),
                    Object::Name(PdfName::new(end_style.to_pdf_name())),
                ],
            );
        }
        _ => {}
    }

    if (config.opacity - 1.0).abs() > 1e-4 {
        annot_dict.insert("CA", Object::Real(config.opacity as f64));
    }

    if let Some(ref author) = config.author {
        annot_dict.insert("T", PdfString::from_str(author));
    }
    if let Some(ref note) = config.popup_note {
        annot_dict.insert("Contents", PdfString::from_str(note));
    }

    // Flags: Bit 3 = Print; Bit 8 = Locked (FR-09.12)
    let mut flags: i64 = 4;
    if config.locked {
        flags |= 128;
    }
    annot_dict.insert("F", Object::Integer(flags));

    // Register /AP
    let mut ap_ref_dict = PdfDictionary::new();
    ap_ref_dict.insert("N", Object::Reference(ap_obj_id));
    annot_dict.insert("AP", Object::Dictionary(ap_ref_dict));

    doc.objects.insert(annot_id, Object::Dictionary(annot_dict));

    // Attach to page
    attach_annot_to_page(doc, page_id, annot_id)?;

    Ok(annot_id)
}

/// Locks or unlocks an annotation to prevent accidental modification (FR-09.12).
///
/// Sets or clears bit 8 (128) in the /F annotation flags according to ISO 32000-1 Table 165.
pub fn set_annotation_locked(
    doc: &mut PdfDocument,
    annot_id: ObjectId,
    locked: bool,
) -> Result<()> {
    let annot_dict = match doc.objects.get_mut(&annot_id) {
        Some(Object::Dictionary(d)) => d,
        _ => {
            return Err(PdfError::ObjectNotFound(
                annot_id.number,
                annot_id.generation,
            ))
        }
    };

    let mut flags = annot_dict.get_i64("F").unwrap_or(4);
    if locked {
        flags |= 128;
    } else {
        flags &= !128;
    }
    annot_dict.insert("F", Object::Integer(flags));
    Ok(())
}

/// Snaps a candidate node coordinate to nearby target points within `snap_radius` (FR-09.10).
///
/// Returns `(snapped_point, did_snap)`.
pub fn snap_node_to_points(
    node: [f64; 2],
    target_points: &[[f64; 2]],
    snap_radius: f64,
) -> ([f64; 2], bool) {
    let mut best_pt = node;
    let mut min_dist = snap_radius;
    let mut did_snap = false;

    for &target in target_points {
        let dist = ((node[0] - target[0]).powi(2) + (node[1] - target[1]).powi(2)).sqrt();
        if dist <= min_dist {
            min_dist = dist;
            best_pt = target;
            did_snap = true;
        }
    }

    (best_pt, did_snap)
}

/// Snaps a line endpoint to horizontal, vertical, or 45-degree angles relative to an anchor point.
///
/// Traceability: FR-09.10.
pub fn snap_angle_orthogonal(
    anchor: [f64; 2],
    target: [f64; 2],
    angle_tolerance_deg: f64,
) -> [f64; 2] {
    let dx = target[0] - anchor[0];
    let dy = target[1] - anchor[1];
    let dist = (dx * dx + dy * dy).sqrt();
    if dist < 1e-4 {
        return target;
    }

    let angle_deg = dy.atan2(dx).to_degrees();
    let norm_angle = if angle_deg < 0.0 {
        angle_deg + 360.0
    } else {
        angle_deg
    };

    let snap_angles = [0.0, 45.0, 90.0, 135.0, 180.0, 225.0, 270.0, 315.0, 360.0];
    for &snap_a in &snap_angles {
        if (norm_angle - snap_a).abs() <= angle_tolerance_deg {
            let rad = snap_a.to_radians();
            return [anchor[0] + dist * rad.cos(), anchor[1] + dist * rad.sin()];
        }
    }

    target
}

// ---------------------------------------------------------------------------
// Internal Appearance Stream Generators
// ---------------------------------------------------------------------------

fn generate_ink_appearance_stream(config: &InkAnnotationConfig, rect: &[f64; 4]) -> Vec<u8> {
    let mut buf = String::new();
    let ox = rect[0];
    let oy = rect[1];

    buf.push_str("/GS0 gs\n");
    let c = config.color;
    buf.push_str(&format!("{:.3} {:.3} {:.3} RG\n", c.0, c.1, c.2));
    buf.push_str(&format!("{:.2} w\n", config.stroke_width));
    buf.push_str("1 J 1 j\n"); // Round cap and round join

    for stroke in &config.strokes {
        if stroke.is_empty() {
            continue;
        }

        let p0 = stroke[0];
        buf.push_str(&format!("{:.2} {:.2} m\n", p0[0] - ox, p0[1] - oy));

        if config.smooth && stroke.len() >= 3 {
            // Cubic bezier curve smoothing (Catmull-Rom approximation)
            for i in 0..stroke.len() - 1 {
                let p_prev = if i == 0 { stroke[0] } else { stroke[i - 1] };
                let p_curr = stroke[i];
                let p_next = stroke[i + 1];
                let p_after = if i + 2 < stroke.len() {
                    stroke[i + 2]
                } else {
                    p_next
                };

                let c1x = (p_curr[0] + (p_next[0] - p_prev[0]) / 6.0) - ox;
                let c1y = (p_curr[1] + (p_next[1] - p_prev[1]) / 6.0) - oy;
                let c2x = (p_next[0] - (p_after[0] - p_curr[0]) / 6.0) - ox;
                let c2y = (p_next[1] - (p_after[1] - p_curr[1]) / 6.0) - oy;
                let c3x = p_next[0] - ox;
                let c3y = p_next[1] - oy;

                buf.push_str(&format!(
                    "{:.2} {:.2} {:.2} {:.2} {:.2} {:.2} c\n",
                    c1x, c1y, c2x, c2y, c3x, c3y
                ));
            }
        } else {
            for pt in &stroke[1..] {
                buf.push_str(&format!("{:.2} {:.2} l\n", pt[0] - ox, pt[1] - oy));
            }
        }

        buf.push_str("S\n");
    }

    buf.into_bytes()
}

fn generate_shape_appearance_stream(config: &ShapeAnnotationConfig, rect: &[f64; 4]) -> Vec<u8> {
    let mut buf = String::new();
    let ox = rect[0];
    let oy = rect[1];

    buf.push_str("/GS0 gs\n");
    buf.push_str(&format!("{:.2} w\n", config.stroke_width));

    // Dash pattern
    if let BorderStyle::Dashed(ref dash) = config.border_style {
        buf.push('[');
        for &v in dash {
            buf.push_str(&format!("{:.1} ", v));
        }
        buf.push_str("] 0 d\n");
    }

    // Set colors
    if let Some(sc) = config.stroke_color {
        buf.push_str(&format!("{:.3} {:.3} {:.3} RG\n", sc.0, sc.1, sc.2));
    }
    if let Some(fc) = config.fill_color {
        buf.push_str(&format!("{:.3} {:.3} {:.3} rg\n", fc.0, fc.1, fc.2));
    }

    let has_fill = config.fill_color.is_some();
    let has_stroke = config.stroke_color.is_some() && config.stroke_width > 0.0;

    let paint_op = match (has_fill, has_stroke) {
        (true, true) => "B",
        (true, false) => "f",
        (false, true) => "S",
        (false, false) => "n",
    };

    match &config.shape_type {
        ShapeType::Rectangle => {
            let rx = config.rect[0] - ox;
            let ry = config.rect[1] - oy;
            let rw = (config.rect[2] - config.rect[0]).abs();
            let rh = (config.rect[3] - config.rect[1]).abs();

            if let BorderEffect::Cloudy { intensity } = config.border_effect {
                append_cloudy_rect_path(&mut buf, rx, ry, rw, rh, intensity);
                buf.push_str(&format!("{}\n", paint_op));
            } else {
                buf.push_str(&format!("{:.2} {:.2} {:.2} {:.2} re\n", rx, ry, rw, rh));
                buf.push_str(&format!("{}\n", paint_op));
            }
        }

        ShapeType::Circle => {
            let rx = config.rect[0] - ox;
            let ry = config.rect[1] - oy;
            let rw = (config.rect[2] - config.rect[0]).abs();
            let rh = (config.rect[3] - config.rect[1]).abs();

            if let BorderEffect::Cloudy { intensity } = config.border_effect {
                append_cloudy_circle_path(&mut buf, rx, ry, rw, rh, intensity);
                buf.push_str(&format!("{}\n", paint_op));
            } else {
                append_ellipse_path(&mut buf, rx, ry, rw, rh);
                buf.push_str(&format!("{}\n", paint_op));
            }
        }

        ShapeType::Line {
            start,
            end,
            start_style,
            end_style,
        } => {
            let p1 = [start[0] - ox, start[1] - oy];
            let p2 = [end[0] - ox, end[1] - oy];

            buf.push_str(&format!(
                "{:.2} {:.2} m {:.2} {:.2} l S\n",
                p1[0], p1[1], p2[0], p2[1]
            ));

            // Draw line endings
            if *start_style != LineEndingStyle::None {
                append_line_ending(
                    &mut buf,
                    p1,
                    p2,
                    *start_style,
                    config.stroke_width,
                    config.stroke_color,
                    config.fill_color,
                );
            }
            if *end_style != LineEndingStyle::None {
                append_line_ending(
                    &mut buf,
                    p2,
                    p1,
                    *end_style,
                    config.stroke_width,
                    config.stroke_color,
                    config.fill_color,
                );
            }
        }

        ShapeType::Polygon { vertices } => {
            if vertices.len() >= 3 {
                if let BorderEffect::Cloudy { intensity } = config.border_effect {
                    append_cloudy_polygon_path(&mut buf, vertices, ox, oy, intensity);
                    buf.push_str(&format!("{}\n", paint_op));
                } else {
                    let v0 = vertices[0];
                    buf.push_str(&format!("{:.2} {:.2} m\n", v0[0] - ox, v0[1] - oy));
                    for v in &vertices[1..] {
                        buf.push_str(&format!("{:.2} {:.2} l\n", v[0] - ox, v[1] - oy));
                    }
                    buf.push_str("h\n"); // Close path
                    buf.push_str(&format!("{}\n", paint_op));
                }
            }
        }

        ShapeType::PolyLine {
            vertices,
            start_style,
            end_style,
        } => {
            if vertices.len() >= 2 {
                let v0 = vertices[0];
                buf.push_str(&format!("{:.2} {:.2} m\n", v0[0] - ox, v0[1] - oy));
                for v in &vertices[1..] {
                    buf.push_str(&format!("{:.2} {:.2} l\n", v[0] - ox, v[1] - oy));
                }
                buf.push_str("S\n");

                if *start_style != LineEndingStyle::None {
                    let p1 = [vertices[0][0] - ox, vertices[0][1] - oy];
                    let p2 = [vertices[1][0] - ox, vertices[1][1] - oy];
                    append_line_ending(
                        &mut buf,
                        p1,
                        p2,
                        *start_style,
                        config.stroke_width,
                        config.stroke_color,
                        config.fill_color,
                    );
                }
                if *end_style != LineEndingStyle::None {
                    let n = vertices.len();
                    let p1 = [vertices[n - 1][0] - ox, vertices[n - 1][1] - oy];
                    let p2 = [vertices[n - 2][0] - ox, vertices[n - 2][1] - oy];
                    append_line_ending(
                        &mut buf,
                        p1,
                        p2,
                        *end_style,
                        config.stroke_width,
                        config.stroke_color,
                        config.fill_color,
                    );
                }
            }
        }
    }

    buf.into_bytes()
}

fn append_ellipse_path(buf: &mut String, x: f64, y: f64, w: f64, h: f64) {
    let rx = w * 0.5;
    let ry = h * 0.5;
    let cx = x + rx;
    let cy = y + ry;
    let kx = 0.552_284_8 * rx;
    let ky = 0.552_284_8 * ry;

    buf.push_str(&format!("{:.2} {:.2} m\n", cx, cy - ry));
    buf.push_str(&format!(
        "{:.2} {:.2} {:.2} {:.2} {:.2} {:.2} c\n",
        cx + kx,
        cy - ry,
        cx + rx,
        cy - ky,
        cx + rx,
        cy
    ));
    buf.push_str(&format!(
        "{:.2} {:.2} {:.2} {:.2} {:.2} {:.2} c\n",
        cx + rx,
        cy + ky,
        cx + kx,
        cy + ry,
        cx,
        cy + ry
    ));
    buf.push_str(&format!(
        "{:.2} {:.2} {:.2} {:.2} {:.2} {:.2} c\n",
        cx - kx,
        cy + ry,
        cx - rx,
        cy + ky,
        cx - rx,
        cy
    ));
    buf.push_str(&format!(
        "{:.2} {:.2} {:.2} {:.2} {:.2} {:.2} c\n",
        cx - rx,
        cy - ky,
        cx - kx,
        cy - ry,
        cx,
        cy - ry
    ));
    buf.push_str("h\n");
}

fn append_cloudy_rect_path(buf: &mut String, x: f64, y: f64, w: f64, h: f64, intensity: f64) {
    let arc_r = (8.0 * intensity).clamp(4.0, 24.0);
    // Draw scalloped arcs along bottom (x -> x+w), right (y -> y+h), top (x+w -> x), left (y+h -> y)
    let bottom_steps = ((w / (arc_r * 2.0)).round() as usize).max(1);
    let right_steps = ((h / (arc_r * 2.0)).round() as usize).max(1);
    let top_steps = bottom_steps;
    let left_steps = right_steps;

    let step_w = w / bottom_steps as f64;
    let step_h = h / right_steps as f64;

    buf.push_str(&format!("{:.2} {:.2} m\n", x, y));

    // Bottom edge (left to right, arching downwards)
    for i in 0..bottom_steps {
        let x0 = x + i as f64 * step_w;
        let x1 = x + (i + 1) as f64 * step_w;
        let mx = (x0 + x1) * 0.5;
        let my = y - arc_r * 0.5;
        buf.push_str(&format!(
            "{:.2} {:.2} {:.2} {:.2} {:.2} {:.2} c\n",
            x0 + (mx - x0) * 0.5,
            my,
            mx + (x1 - mx) * 0.5,
            my,
            x1,
            y
        ));
    }

    // Right edge (bottom to top, arching rightwards)
    for i in 0..right_steps {
        let y0 = y + i as f64 * step_h;
        let y1 = y + (i + 1) as f64 * step_h;
        let my = (y0 + y1) * 0.5;
        let mx = x + w + arc_r * 0.5;
        buf.push_str(&format!(
            "{:.2} {:.2} {:.2} {:.2} {:.2} {:.2} c\n",
            mx,
            y0 + (my - y0) * 0.5,
            mx,
            my + (y1 - my) * 0.5,
            x + w,
            y1
        ));
    }

    // Top edge (right to left, arching upwards)
    for i in 0..top_steps {
        let x0 = x + w - i as f64 * step_w;
        let x1 = x + w - (i + 1) as f64 * step_w;
        let mx = (x0 + x1) * 0.5;
        let my = y + h + arc_r * 0.5;
        buf.push_str(&format!(
            "{:.2} {:.2} {:.2} {:.2} {:.2} {:.2} c\n",
            x0 + (mx - x0) * 0.5,
            my,
            mx + (x1 - mx) * 0.5,
            my,
            x1,
            y + h
        ));
    }

    // Left edge (top to bottom, arching leftwards)
    for i in 0..left_steps {
        let y0 = y + h - i as f64 * step_h;
        let y1 = y + h - (i + 1) as f64 * step_h;
        let my = (y0 + y1) * 0.5;
        let mx = x - arc_r * 0.5;
        buf.push_str(&format!(
            "{:.2} {:.2} {:.2} {:.2} {:.2} {:.2} c\n",
            mx,
            y0 + (my - y0) * 0.5,
            mx,
            my + (y1 - my) * 0.5,
            x,
            y1
        ));
    }

    buf.push_str("h\n");
}

fn append_cloudy_circle_path(buf: &mut String, x: f64, y: f64, w: f64, h: f64, intensity: f64) {
    let rx = w * 0.5;
    let ry = h * 0.5;
    let cx = x + rx;
    let cy = y + ry;
    let num_arcs = (12.0 * intensity).round().clamp(8.0, 36.0) as usize;
    let arc_angle = std::f64::consts::TAU / num_arcs as f64;
    let bump = (4.0 * intensity).max(2.0);

    let start_x = cx + rx;
    let start_y = cy;
    buf.push_str(&format!("{:.2} {:.2} m\n", start_x, start_y));

    for i in 0..num_arcs {
        let a0 = i as f64 * arc_angle;
        let a1 = (i + 1) as f64 * arc_angle;
        let am = (a0 + a1) * 0.5;

        let end_x = cx + rx * a1.cos();
        let end_y = cy + ry * a1.sin();

        let mid_x = cx + (rx + bump) * am.cos();
        let mid_y = cy + (ry + bump) * am.sin();

        buf.push_str(&format!(
            "{:.2} {:.2} {:.2} {:.2} {:.2} {:.2} c\n",
            mid_x, mid_y, mid_x, mid_y, end_x, end_y
        ));
    }

    buf.push_str("h\n");
}

fn append_cloudy_polygon_path(
    buf: &mut String,
    vertices: &[[f64; 2]],
    ox: f64,
    oy: f64,
    intensity: f64,
) {
    let arc_r = (6.0 * intensity).clamp(3.0, 18.0);
    let v0 = vertices[0];
    buf.push_str(&format!("{:.2} {:.2} m\n", v0[0] - ox, v0[1] - oy));

    for i in 0..vertices.len() {
        let p1 = vertices[i];
        let p2 = vertices[(i + 1) % vertices.len()];

        let dx = p2[0] - p1[0];
        let dy = p2[1] - p1[1];
        let seg_len = (dx * dx + dy * dy).sqrt();
        let steps = ((seg_len / (arc_r * 2.0)).round() as usize).max(1);

        let step_x = dx / steps as f64;
        let step_y = dy / steps as f64;

        // Normal vector pointing outwards (rotate (dx, dy) 90 deg clockwise)
        let nx = dy / seg_len;
        let ny = -dx / seg_len;

        for s in 0..steps {
            let sx = p1[0] + s as f64 * step_x;
            let sy = p1[1] + s as f64 * step_y;
            let ex = p1[0] + (s + 1) as f64 * step_x;
            let ey = p1[1] + (s + 1) as f64 * step_y;

            let mx = (sx + ex) * 0.5 + nx * arc_r * 0.6;
            let my = (sy + ey) * 0.5 + ny * arc_r * 0.6;

            buf.push_str(&format!(
                "{:.2} {:.2} {:.2} {:.2} {:.2} {:.2} c\n",
                mx - ox,
                my - oy,
                mx - ox,
                my - oy,
                ex - ox,
                ey - oy
            ));
        }
    }

    buf.push_str("h\n");
}

fn append_line_ending(
    buf: &mut String,
    tip: [f64; 2],
    origin: [f64; 2],
    style: LineEndingStyle,
    stroke_width: f64,
    stroke_color: Option<(f32, f32, f32)>,
    fill_color: Option<(f32, f32, f32)>,
) {
    let dx = tip[0] - origin[0];
    let dy = tip[1] - origin[1];
    let len = (dx * dx + dy * dy).sqrt();
    if len < 1e-4 {
        return;
    }

    let ux = dx / len;
    let uy = dy / len;
    let perp_x = -uy;
    let perp_y = ux;

    let arrow_size = (stroke_width * 3.5).clamp(8.0, 28.0);

    match style {
        LineEndingStyle::OpenArrow => {
            let left_x = tip[0] - arrow_size * ux + arrow_size * 0.5 * perp_x;
            let left_y = tip[1] - arrow_size * uy + arrow_size * 0.5 * perp_y;
            let right_x = tip[0] - arrow_size * ux - arrow_size * 0.5 * perp_x;
            let right_y = tip[1] - arrow_size * uy - arrow_size * 0.5 * perp_y;

            buf.push_str(&format!(
                "{:.2} {:.2} m {:.2} {:.2} l {:.2} {:.2} l S\n",
                left_x, left_y, tip[0], tip[1], right_x, right_y
            ));
        }

        LineEndingStyle::ClosedArrow => {
            let left_x = tip[0] - arrow_size * ux + arrow_size * 0.5 * perp_x;
            let left_y = tip[1] - arrow_size * uy + arrow_size * 0.5 * perp_y;
            let right_x = tip[0] - arrow_size * ux - arrow_size * 0.5 * perp_x;
            let right_y = tip[1] - arrow_size * uy - arrow_size * 0.5 * perp_y;

            buf.push_str(&format!(
                "{:.2} {:.2} m {:.2} {:.2} l {:.2} {:.2} l h ",
                tip[0], tip[1], left_x, left_y, right_x, right_y
            ));
            if fill_color.is_some() || stroke_color.is_some() {
                buf.push_str("B\n");
            } else {
                buf.push_str("S\n");
            }
        }

        LineEndingStyle::Circle => {
            let r = arrow_size * 0.4;
            let cx = tip[0] - r * ux;
            let cy = tip[1] - r * uy;
            append_ellipse_path(buf, cx - r, cy - r, r * 2.0, r * 2.0);
            if fill_color.is_some() {
                buf.push_str("B\n");
            } else {
                buf.push_str("S\n");
            }
        }

        LineEndingStyle::Square => {
            let half = arrow_size * 0.4;
            let cx = tip[0] - half * ux;
            let cy = tip[1] - half * uy;
            buf.push_str(&format!(
                "{:.2} {:.2} {:.2} {:.2} re ",
                cx - half,
                cy - half,
                half * 2.0,
                half * 2.0
            ));
            if fill_color.is_some() {
                buf.push_str("B\n");
            } else {
                buf.push_str("S\n");
            }
        }

        LineEndingStyle::Diamond => {
            let d = arrow_size * 0.5;
            let p_top = [tip[0], tip[1]];
            let p_left = [
                tip[0] - d * ux + d * 0.5 * perp_x,
                tip[1] - d * uy + d * 0.5 * perp_y,
            ];
            let p_bot = [tip[0] - d * 2.0 * ux, tip[1] - d * 2.0 * uy];
            let p_right = [
                tip[0] - d * ux - d * 0.5 * perp_x,
                tip[1] - d * uy - d * 0.5 * perp_y,
            ];

            buf.push_str(&format!(
                "{:.2} {:.2} m {:.2} {:.2} l {:.2} {:.2} l {:.2} {:.2} l h ",
                p_top[0],
                p_top[1],
                p_left[0],
                p_left[1],
                p_bot[0],
                p_bot[1],
                p_right[0],
                p_right[1]
            ));
            if fill_color.is_some() {
                buf.push_str("B\n");
            } else {
                buf.push_str("S\n");
            }
        }

        LineEndingStyle::Butt => {
            let half = arrow_size * 0.5;
            let p1 = [tip[0] + half * perp_x, tip[1] + half * perp_y];
            let p2 = [tip[0] - half * perp_x, tip[1] - half * perp_y];
            buf.push_str(&format!(
                "{:.2} {:.2} m {:.2} {:.2} l S\n",
                p1[0], p1[1], p2[0], p2[1]
            ));
        }

        LineEndingStyle::Slash => {
            let half = arrow_size * 0.5;
            let diag_x = (perp_x + ux) * std::f64::consts::FRAC_1_SQRT_2;
            let diag_y = (perp_y + uy) * std::f64::consts::FRAC_1_SQRT_2;
            let p1 = [tip[0] + half * diag_x, tip[1] + half * diag_y];
            let p2 = [tip[0] - half * diag_x, tip[1] - half * diag_y];
            buf.push_str(&format!(
                "{:.2} {:.2} m {:.2} {:.2} l S\n",
                p1[0], p1[1], p2[0], p2[1]
            ));
        }

        _ => {}
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn bounds_of_points(points: &[[f64; 2]], pad: f64) -> (f64, f64, f64, f64) {
    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut max_y = f64::NEG_INFINITY;

    for &[x, y] in points {
        if x < min_x {
            min_x = x;
        }
        if y < min_y {
            min_y = y;
        }
        if x > max_x {
            max_x = x;
        }
        if y > max_y {
            max_y = y;
        }
    }

    (min_x - pad, min_y - pad, max_x + pad, max_y + pad)
}

fn point_to_segment_distance(pt: [f64; 2], seg_a: [f64; 2], seg_b: [f64; 2]) -> f64 {
    let dx = seg_b[0] - seg_a[0];
    let dy = seg_b[1] - seg_a[1];
    let len_sq = dx * dx + dy * dy;

    if len_sq < 1e-6 {
        return ((pt[0] - seg_a[0]).powi(2) + (pt[1] - seg_a[1]).powi(2)).sqrt();
    }

    let t = (((pt[0] - seg_a[0]) * dx + (pt[1] - seg_a[1]) * dy) / len_sq).clamp(0.0, 1.0);
    let proj_x = seg_a[0] + t * dx;
    let proj_y = seg_a[1] + t * dy;

    ((pt[0] - proj_x).powi(2) + (pt[1] - proj_y).powi(2)).sqrt()
}

fn attach_annot_to_page(
    doc: &mut PdfDocument,
    page_id: ObjectId,
    annot_id: ObjectId,
) -> Result<()> {
    let page_obj = doc
        .objects
        .get_mut(&page_id)
        .ok_or(PdfError::ObjectNotFound(page_id.number, page_id.generation))?;

    let page_dict = page_obj
        .as_dict_mut()
        .ok_or_else(|| PdfError::InvalidStructure("Expected page dictionary".into()))?;

    let annots = page_dict
        .map
        .entry(PdfName::new("Annots"))
        .or_insert_with(|| Object::Array(Vec::new()));

    if let Object::Array(arr) = annots {
        arr.push(Object::Reference(annot_id));
    }

    Ok(())
}

fn remove_annot_from_page(
    doc: &mut PdfDocument,
    page_id: ObjectId,
    annot_id: ObjectId,
) -> Result<()> {
    let page_obj = doc
        .objects
        .get_mut(&page_id)
        .ok_or(PdfError::ObjectNotFound(page_id.number, page_id.generation))?;

    let page_dict = page_obj
        .as_dict_mut()
        .ok_or_else(|| PdfError::InvalidStructure("Expected page dictionary".into()))?;

    if let Some(Object::Array(arr)) = page_dict.map.get_mut(&PdfName::new("Annots")) {
        arr.retain(|obj| {
            if let Object::Reference(id) = obj {
                *id != annot_id
            } else {
                true
            }
        });
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_and_erase_ink_annotation() {
        let mut doc = PdfDocument::new();
        doc.add_blank_page(612.0, 792.0);

        let stroke1 = vec![
            [100.0, 100.0],
            [110.0, 120.0],
            [130.0, 150.0],
            [160.0, 180.0],
        ];
        let stroke2 = vec![[300.0, 300.0], [310.0, 320.0], [330.0, 350.0]];

        let config = InkAnnotationConfig {
            strokes: vec![stroke1, stroke2],
            stroke_width: 3.0,
            color: (0.8, 0.1, 0.1),
            opacity: 0.9,
            smooth: true,
            author: Some("Architect".into()),
            locked: true,
            popup_note: Some("Check foundation wiring".into()),
        };

        let annot_id = add_ink_annotation(&mut doc, 0, &config).expect("add ink");
        assert!(doc.objects.contains_key(&annot_id));

        let dict = doc.objects.get(&annot_id).unwrap().as_dict().unwrap();
        assert_eq!(dict.get_name("Subtype"), Some("Ink"));
        assert_eq!(dict.get_i64("F"), Some(132)); // 4 (Print) + 128 (Locked)
        assert!(dict.contains_key("AP"));

        // Test eraser on stroke 1
        let erased =
            erase_ink_strokes(&mut doc, 0, annot_id, [115.0, 125.0], 15.0).expect("erase stroke");
        assert!(erased);

        // Annotation should still exist with only stroke 2
        assert!(doc.objects.contains_key(&annot_id));
        let updated_dict = doc.objects.get(&annot_id).unwrap().as_dict().unwrap();
        let ink_list = updated_dict.get_array("InkList").unwrap();
        assert_eq!(ink_list.len(), 1);

        // Test eraser on stroke 2 (all strokes gone => annotation deleted)
        let erased_all =
            erase_ink_strokes(&mut doc, 0, annot_id, [315.0, 325.0], 20.0).expect("erase all");
        assert!(erased_all);
        assert!(!doc.objects.contains_key(&annot_id));
    }

    #[test]
    fn test_shape_annotations_and_cloudy_borders() {
        let mut doc = PdfDocument::new();
        doc.add_blank_page(612.0, 792.0);

        // 1. Cloudy Rectangle
        let cloud_rect = ShapeAnnotationConfig {
            shape_type: ShapeType::Rectangle,
            rect: [50.0, 500.0, 250.0, 650.0],
            stroke_color: Some((0.1, 0.6, 0.2)),
            fill_color: Some((0.9, 1.0, 0.9)),
            stroke_width: 2.5,
            border_style: BorderStyle::Solid,
            border_effect: BorderEffect::Cloudy { intensity: 1.5 },
            opacity: 0.85,
            author: Some("Engineer".into()),
            locked: false,
            popup_note: Some("Revision Area A".into()),
        };
        let rect_id = add_shape_annotation(&mut doc, 0, &cloud_rect).expect("add cloudy rect");
        assert!(doc.objects.contains_key(&rect_id));

        let dict = doc.objects.get(&rect_id).unwrap().as_dict().unwrap();
        assert_eq!(dict.get_name("Subtype"), Some("Square"));
        assert!(dict.contains_key("BE"));

        // 2. Double-Headed Arrow
        let arrow_config = ShapeAnnotationConfig {
            shape_type: ShapeType::Line {
                start: [100.0, 200.0],
                end: [400.0, 200.0],
                start_style: LineEndingStyle::OpenArrow,
                end_style: LineEndingStyle::ClosedArrow,
            },
            rect: [100.0, 200.0, 400.0, 200.0],
            stroke_color: Some((0.8, 0.0, 0.0)),
            fill_color: Some((0.8, 0.0, 0.0)),
            stroke_width: 3.0,
            border_style: BorderStyle::Dashed(vec![4.0, 2.0]),
            border_effect: BorderEffect::None,
            opacity: 1.0,
            author: None,
            locked: true,
            popup_note: None,
        };
        let arrow_id = add_shape_annotation(&mut doc, 0, &arrow_config).expect("add arrow");
        assert!(doc.objects.contains_key(&arrow_id));

        let a_dict = doc.objects.get(&arrow_id).unwrap().as_dict().unwrap();
        assert_eq!(a_dict.get_name("Subtype"), Some("Line"));
        let le = a_dict.get_array("LE").unwrap();
        assert_eq!(le[0].as_name(), Some("OpenArrow"));
        assert_eq!(le[1].as_name(), Some("ClosedArrow"));

        // Test lock toggle
        set_annotation_locked(&mut doc, arrow_id, false).expect("unlock");
        let a_dict2 = doc.objects.get(&arrow_id).unwrap().as_dict().unwrap();
        assert_eq!(a_dict2.get_i64("F").unwrap() & 128, 0);

        // 3. Node Snapping Engine (FR-09.10)
        let targets = vec![[100.0, 100.0], [200.0, 200.0], [300.0, 300.0]];
        let (snapped, did_snap) = snap_node_to_points([102.5, 98.0], &targets, 5.0);
        assert!(did_snap);
        assert_eq!(snapped, [100.0, 100.0]);

        let (no_snap, did_snap2) = snap_node_to_points([150.0, 150.0], &targets, 5.0);
        assert!(!did_snap2);
        assert_eq!(no_snap, [150.0, 150.0]);

        // Orthogonal snapping
        let ortho = snap_angle_orthogonal([0.0, 0.0], [100.0, 2.0], 5.0);
        assert!((ortho[1]).abs() < 1e-3);
    }
}
