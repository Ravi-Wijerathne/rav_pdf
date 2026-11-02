//! PDF Rubber Stamps, Dynamic Stamp Generation, Callout & Typewriter Annotations conforming to ISO 32000-1.
//!
//! Traceability:
//! - FR-08.1.3 (Callout text annotations: text box with leader line pointing to area)
//! - FR-08.1.4 (Typewriter annotations for direct text placement)
//! - FR-10.1 (Predefined standard business stamps: Approved, Draft, Confidential, etc.)
//! - FR-10.2 (Predefined Sign Here stamps: Sign Here, Initial Here, Witness, Accepted)
//! - FR-10.3 (Dynamic stamps with current date, time, and user identity)
//! - FR-10.4, FR-10.5 (Custom stamps and category organization)
//! - FR-10.6, FR-10.7 (Stamp resizing, repositioning, rotation, and pop-up notes)

use chrono::{DateTime, Utc};
use ravpdf_core::document::PdfDocument;
use ravpdf_core::error::{PdfError, Result};
use ravpdf_core::object::{Object, ObjectId, PdfDictionary, PdfName, PdfStream, PdfString};

/// Predefined standard business and signing rubber stamps conforming to ISO 32000-1 Section 12.5.6.12.
///
/// Traceability: FR-10.1, FR-10.2, FR-10.4.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum StandardStamp {
    // Standard Business Stamps (FR-10.1)
    Approved,
    NotApproved,
    Draft,
    Final,
    Confidential,
    ForPublicRelease,
    NotForPublicRelease,
    Void,
    PreliminaryResults,
    InformationOnly,
    ForComment,
    Expired,

    // Sign Here Stamps (FR-10.2)
    SignHere,
    InitialHere,
    Witness,
    Accepted,

    // Custom Named Stamp (FR-10.4, FR-10.5)
    Custom(String),
}

impl StandardStamp {
    /// Human-readable label for the stamp banner.
    pub fn label(&self) -> &str {
        match self {
            StandardStamp::Approved => "APPROVED",
            StandardStamp::NotApproved => "NOT APPROVED",
            StandardStamp::Draft => "DRAFT",
            StandardStamp::Final => "FINAL",
            StandardStamp::Confidential => "CONFIDENTIAL",
            StandardStamp::ForPublicRelease => "FOR PUBLIC RELEASE",
            StandardStamp::NotForPublicRelease => "NOT FOR PUBLIC RELEASE",
            StandardStamp::Void => "VOID",
            StandardStamp::PreliminaryResults => "PRELIMINARY RESULTS",
            StandardStamp::InformationOnly => "INFORMATION ONLY",
            StandardStamp::ForComment => "FOR COMMENT",
            StandardStamp::Expired => "EXPIRED",
            StandardStamp::SignHere => "SIGN HERE",
            StandardStamp::InitialHere => "INITIAL HERE",
            StandardStamp::Witness => "WITNESS",
            StandardStamp::Accepted => "ACCEPTED",
            StandardStamp::Custom(name) => name.as_str(),
        }
    }

    /// PDF standard Name key for `/Subtype /Stamp /Name /...`.
    pub fn pdf_name(&self) -> &str {
        match self {
            StandardStamp::Approved => "Approved",
            StandardStamp::NotApproved => "NotApproved",
            StandardStamp::Draft => "Draft",
            StandardStamp::Final => "Final",
            StandardStamp::Confidential => "Confidential",
            StandardStamp::ForPublicRelease => "ForPublicRelease",
            StandardStamp::NotForPublicRelease => "NotForPublicRelease",
            StandardStamp::Void => "Void",
            StandardStamp::PreliminaryResults => "PreliminaryResults",
            StandardStamp::InformationOnly => "InformationOnly",
            StandardStamp::ForComment => "ForComment",
            StandardStamp::Expired => "Expired",
            StandardStamp::SignHere => "SignHere",
            StandardStamp::InitialHere => "InitialHere",
            StandardStamp::Witness => "Witness",
            StandardStamp::Accepted => "Accepted",
            StandardStamp::Custom(name) => name.as_str(),
        }
    }

    /// Default primary RGB color for the stamp border and text.
    pub fn default_color(&self) -> (f32, f32, f32) {
        match self {
            StandardStamp::Approved | StandardStamp::Accepted | StandardStamp::ForPublicRelease => {
                (0.12, 0.55, 0.22)
            } // Dark Green
            StandardStamp::NotApproved
            | StandardStamp::Void
            | StandardStamp::Expired
            | StandardStamp::Confidential
            | StandardStamp::NotForPublicRelease => (0.82, 0.12, 0.12), // Deep Red
            StandardStamp::Draft
            | StandardStamp::PreliminaryResults
            | StandardStamp::ForComment => (0.85, 0.45, 0.05), // Amber / Warm Orange
            StandardStamp::Final
            | StandardStamp::InformationOnly
            | StandardStamp::Witness
            | StandardStamp::InitialHere => (0.10, 0.35, 0.75), // Corporate Blue
            StandardStamp::SignHere => (0.85, 0.60, 0.0), // Gold / Signpost
            StandardStamp::Custom(_) => (0.2, 0.2, 0.2),  // Dark Charcoal
        }
    }

    /// Default recommended dimensions `(width, height)` in PDF points.
    pub fn default_dimensions(&self) -> (f64, f64) {
        match self {
            StandardStamp::Approved
            | StandardStamp::NotApproved
            | StandardStamp::Draft
            | StandardStamp::Final
            | StandardStamp::Void
            | StandardStamp::Accepted => (180.0, 56.0),
            StandardStamp::Confidential
            | StandardStamp::ForPublicRelease
            | StandardStamp::NotForPublicRelease
            | StandardStamp::PreliminaryResults
            | StandardStamp::InformationOnly
            | StandardStamp::ForComment
            | StandardStamp::Expired => (220.0, 60.0),
            StandardStamp::SignHere | StandardStamp::InitialHere | StandardStamp::Witness => {
                (140.0, 42.0)
            }
            StandardStamp::Custom(_) => (180.0, 56.0),
        }
    }

    /// Standard user category organizing stamps (FR-10.5).
    pub fn category(&self) -> &'static str {
        match self {
            StandardStamp::SignHere
            | StandardStamp::InitialHere
            | StandardStamp::Witness
            | StandardStamp::Accepted => "Sign Here",
            StandardStamp::Custom(_) => "Custom",
            _ => "Standard Business",
        }
    }

    /// Parses a string loosely into a standard stamp type.
    pub fn from_str_loose(s: &str) -> Self {
        let clean = s.trim().to_lowercase().replace(['-', '_', ' '], "");
        match clean.as_str() {
            "approved" => StandardStamp::Approved,
            "notapproved" => StandardStamp::NotApproved,
            "draft" => StandardStamp::Draft,
            "final" => StandardStamp::Final,
            "confidential" => StandardStamp::Confidential,
            "forpublicrelease" | "publicrelease" => StandardStamp::ForPublicRelease,
            "notforpublicrelease" => StandardStamp::NotForPublicRelease,
            "void" => StandardStamp::Void,
            "preliminaryresults" | "preliminary" => StandardStamp::PreliminaryResults,
            "informationonly" | "info" => StandardStamp::InformationOnly,
            "forcomment" | "comment" => StandardStamp::ForComment,
            "expired" => StandardStamp::Expired,
            "signhere" => StandardStamp::SignHere,
            "initialhere" => StandardStamp::InitialHere,
            "witness" => StandardStamp::Witness,
            "accepted" => StandardStamp::Accepted,
            _ => StandardStamp::Custom(s.trim().to_string()),
        }
    }
}

/// Configuration options for stamping onto a PDF document.
///
/// Traceability: FR-10.1 - FR-10.7.
#[derive(Debug, Clone)]
pub struct StampConfig {
    /// Type of stamp (Standard business, sign here, or custom)
    pub stamp_type: StandardStamp,
    /// Bounding rectangle `[x, y, width, height]` in PDF points
    pub rect: [f64; 4],
    /// Author / user identity (for dynamic stamp embed and annotation T field)
    pub author: Option<String>,
    /// Dynamic timestamp to embed (FR-10.3). If Some, generates dynamic subtitle line.
    pub dynamic_date: Option<DateTime<Utc>>,
    /// Custom override text (if None, uses stamp_type.label())
    pub custom_text: Option<String>,
    /// Custom border & text color. If None, uses stamp_type.default_color().
    pub color: Option<(f32, f32, f32)>,
    /// Alpha opacity `[0.0, 1.0]` (default 0.9 for rubber stamp look)
    pub opacity: f32,
    /// Rotation tilt angle in degrees (e.g. -12.0 for classic angled rubber stamp, 0.0 for flat)
    pub rotation_deg: f64,
    /// Optional pop-up note contents attached to the stamp (FR-10.7)
    pub popup_note: Option<String>,
}

impl Default for StampConfig {
    fn default() -> Self {
        let (w, h) = StandardStamp::Approved.default_dimensions();
        Self {
            stamp_type: StandardStamp::Approved,
            rect: [72.0, 650.0, w, h],
            author: None,
            dynamic_date: None,
            custom_text: None,
            color: None,
            opacity: 0.92,
            rotation_deg: -8.0,
            popup_note: None,
        }
    }
}

/// Adds a rubber stamp annotation with an ISO 32000-1 Appearance Stream (`/AP /N`) onto a page.
///
/// Conforms to ISO 32000-1 Clause 12.5.6.12 (Stamp Annotations) & Clause 12.5.5 (Appearance Streams).
/// Traceability: FR-10.1 - FR-10.7.
pub fn add_stamp_annotation(
    doc: &mut PdfDocument,
    page_index: usize,
    config: &StampConfig,
) -> Result<ObjectId> {
    let page_ids = doc.page_ids();
    let page_id = *page_ids
        .get(page_index)
        .ok_or_else(|| PdfError::InvalidStructure(format!("Page {} out of range", page_index)))?;

    let color = config
        .color
        .unwrap_or_else(|| config.stamp_type.default_color());
    let label = config
        .custom_text
        .as_deref()
        .unwrap_or_else(|| config.stamp_type.label());

    let w = config.rect[2].max(40.0);
    let h = config.rect[3].max(20.0);

    // 1. Generate ISO 32000-1 Appearance Stream for the stamp
    let mut app_stream_content = String::new();
    app_stream_content.push_str("q\n");

    // Optional rotation around center
    if config.rotation_deg.abs() > 0.01 {
        let rad = config.rotation_deg.to_radians();
        let cos_r = rad.cos();
        let sin_r = rad.sin();
        let cx = w / 2.0;
        let cy = h / 2.0;
        // Translate to center, rotate, translate back:
        // CTM = [cos, sin, -sin, cos, cx - cx*cos + cy*sin, cy - cx*sin - cy*cos]
        let e = cx - cx * cos_r + cy * sin_r;
        let f = cy - cx * sin_r - cy * cos_r;
        app_stream_content.push_str(&format!(
            "{:.4} {:.4} {:.4} {:.4} {:.2} {:.2} cm\n",
            cos_r, sin_r, -sin_r, cos_r, e, f
        ));
    }

    // Background fill (light tint with 12% opacity)
    app_stream_content.push_str(&format!(
        "{:.3} {:.3} {:.3} rg\n",
        color.0 * 0.15 + 0.85,
        color.1 * 0.15 + 0.85,
        color.2 * 0.15 + 0.85
    ));
    app_stream_content.push_str(&format!("2.0 2.0 {:.2} {:.2} re f\n", w - 4.0, h - 4.0));

    // Outer double border
    app_stream_content.push_str(&format!(
        "{:.3} {:.3} {:.3} RG\n2.5 w\n",
        color.0, color.1, color.2
    ));
    app_stream_content.push_str(&format!("2.0 2.0 {:.2} {:.2} re S\n", w - 4.0, h - 4.0));
    app_stream_content.push_str("0.75 w\n");
    app_stream_content.push_str(&format!("5.0 5.0 {:.2} {:.2} re S\n", w - 10.0, h - 10.0));

    // Text placement
    app_stream_content.push_str(&format!(
        "BT\n/F1 {:.2} Tf\n{:.3} {:.3} {:.3} rg\n",
        if config.dynamic_date.is_some() {
            h * 0.38
        } else {
            h * 0.48
        },
        color.0,
        color.1,
        color.2
    ));

    let main_font_size = if config.dynamic_date.is_some() {
        (h * 0.38).clamp(8.0, 28.0)
    } else {
        (h * 0.48).clamp(9.0, 32.0)
    };

    let est_char_w = main_font_size * 0.55;
    let text_w = label.len() as f64 * est_char_w;
    let text_x = ((w - text_w) / 2.0).max(8.0);
    let text_y = if config.dynamic_date.is_some() {
        h * 0.52
    } else {
        (h - main_font_size) / 2.0 + 2.0
    };

    let escaped_label = escape_pdf_str(label);
    app_stream_content.push_str(&format!(
        "1 0 0 1 {:.2} {:.2} Tm\n/F1 {:.2} Tf\n({}) Tj\n",
        text_x, text_y, main_font_size, escaped_label
    ));

    // Dynamic subtitle (FR-10.3: Date, Time, User)
    if let Some(date) = config.dynamic_date {
        let sub_font_size = (main_font_size * 0.48).clamp(6.0, 12.0);
        let author_str = config.author.as_deref().unwrap_or("Authorized");
        let date_str = date.format("%Y-%m-%d %H:%M UTC").to_string();
        let sub_text = format!("By: {} | {}", author_str, date_str);
        let sub_w = sub_text.len() as f64 * (sub_font_size * 0.52);
        let sub_x = ((w - sub_w) / 2.0).max(6.0);
        let sub_y = text_y - sub_font_size - 4.0;
        let escaped_sub = escape_pdf_str(&sub_text);

        app_stream_content.push_str(&format!(
            "1 0 0 1 {:.2} {:.2} Tm\n/F1 {:.2} Tf\n({}) Tj\n",
            sub_x, sub_y, sub_font_size, escaped_sub
        ));
    }

    app_stream_content.push_str("ET\nQ\n");

    // 2. Create the Appearance Form XObject
    let app_obj_id = doc.next_object_id();
    let mut app_dict = PdfDictionary::new();
    app_dict.insert("Type", PdfName::new("XObject"));
    app_dict.insert("Subtype", PdfName::new("Form"));
    app_dict.insert(
        "BBox",
        vec![
            Object::Real(0.0),
            Object::Real(0.0),
            Object::Real(w),
            Object::Real(h),
        ],
    );

    // Register font in Form XObject resources
    let mut form_res = PdfDictionary::new();
    let mut form_fonts = PdfDictionary::new();
    let mut f1_dict = PdfDictionary::new();
    f1_dict.insert("Type", PdfName::new("Font"));
    f1_dict.insert("Subtype", PdfName::new("Type1"));
    f1_dict.insert("BaseFont", PdfName::new("Helvetica-Bold"));
    form_fonts.insert("F1", f1_dict);
    form_res.insert("Font", form_fonts);
    app_dict.insert("Resources", form_res);

    let mut app_stream = PdfStream::with_plain_data(app_dict, app_stream_content.into_bytes());
    let _ = app_stream.compress_flate();
    doc.objects.insert(app_obj_id, Object::Stream(app_stream));

    // 3. Create the Stamp Annotation Dictionary
    let annot_id = doc.next_object_id();
    let mut annot_dict = PdfDictionary::new();
    annot_dict.insert("Type", PdfName::new("Annot"));
    annot_dict.insert("Subtype", PdfName::new("Stamp"));
    annot_dict.insert(
        "Rect",
        vec![
            Object::Real(config.rect[0]),
            Object::Real(config.rect[1]),
            Object::Real(config.rect[0] + w),
            Object::Real(config.rect[1] + h),
        ],
    );
    annot_dict.insert("Name", PdfName::new(config.stamp_type.pdf_name()));

    let mut ap_dict = PdfDictionary::new();
    ap_dict.insert("N", app_obj_id);
    annot_dict.insert("AP", ap_dict);

    if (config.opacity - 1.0).abs() > 0.001 {
        annot_dict.insert("CA", config.opacity as f64);
    }

    if let Some(ref auth) = config.author {
        annot_dict.insert("T", PdfString::from_str(auth));
    }

    if let Some(ref note) = config.popup_note {
        annot_dict.insert("Contents", PdfString::from_str(note));
    }

    doc.objects.insert(annot_id, Object::Dictionary(annot_dict));

    // 4. Attach to Page /Annots
    let page_obj = doc
        .objects
        .get_mut(&page_id)
        .ok_or(PdfError::ObjectNotFound(page_id.number, page_id.generation))?;

    let p_dict = page_obj
        .as_dict_mut()
        .ok_or_else(|| PdfError::InvalidStructure("Expected page dictionary".into()))?;

    match p_dict.get_mut("Annots") {
        Some(Object::Array(arr)) => arr.push(Object::Reference(annot_id)),
        Some(Object::Reference(orig)) => {
            let o = *orig;
            p_dict.insert(
                "Annots",
                vec![Object::Reference(o), Object::Reference(annot_id)],
            );
        }
        _ => {
            p_dict.insert("Annots", vec![Object::Reference(annot_id)]);
        }
    }

    Ok(annot_id)
}

/// Configuration for a Callout text annotation with a pointing leader line.
///
/// Traceability: FR-08.1.3.
#[derive(Debug, Clone)]
pub struct CalloutConfig {
    /// Bounding box of the text note `[x, y, width, height]` in PDF points
    pub text_rect: [f64; 4],
    /// Callout text contents
    pub text: String,
    /// Point coordinates `(x, y)` where the arrow / pointer originates
    pub leader_start: (f64, f64),
    /// Knee / elbow coordinates `(x, y)` where the leader line bends towards the box
    pub leader_knee: (f64, f64),
    /// Line and border color (RGB 0.0 - 1.0)
    pub border_color: (f32, f32, f32),
    /// Optional box background fill color
    pub fill_color: Option<(f32, f32, f32)>,
    /// Text color (default black)
    pub text_color: (f32, f32, f32),
    /// Font size in points (default 10.0)
    pub font_size: f64,
    /// Author / reviewer name
    pub author: Option<String>,
}

impl Default for CalloutConfig {
    fn default() -> Self {
        Self {
            text_rect: [200.0, 500.0, 160.0, 48.0],
            text: "See note here".to_string(),
            leader_start: (100.0, 600.0),
            leader_knee: (150.0, 550.0),
            border_color: (0.1, 0.45, 0.85),
            fill_color: Some((0.95, 0.97, 1.0)),
            text_color: (0.1, 0.1, 0.1),
            font_size: 10.0,
            author: None,
        }
    }
}

/// Adds an ISO 32000-1 Callout FreeText annotation (`/IT /FreeTextCallout`) with leader line and appearance stream.
///
/// Conforms to ISO 32000-1 Clause 12.5.6.6 (Free Text Annotations, Table 174). Traceability: FR-08.1.3.
pub fn add_callout_annotation(
    doc: &mut PdfDocument,
    page_index: usize,
    config: &CalloutConfig,
) -> Result<ObjectId> {
    let page_ids = doc.page_ids();
    let page_id = *page_ids
        .get(page_index)
        .ok_or_else(|| PdfError::InvalidStructure(format!("Page {} out of range", page_index)))?;

    // Overall bounding box including leader line
    let min_x = config.text_rect[0]
        .min(config.leader_start.0)
        .min(config.leader_knee.0);
    let max_x = (config.text_rect[0] + config.text_rect[2])
        .max(config.leader_start.0)
        .max(config.leader_knee.0);
    let min_y = config.text_rect[1]
        .min(config.leader_start.1)
        .min(config.leader_knee.1);
    let max_y = (config.text_rect[1] + config.text_rect[3])
        .max(config.leader_start.1)
        .max(config.leader_knee.1);

    let annot_w = max_x - min_x + 10.0;
    let annot_h = max_y - min_y + 10.0;

    // Leader line target on box (connect knee to closest point on box)
    let box_cx = config.text_rect[0] + config.text_rect[2] / 2.0;
    let box_cy = config.text_rect[1] + config.text_rect[3] / 2.0;
    let attach_x = if config.leader_knee.0 < config.text_rect[0] {
        config.text_rect[0]
    } else if config.leader_knee.0 > config.text_rect[0] + config.text_rect[2] {
        config.text_rect[0] + config.text_rect[2]
    } else {
        box_cx
    };
    let attach_y = if config.leader_knee.1 < config.text_rect[1] {
        config.text_rect[1]
    } else if config.leader_knee.1 > config.text_rect[1] + config.text_rect[3] {
        config.text_rect[1] + config.text_rect[3]
    } else {
        box_cy
    };

    // Build appearance stream
    let mut app_stream_content = String::new();
    app_stream_content.push_str("q\n");

    // Translate to local coords relative to (min_x, min_y)
    app_stream_content.push_str(&format!("1 0 0 1 {:.2} {:.2} cm\n", -min_x, -min_y));

    // 1. Leader Line from pointer to knee to box
    app_stream_content.push_str(&format!(
        "{:.3} {:.3} {:.3} RG\n1.2 w\n",
        config.border_color.0, config.border_color.1, config.border_color.2
    ));
    app_stream_content.push_str(&format!(
        "{:.2} {:.2} m {:.2} {:.2} l {:.2} {:.2} l S\n",
        config.leader_start.0,
        config.leader_start.1,
        config.leader_knee.0,
        config.leader_knee.1,
        attach_x,
        attach_y
    ));

    // Pointer dot at start
    app_stream_content.push_str(&format!(
        "{:.3} {:.3} {:.3} rg\n",
        config.border_color.0, config.border_color.1, config.border_color.2
    ));
    app_stream_content.push_str(&format!(
        "{:.2} {:.2} 4.0 4.0 re f\n",
        config.leader_start.0 - 2.0,
        config.leader_start.1 - 2.0
    ));

    // 2. Text Box background & border
    let bx = config.text_rect[0];
    let by = config.text_rect[1];
    let bw = config.text_rect[2];
    let bh = config.text_rect[3];

    if let Some(fill) = config.fill_color {
        app_stream_content.push_str(&format!("{:.3} {:.3} {:.3} rg\n", fill.0, fill.1, fill.2));
        app_stream_content.push_str(&format!("{:.2} {:.2} {:.2} {:.2} re f\n", bx, by, bw, bh));
    }

    app_stream_content.push_str(&format!(
        "{:.3} {:.3} {:.3} RG\n1.0 w\n",
        config.border_color.0, config.border_color.1, config.border_color.2
    ));
    app_stream_content.push_str(&format!("{:.2} {:.2} {:.2} {:.2} re S\n", bx, by, bw, bh));

    // 3. Text
    let tx = bx + 6.0;
    let ty = by + (bh - config.font_size) / 2.0 + 1.0;
    let escaped = escape_pdf_str(&config.text);
    app_stream_content.push_str(&format!(
        "BT\n/F1 {:.2} Tf\n{:.3} {:.3} {:.3} rg\n1 0 0 1 {:.2} {:.2} Tm\n({}) Tj\nET\nQ\n",
        config.font_size,
        config.text_color.0,
        config.text_color.1,
        config.text_color.2,
        tx,
        ty,
        escaped
    ));

    // Form XObject
    let app_obj_id = doc.next_object_id();
    let mut app_dict = PdfDictionary::new();
    app_dict.insert("Type", PdfName::new("XObject"));
    app_dict.insert("Subtype", PdfName::new("Form"));
    app_dict.insert(
        "BBox",
        vec![
            Object::Real(0.0),
            Object::Real(0.0),
            Object::Real(annot_w),
            Object::Real(annot_h),
        ],
    );

    let mut form_res = PdfDictionary::new();
    let mut form_fonts = PdfDictionary::new();
    let mut f1_dict = PdfDictionary::new();
    f1_dict.insert("Type", PdfName::new("Font"));
    f1_dict.insert("Subtype", PdfName::new("Type1"));
    f1_dict.insert("BaseFont", PdfName::new("Helvetica"));
    form_fonts.insert("F1", f1_dict);
    form_res.insert("Font", form_fonts);
    app_dict.insert("Resources", form_res);

    let mut app_stream = PdfStream::with_plain_data(app_dict, app_stream_content.into_bytes());
    let _ = app_stream.compress_flate();
    doc.objects.insert(app_obj_id, Object::Stream(app_stream));

    // FreeText Callout Annotation
    let annot_id = doc.next_object_id();
    let mut annot_dict = PdfDictionary::new();
    annot_dict.insert("Type", PdfName::new("Annot"));
    annot_dict.insert("Subtype", PdfName::new("FreeText"));
    annot_dict.insert("IT", PdfName::new("FreeTextCallout"));
    annot_dict.insert(
        "Rect",
        vec![
            Object::Real(min_x),
            Object::Real(min_y),
            Object::Real(min_x + annot_w),
            Object::Real(min_y + annot_h),
        ],
    );
    // Callout Line Coordinates: [x1 y1 x2 y2 x3 y3]
    annot_dict.insert(
        "CL",
        vec![
            Object::Real(config.leader_start.0),
            Object::Real(config.leader_start.1),
            Object::Real(config.leader_knee.0),
            Object::Real(config.leader_knee.1),
            Object::Real(attach_x),
            Object::Real(attach_y),
        ],
    );
    annot_dict.insert("Contents", PdfString::from_str(&config.text));

    let mut ap_dict = PdfDictionary::new();
    ap_dict.insert("N", app_obj_id);
    annot_dict.insert("AP", ap_dict);

    if let Some(ref auth) = config.author {
        annot_dict.insert("T", PdfString::from_str(auth));
    }

    doc.objects.insert(annot_id, Object::Dictionary(annot_dict));

    // Link into page /Annots
    let page_obj = doc
        .objects
        .get_mut(&page_id)
        .ok_or(PdfError::ObjectNotFound(page_id.number, page_id.generation))?;
    let p_dict = page_obj
        .as_dict_mut()
        .ok_or_else(|| PdfError::InvalidStructure("Expected page dictionary".into()))?;

    match p_dict.get_mut("Annots") {
        Some(Object::Array(arr)) => arr.push(Object::Reference(annot_id)),
        Some(Object::Reference(orig)) => {
            let o = *orig;
            p_dict.insert(
                "Annots",
                vec![Object::Reference(o), Object::Reference(annot_id)],
            );
        }
        _ => {
            p_dict.insert("Annots", vec![Object::Reference(annot_id)]);
        }
    }

    Ok(annot_id)
}

/// Configuration for a Typewriter annotation placed directly on the page.
///
/// Traceability: FR-08.1.4.
#[derive(Debug, Clone)]
pub struct TypewriterConfig {
    /// Insertion coordinate `(x, y)` in PDF points
    pub pos: (f64, f64),
    /// Typewritten text
    pub text: String,
    /// Font size in points (default 12.0)
    pub font_size: f64,
    /// Font color as RGB
    pub color: (f32, f32, f32),
    /// Author
    pub author: Option<String>,
}

impl Default for TypewriterConfig {
    fn default() -> Self {
        Self {
            pos: (72.0, 700.0),
            text: "Typewritten text".to_string(),
            font_size: 12.0,
            color: (0.0, 0.0, 0.0),
            author: None,
        }
    }
}

/// Adds a Typewriter annotation (`/IT /FreeTextTypewriter`) onto a page conforming to ISO 32000-1 Clause 12.5.6.6.
///
/// Traceability: FR-08.1.4.
pub fn add_typewriter_annotation(
    doc: &mut PdfDocument,
    page_index: usize,
    config: &TypewriterConfig,
) -> Result<ObjectId> {
    let page_ids = doc.page_ids();
    let page_id = *page_ids
        .get(page_index)
        .ok_or_else(|| PdfError::InvalidStructure(format!("Page {} out of range", page_index)))?;

    let est_w = config.text.len() as f64 * (config.font_size * 0.6) + 10.0;
    let est_h = config.font_size * 1.5;

    let escaped = escape_pdf_str(&config.text);
    let app_stream_content = format!(
        "q\nBT\n/F1 {:.2} Tf\n{:.3} {:.3} {:.3} rg\n1 0 0 1 2.0 3.0 Tm\n({}) Tj\nET\nQ\n",
        config.font_size, config.color.0, config.color.1, config.color.2, escaped
    );

    let app_obj_id = doc.next_object_id();
    let mut app_dict = PdfDictionary::new();
    app_dict.insert("Type", PdfName::new("XObject"));
    app_dict.insert("Subtype", PdfName::new("Form"));
    app_dict.insert(
        "BBox",
        vec![
            Object::Real(0.0),
            Object::Real(0.0),
            Object::Real(est_w),
            Object::Real(est_h),
        ],
    );

    let mut form_res = PdfDictionary::new();
    let mut form_fonts = PdfDictionary::new();
    let mut f1_dict = PdfDictionary::new();
    f1_dict.insert("Type", PdfName::new("Font"));
    f1_dict.insert("Subtype", PdfName::new("Type1"));
    f1_dict.insert("BaseFont", PdfName::new("Courier"));
    form_fonts.insert("F1", f1_dict);
    form_res.insert("Font", form_fonts);
    app_dict.insert("Resources", form_res);

    let mut app_stream = PdfStream::with_plain_data(app_dict, app_stream_content.into_bytes());
    let _ = app_stream.compress_flate();
    doc.objects.insert(app_obj_id, Object::Stream(app_stream));

    let annot_id = doc.next_object_id();
    let mut annot_dict = PdfDictionary::new();
    annot_dict.insert("Type", PdfName::new("Annot"));
    annot_dict.insert("Subtype", PdfName::new("FreeText"));
    annot_dict.insert("IT", PdfName::new("FreeTextTypewriter"));
    annot_dict.insert(
        "Rect",
        vec![
            Object::Real(config.pos.0),
            Object::Real(config.pos.1),
            Object::Real(config.pos.0 + est_w),
            Object::Real(config.pos.1 + est_h),
        ],
    );
    annot_dict.insert("Contents", PdfString::from_str(&config.text));

    let mut ap_dict = PdfDictionary::new();
    ap_dict.insert("N", app_obj_id);
    annot_dict.insert("AP", ap_dict);

    if let Some(ref auth) = config.author {
        annot_dict.insert("T", PdfString::from_str(auth));
    }

    doc.objects.insert(annot_id, Object::Dictionary(annot_dict));

    let page_obj = doc
        .objects
        .get_mut(&page_id)
        .ok_or(PdfError::ObjectNotFound(page_id.number, page_id.generation))?;
    let p_dict = page_obj
        .as_dict_mut()
        .ok_or_else(|| PdfError::InvalidStructure("Expected page dictionary".into()))?;

    match p_dict.get_mut("Annots") {
        Some(Object::Array(arr)) => arr.push(Object::Reference(annot_id)),
        Some(Object::Reference(orig)) => {
            let o = *orig;
            p_dict.insert(
                "Annots",
                vec![Object::Reference(o), Object::Reference(annot_id)],
            );
        }
        _ => {
            p_dict.insert("Annots", vec![Object::Reference(annot_id)]);
        }
    }

    Ok(annot_id)
}

fn escape_pdf_str(s: &str) -> String {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_standard_and_dynamic_stamps() {
        let mut doc = PdfDocument::new();
        doc.add_blank_page(612.0, 792.0);

        // 1. Add standard Approved stamp
        let config = StampConfig {
            stamp_type: StandardStamp::Approved,
            rect: [100.0, 600.0, 180.0, 56.0],
            ..Default::default()
        };
        let annot_id = add_stamp_annotation(&mut doc, 0, &config).expect("add approved stamp");
        assert!(doc.objects.contains_key(&annot_id));

        // 2. Add dynamic Confidential stamp with date and author
        let dyn_config = StampConfig {
            stamp_type: StandardStamp::Confidential,
            rect: [100.0, 500.0, 220.0, 60.0],
            author: Some("SecurityOfficer".to_string()),
            dynamic_date: Some(Utc::now()),
            popup_note: Some("Classified review complete".to_string()),
            ..Default::default()
        };

        let dyn_id =
            add_stamp_annotation(&mut doc, 0, &dyn_config).expect("add dynamic confidential stamp");
        assert!(doc.objects.contains_key(&dyn_id));

        let dyn_obj = doc.objects.get(&dyn_id).unwrap().as_dict().unwrap();
        assert_eq!(dyn_obj.get_name("Subtype"), Some("Stamp"));
        assert_eq!(dyn_obj.get_name("Name"), Some("Confidential"));
        assert_eq!(
            dyn_obj.get_str("Contents").as_deref(),
            Some("Classified review complete")
        );
        assert_eq!(dyn_obj.get_str("T").as_deref(), Some("SecurityOfficer"));

        // Verify Appearance Stream Form XObject was attached
        let ap = dyn_obj.get_dict("AP").expect("AP dict");
        assert!(ap.contains_key("N"));
    }

    #[test]
    fn test_callout_and_typewriter_annotations() {
        let mut doc = PdfDocument::new();
        doc.add_blank_page(612.0, 792.0);

        // 1. Add Callout annotation (FR-08.1.3)
        let callout = CalloutConfig {
            text_rect: [250.0, 450.0, 180.0, 40.0],
            text: "Important clause here".to_string(),
            leader_start: (150.0, 550.0),
            leader_knee: (200.0, 470.0),
            border_color: (0.8, 0.2, 0.2),
            fill_color: Some((1.0, 0.95, 0.95)),
            text_color: (0.1, 0.1, 0.1),
            font_size: 10.0,
            author: Some("LeadReviewer".to_string()),
        };

        let callout_id =
            add_callout_annotation(&mut doc, 0, &callout).expect("add callout annotation");
        let callout_obj = doc.objects.get(&callout_id).unwrap().as_dict().unwrap();
        assert_eq!(callout_obj.get_name("Subtype"), Some("FreeText"));
        assert_eq!(callout_obj.get_name("IT"), Some("FreeTextCallout"));
        assert!(callout_obj.contains_key("CL"));

        // 2. Add Typewriter annotation (FR-08.1.4)
        let typewriter = TypewriterConfig {
            pos: (72.0, 720.0),
            text: "Certified Copy - Document Ref #4092".to_string(),
            font_size: 11.0,
            color: (0.0, 0.0, 0.0),
            author: Some("Clerk".to_string()),
        };

        let tw_id =
            add_typewriter_annotation(&mut doc, 0, &typewriter).expect("add typewriter annotation");
        let tw_obj = doc.objects.get(&tw_id).unwrap().as_dict().unwrap();
        assert_eq!(tw_obj.get_name("Subtype"), Some("FreeText"));
        assert_eq!(tw_obj.get_name("IT"), Some("FreeTextTypewriter"));
        assert_eq!(
            tw_obj.get_str("Contents").as_deref(),
            Some("Certified Copy - Document Ref #4092")
        );
    }
}
