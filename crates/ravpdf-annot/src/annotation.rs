//! PDF Annotations conforming to ISO 32000-1 Section 12.5.
//!
//! Traceability: FR-08.1-3, FR-09.1-4, FR-10.1-4.

use ravpdf_core::document::PdfDocument;
use ravpdf_core::error::{PdfError, Result};
use ravpdf_core::object::{Object, ObjectId, PdfDictionary, PdfName, PdfString};
use regex::Regex;

/// Action associated with a link annotation conforming to ISO 32000-1 Section 12.6.
///
/// Traceability: FR-20.2.
#[derive(Debug, Clone, PartialEq)]
pub enum LinkAction {
    /// External web URI (/S /URI /URI (https://...))
    Uri(String),
    /// Jump to 0-based page index (/S /GoTo or /Dest)
    GoToPage(usize),
    /// Jump to named destination (/Dest (name))
    Named(String),
    /// Open file or application (/S /Launch /F (path))
    OpenFile(String),
    /// Execute embedded JavaScript (/S /JavaScript /JS (code))
    JavaScript(String),
    /// Submit interactive form data (/S /SubmitForm)
    SubmitForm { url: String, flags: u32 },
    /// Reset interactive form fields (/S /ResetForm)
    ResetForm { fields: Vec<String>, exclude: bool },
}

/// Visual effect when link annotation is activated conforming to ISO 32000-1 Section 12.5.6.5 Table 173.
///
/// Traceability: FR-20.6.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkHighlight {
    /// /N: No highlighting
    None,
    /// /I: Invert the contents of the annotation rectangle (default)
    Invert,
    /// /O: Invert the annotation's border
    Outline,
    /// /P: Push effect (display annotation as if being pressed)
    Push,
}

impl LinkHighlight {
    pub fn to_pdf_name(&self) -> &'static str {
        match self {
            LinkHighlight::None => "N",
            LinkHighlight::Invert => "I",
            LinkHighlight::Outline => "O",
            LinkHighlight::Push => "P",
        }
    }

    pub fn from_pdf_name(name: &str) -> Self {
        match name {
            "N" => LinkHighlight::None,
            "O" => LinkHighlight::Outline,
            "P" => LinkHighlight::Push,
            _ => LinkHighlight::Invert,
        }
    }
}

/// Subtypes of PDF annotations conforming to ISO 32000-1 Section 12.5.
#[derive(Debug, Clone, PartialEq)]
pub enum AnnotationType {
    Text,          // Sticky Note
    FreeText,      // Text box on page
    Highlight,     // Highlight
    Underline,     // Underline
    StrikeOut,     // Strikeout
    Square,        // Rectangle
    Circle,        // Oval / Circle
    Line,          // Line
    Ink,           // Freehand drawing
    Stamp(String), // Rubber stamp
    Link {
        action: LinkAction,
        border_width: f64,
        highlight: LinkHighlight,
    },
}

impl AnnotationType {
    pub fn subtype_name(&self) -> &'static str {
        match self {
            AnnotationType::Text => "Text",
            AnnotationType::FreeText => "FreeText",
            AnnotationType::Highlight => "Highlight",
            AnnotationType::Underline => "Underline",
            AnnotationType::StrikeOut => "StrikeOut",
            AnnotationType::Square => "Square",
            AnnotationType::Circle => "Circle",
            AnnotationType::Line => "Line",
            AnnotationType::Ink => "Ink",
            AnnotationType::Stamp(_) => "Stamp",
            AnnotationType::Link { .. } => "Link",
        }
    }

    pub fn from_subtype_name(name: &str, stamp_name: Option<String>) -> Option<Self> {
        match name {
            "Text" => Some(AnnotationType::Text),
            "FreeText" => Some(AnnotationType::FreeText),
            "Highlight" => Some(AnnotationType::Highlight),
            "Underline" => Some(AnnotationType::Underline),
            "StrikeOut" => Some(AnnotationType::StrikeOut),
            "Square" => Some(AnnotationType::Square),
            "Circle" => Some(AnnotationType::Circle),
            "Line" => Some(AnnotationType::Line),
            "Ink" => Some(AnnotationType::Ink),
            "Stamp" => Some(AnnotationType::Stamp(
                stamp_name.unwrap_or_else(|| "Draft".into()),
            )),
            _ => None,
        }
    }
}

/// Structured representation of a PDF annotation.
#[derive(Debug, Clone, PartialEq)]
pub struct Annotation {
    pub id: Option<ObjectId>,
    pub annot_type: AnnotationType,
    pub rect: [f64; 4],
    pub contents: Option<String>,
    pub color: Option<(f32, f32, f32)>,
    pub opacity: f32,
    pub author: Option<String>,
}

impl Annotation {
    pub fn new(annot_type: AnnotationType, rect: [f64; 4]) -> Self {
        Self {
            id: None,
            annot_type,
            rect,
            contents: None,
            color: Some((1.0, 1.0, 0.0)), // Default yellow
            opacity: 1.0,
            author: None,
        }
    }

    /// Creates a clickable web hyperlink annotation (FR-20.1, FR-20.2).
    pub fn new_link_uri(rect: [f64; 4], uri: impl Into<String>) -> Self {
        Self {
            id: None,
            annot_type: AnnotationType::Link {
                action: LinkAction::Uri(uri.into()),
                border_width: 0.0,
                highlight: LinkHighlight::Invert,
            },
            rect,
            contents: None,
            color: Some((0.0, 0.45, 0.85)),
            opacity: 1.0,
            author: None,
        }
    }

    /// Creates an internal page navigation link annotation (FR-20.1, FR-20.2).
    pub fn new_link_goto(rect: [f64; 4], target_page: usize) -> Self {
        Self {
            id: None,
            annot_type: AnnotationType::Link {
                action: LinkAction::GoToPage(target_page),
                border_width: 0.0,
                highlight: LinkHighlight::Invert,
            },
            rect,
            contents: None,
            color: Some((0.0, 0.45, 0.85)),
            opacity: 1.0,
            author: None,
        }
    }

    /// Creates a named destination link annotation (FR-20.2, FR-20.4).
    pub fn new_link_named(rect: [f64; 4], name: impl Into<String>) -> Self {
        Self {
            id: None,
            annot_type: AnnotationType::Link {
                action: LinkAction::Named(name.into()),
                border_width: 0.0,
                highlight: LinkHighlight::Invert,
            },
            rect,
            contents: None,
            color: Some((0.0, 0.45, 0.85)),
            opacity: 1.0,
            author: None,
        }
    }

    /// Creates an open/launch file link annotation (FR-20.2).
    pub fn new_link_launch(rect: [f64; 4], path: impl Into<String>) -> Self {
        Self {
            id: None,
            annot_type: AnnotationType::Link {
                action: LinkAction::OpenFile(path.into()),
                border_width: 0.0,
                highlight: LinkHighlight::Invert,
            },
            rect,
            contents: None,
            color: Some((0.0, 0.45, 0.85)),
            opacity: 1.0,
            author: None,
        }
    }

    /// Creates an execute JavaScript link annotation (FR-20.2, FR-12).
    pub fn new_link_javascript(rect: [f64; 4], code: impl Into<String>) -> Self {
        Self {
            id: None,
            annot_type: AnnotationType::Link {
                action: LinkAction::JavaScript(code.into()),
                border_width: 0.0,
                highlight: LinkHighlight::Invert,
            },
            rect,
            contents: None,
            color: Some((0.0, 0.45, 0.85)),
            opacity: 1.0,
            author: None,
        }
    }

    /// Creates a submit form link annotation (FR-20.2, FR-11.8).
    pub fn new_link_submit_form(rect: [f64; 4], url: impl Into<String>, flags: u32) -> Self {
        Self {
            id: None,
            annot_type: AnnotationType::Link {
                action: LinkAction::SubmitForm {
                    url: url.into(),
                    flags,
                },
                border_width: 0.0,
                highlight: LinkHighlight::Invert,
            },
            rect,
            contents: None,
            color: Some((0.0, 0.45, 0.85)),
            opacity: 1.0,
            author: None,
        }
    }

    /// Creates a reset form link annotation (FR-20.2, FR-11.8).
    pub fn new_link_reset_form(rect: [f64; 4], fields: Vec<String>, exclude: bool) -> Self {
        Self {
            id: None,
            annot_type: AnnotationType::Link {
                action: LinkAction::ResetForm { fields, exclude },
                border_width: 0.0,
                highlight: LinkHighlight::Invert,
            },
            rect,
            contents: None,
            color: Some((0.0, 0.45, 0.85)),
            opacity: 1.0,
            author: None,
        }
    }

    /// Serializes this annotation into an ISO 32000-1 Section 12.5 annotation dictionary.
    pub fn to_dictionary(&self) -> PdfDictionary {
        let mut dict = PdfDictionary::new();
        dict.insert("Type", PdfName::new("Annot"));
        dict.insert("Subtype", PdfName::new(self.annot_type.subtype_name()));
        dict.insert(
            "Rect",
            vec![
                Object::Real(self.rect[0]),
                Object::Real(self.rect[1]),
                Object::Real(self.rect[2]),
                Object::Real(self.rect[3]),
            ],
        );

        if let Some(ref text) = self.contents {
            dict.insert("Contents", PdfString::from_str(text));
        }

        if let Some((r, g, b)) = self.color {
            dict.insert(
                "C",
                vec![
                    Object::Real(r as f64),
                    Object::Real(g as f64),
                    Object::Real(b as f64),
                ],
            );
        }

        if (self.opacity - 1.0).abs() > 1e-4 {
            dict.insert("CA", Object::Real(self.opacity as f64));
        }

        if let Some(ref auth) = self.author {
            dict.insert("T", PdfString::from_str(auth));
        }

        if let AnnotationType::Stamp(ref stamp_name) = self.annot_type {
            dict.insert("Name", PdfName::new(stamp_name.as_str()));
        }

        if let AnnotationType::Link {
            ref action,
            border_width,
            highlight,
        } = self.annot_type
        {
            dict.insert("H", PdfName::new(highlight.to_pdf_name()));
            dict.insert(
                "Border",
                vec![
                    Object::Real(0.0),
                    Object::Real(0.0),
                    Object::Real(border_width),
                ],
            );
            match action {
                LinkAction::Uri(ref uri) => {
                    let mut action_dict = PdfDictionary::new();
                    action_dict.insert("Type", PdfName::new("Action"));
                    action_dict.insert("S", PdfName::new("URI"));
                    action_dict.insert("URI", PdfString::from_str(uri));
                    dict.insert("A", Object::Dictionary(action_dict));
                }
                LinkAction::GoToPage(target_page) => {
                    let mut action_dict = PdfDictionary::new();
                    action_dict.insert("Type", PdfName::new("Action"));
                    action_dict.insert("S", PdfName::new("GoTo"));
                    action_dict.insert(
                        "D",
                        vec![
                            Object::Integer(*target_page as i64),
                            Object::Name(PdfName::new("Fit")),
                        ],
                    );
                    dict.insert("A", Object::Dictionary(action_dict));
                }
                LinkAction::Named(ref name) => {
                    dict.insert("Dest", PdfString::from_str(name));
                }
                LinkAction::OpenFile(ref path) => {
                    let mut action_dict = PdfDictionary::new();
                    action_dict.insert("Type", PdfName::new("Action"));
                    action_dict.insert("S", PdfName::new("Launch"));
                    action_dict.insert("F", PdfString::from_str(path));
                    dict.insert("A", Object::Dictionary(action_dict));
                }
                LinkAction::JavaScript(ref code) => {
                    let mut action_dict = PdfDictionary::new();
                    action_dict.insert("Type", PdfName::new("Action"));
                    action_dict.insert("S", PdfName::new("JavaScript"));
                    action_dict.insert("JS", PdfString::from_str(code));
                    dict.insert("A", Object::Dictionary(action_dict));
                }
                LinkAction::SubmitForm { ref url, flags } => {
                    let mut action_dict = PdfDictionary::new();
                    action_dict.insert("Type", PdfName::new("Action"));
                    action_dict.insert("S", PdfName::new("SubmitForm"));
                    let mut f_dict = PdfDictionary::new();
                    f_dict.insert("FS", PdfName::new("URL"));
                    f_dict.insert("F", PdfString::from_str(url));
                    action_dict.insert("F", Object::Dictionary(f_dict));
                    action_dict.insert("Flags", Object::Integer(*flags as i64));
                    dict.insert("A", Object::Dictionary(action_dict));
                }
                LinkAction::ResetForm { ref fields, exclude } => {
                    let mut action_dict = PdfDictionary::new();
                    action_dict.insert("Type", PdfName::new("Action"));
                    action_dict.insert("S", PdfName::new("ResetForm"));
                    if !fields.is_empty() {
                        let arr = fields
                            .iter()
                            .map(|f| Object::String(PdfString::from_str(f)))
                            .collect();
                        action_dict.insert("Fields", Object::Array(arr));
                    }
                    action_dict.insert("Flags", Object::Integer(if *exclude { 1 } else { 0 }));
                    dict.insert("A", Object::Dictionary(action_dict));
                }
            }
        }

        dict
    }

    /// Deserializes an annotation from a PDF dictionary.
    pub fn from_dictionary(id: ObjectId, dict: &PdfDictionary) -> Option<Self> {
        let subtype = dict.get_name("Subtype")?;
        let stamp_name = dict.get_name("Name").map(|s| s.to_string());

        let annot_type = if subtype == "Link" {
            let highlight = dict
                .get_name("H")
                .map(LinkHighlight::from_pdf_name)
                .unwrap_or(LinkHighlight::Invert);

            let border_width = dict
                .get_array("Border")
                .and_then(|arr| arr.get(2))
                .and_then(Object::as_f64)
                .unwrap_or(0.0);

            let action = if let Some(Object::Dictionary(a_dict)) = dict.get("A") {
                let s = a_dict.get_name("S").unwrap_or("");
                if s == "URI" {
                    LinkAction::Uri(a_dict.get_str("URI").unwrap_or_default())
                } else if s == "GoTo" {
                    let page_idx = if let Some(Object::Array(d_arr)) = a_dict.get("D") {
                        d_arr.first().and_then(Object::as_i64).unwrap_or(0).max(0) as usize
                    } else {
                        0
                    };
                    LinkAction::GoToPage(page_idx)
                } else if s == "Launch" {
                    LinkAction::OpenFile(a_dict.get_str("F").unwrap_or_default())
                } else if s == "JavaScript" {
                    LinkAction::JavaScript(a_dict.get_str("JS").unwrap_or_default())
                } else if s == "SubmitForm" {
                    let url = if let Some(Object::Dictionary(f_dict)) = a_dict.get("F") {
                        f_dict.get_str("F").unwrap_or_default()
                    } else {
                        a_dict.get_str("F").unwrap_or_default()
                    };
                    let flags = a_dict.get_i64("Flags").unwrap_or(0).max(0) as u32;
                    LinkAction::SubmitForm { url, flags }
                } else if s == "ResetForm" {
                    let fields = a_dict
                        .get_array("Fields")
                        .map(|arr| {
                            arr.iter()
                                .filter_map(Object::as_str)
                                .map(|s| s.to_string())
                                .collect()
                        })
                        .unwrap_or_default();
                    let flags = a_dict.get_i64("Flags").unwrap_or(0);
                    let exclude = (flags & 1) != 0;
                    LinkAction::ResetForm { fields, exclude }
                } else {
                    LinkAction::Uri("https://".into())
                }
            } else if let Some(dest) = dict.get("Dest") {
                match dest {
                    Object::Array(d_arr) => {
                        let page_idx =
                            d_arr.first().and_then(Object::as_i64).unwrap_or(0).max(0) as usize;
                        LinkAction::GoToPage(page_idx)
                    }
                    Object::String(s) => LinkAction::Named(s.to_string_lossy()),
                    Object::Name(n) => LinkAction::Named(n.as_str().to_string()),
                    _ => LinkAction::Uri("https://".into()),
                }
            } else {
                LinkAction::Uri("https://".into())
            };

            AnnotationType::Link {
                action,
                border_width,
                highlight,
            }
        } else {
            AnnotationType::from_subtype_name(subtype, stamp_name)?
        };

        let rect_arr = dict.get_array("Rect")?;
        if rect_arr.len() < 4 {
            return None;
        }

        let rect = [
            rect_arr[0].as_f64()?,
            rect_arr[1].as_f64()?,
            rect_arr[2].as_f64()?,
            rect_arr[3].as_f64()?,
        ];

        let contents = dict.get_str("Contents");
        let author = dict.get_str("T");
        let opacity = dict.get_f64("CA").unwrap_or(1.0) as f32;

        let color = if let Some(c_arr) = dict.get_array("C") {
            if c_arr.len() >= 3 {
                Some((
                    c_arr[0].as_f64().unwrap_or(0.0) as f32,
                    c_arr[1].as_f64().unwrap_or(0.0) as f32,
                    c_arr[2].as_f64().unwrap_or(0.0) as f32,
                ))
            } else {
                None
            }
        } else {
            None
        };

        Some(Self {
            id: Some(id),
            annot_type,
            rect,
            contents,
            color,
            opacity,
            author,
        })
    }
}

/// Automatically detects HTTP/HTTPS URLs within text along with their character span (start, end).
///
/// Traceability: FR-20.5.
pub fn detect_urls_in_text(text: &str) -> Vec<(String, usize, usize)> {
    let Ok(re) = Regex::new(r#"https?://[^\s<>"'{}|\\^`]+[^\s`!()\[\]{};:'".,<>?«»“”‘’]"#)
    else {
        return Vec::new();
    };

    re.find_iter(text)
        .map(|m| (m.as_str().to_string(), m.start(), m.end()))
        .collect()
}

/// Retrieves all annotations attached to the specified page.
pub fn get_page_annotations(doc: &PdfDocument, page_index: usize) -> Result<Vec<Annotation>> {
    let page_ids = doc.page_ids();
    let page_id = *page_ids
        .get(page_index)
        .ok_or_else(|| PdfError::InvalidStructure(format!("Page {} out of range", page_index)))?;

    let page_dict = doc
        .objects
        .get(&page_id)
        .and_then(Object::as_dict)
        .ok_or_else(|| PdfError::InvalidStructure("Missing page dict".into()))?;

    let mut annotations = Vec::new();

    if let Some(annots_arr) = page_dict.get_array("Annots") {
        for item in annots_arr {
            if let Some(annot_id) = item.as_reference() {
                if let Some(annot_dict) = doc.objects.get(&annot_id).and_then(Object::as_dict) {
                    if let Some(annot) = Annotation::from_dictionary(annot_id, annot_dict) {
                        annotations.push(annot);
                    }
                }
            }
        }
    }

    Ok(annotations)
}

/// Attaches a new annotation to the specified page.
pub fn add_annotation_to_page(
    doc: &mut PdfDocument,
    page_index: usize,
    annot: &Annotation,
) -> Result<ObjectId> {
    let page_ids = doc.page_ids();
    let page_id = *page_ids
        .get(page_index)
        .ok_or_else(|| PdfError::InvalidStructure(format!("Page {} out of range", page_index)))?;

    let annot_id = doc.next_object_id();
    let annot_dict = annot.to_dictionary();
    doc.objects.insert(annot_id, Object::Dictionary(annot_dict));

    // Append to page's /Annots array
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

    Ok(annot_id)
}

/// Deletes an annotation from the specified page and document object graph (FR-08.3).
///
/// Also cleans up any linked `/Popup` annotation object.
pub fn delete_annotation(
    doc: &mut PdfDocument,
    page_index: usize,
    annot_id: ObjectId,
) -> Result<()> {
    let page_ids = doc.page_ids();
    let page_id = *page_ids
        .get(page_index)
        .ok_or_else(|| PdfError::InvalidStructure(format!("Page {} out of range", page_index)))?;

    // 1. Check for linked popup annotation before removal
    let popup_id = if let Some(Object::Dictionary(dict)) = doc.objects.get(&annot_id) {
        if let Some(Object::Reference(p_id)) = dict.get("Popup") {
            Some(*p_id)
        } else {
            None
        }
    } else {
        None
    };

    // 2. Remove reference from page's /Annots array
    let page_obj = doc
        .objects
        .get_mut(&page_id)
        .ok_or(PdfError::ObjectNotFound(page_id.number, page_id.generation))?;

    let page_dict = page_obj
        .as_dict_mut()
        .ok_or_else(|| PdfError::InvalidStructure("Expected page dictionary".into()))?;

    if let Some(Object::Array(ref mut arr)) = page_dict.get_mut("Annots") {
        arr.retain(|item| {
            if let Some(r) = item.as_reference() {
                r != annot_id && Some(r) != popup_id
            } else {
                true
            }
        });
    }

    // 3. Remove the annotation object and any linked popup object from doc.objects
    doc.objects.remove(&annot_id);
    if let Some(p_id) = popup_id {
        doc.objects.remove(&p_id);
    }

    Ok(())
}

/// Updates the bounding rectangle of an existing annotation (FR-08.3, FR-09.12).
///
/// Modifies the `/Rect` entry conforming to ISO 32000-1 Clause 12.5.2 and updates modification timestamp `/M`.
pub fn update_annotation_rect(
    doc: &mut PdfDocument,
    annot_id: ObjectId,
    new_rect: [f64; 4],
) -> Result<()> {
    let annot_obj = doc
        .objects
        .get_mut(&annot_id)
        .ok_or(PdfError::ObjectNotFound(
            annot_id.number,
            annot_id.generation,
        ))?;

    let dict = annot_obj
        .as_dict_mut()
        .ok_or_else(|| PdfError::InvalidStructure("Expected annotation dictionary".into()))?;

    // Standardize rect coordinates [x1, y1, x2, y2]
    let min_x = new_rect[0].min(new_rect[2]);
    let min_y = new_rect[1].min(new_rect[3]);
    let max_x = new_rect[0].max(new_rect[2]);
    let max_y = new_rect[1].max(new_rect[3]);

    dict.insert(
        "Rect",
        Object::Array(vec![
            Object::Real(min_x),
            Object::Real(min_y),
            Object::Real(max_x),
            Object::Real(max_y),
        ]),
    );

    // Update /M date
    let now_str = chrono::Utc::now().format("D:%Y%m%d%H%M%SZ").to_string();
    dict.insert("M", Object::String(PdfString::from_str(&now_str)));

    Ok(())
}

/// Updates textual content and author of an existing annotation (FR-08.1, FR-08.3).
pub fn update_annotation_contents(
    doc: &mut PdfDocument,
    annot_id: ObjectId,
    contents: &str,
    author: Option<&str>,
) -> Result<()> {
    let annot_obj = doc
        .objects
        .get_mut(&annot_id)
        .ok_or(PdfError::ObjectNotFound(
            annot_id.number,
            annot_id.generation,
        ))?;

    let dict = annot_obj
        .as_dict_mut()
        .ok_or_else(|| PdfError::InvalidStructure("Expected annotation dictionary".into()))?;

    dict.insert("Contents", Object::String(PdfString::from_str(contents)));

    if let Some(auth) = author {
        dict.insert("T", Object::String(PdfString::from_str(auth)));
    }

    let now_str = chrono::Utc::now().format("D:%Y%m%d%H%M%SZ").to_string();
    dict.insert("M", Object::String(PdfString::from_str(&now_str)));

    Ok(())
}

/// Updates stroke/fill color and opacity of an existing annotation (FR-08.3, FR-09.11).
pub fn update_annotation_color(
    doc: &mut PdfDocument,
    annot_id: ObjectId,
    color: (f32, f32, f32),
    opacity: Option<f32>,
) -> Result<()> {
    let annot_obj = doc
        .objects
        .get_mut(&annot_id)
        .ok_or(PdfError::ObjectNotFound(
            annot_id.number,
            annot_id.generation,
        ))?;

    let dict = annot_obj
        .as_dict_mut()
        .ok_or_else(|| PdfError::InvalidStructure("Expected annotation dictionary".into()))?;

    dict.insert(
        "C",
        Object::Array(vec![
            Object::Real(color.0 as f64),
            Object::Real(color.1 as f64),
            Object::Real(color.2 as f64),
        ]),
    );

    if let Some(ca) = opacity {
        dict.insert("CA", Object::Real(ca.clamp(0.0, 1.0) as f64));
    }

    let now_str = chrono::Utc::now().format("D:%Y%m%d%H%M%SZ").to_string();
    dict.insert("M", Object::String(PdfString::from_str(&now_str)));

    Ok(())
}

/// Exports all annotations across all pages of a document into ISO 19444-1 compliant XFDF XML (FR-08.3.6).
pub fn export_annotations_to_xfdf(doc: &PdfDocument) -> Result<String> {
    let mut xml = String::new();
    xml.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    xml.push_str("<xfdf xmlns=\"http://ns.adobe.com/xfdf/\" xml:space=\"preserve\">\n");
    xml.push_str("  <annots>\n");

    let total_pages = doc.page_count();
    for page_idx in 0..total_pages {
        let annots = get_page_annotations(doc, page_idx).unwrap_or_default();
        for annot in annots {
            let tag = match annot.annot_type {
                AnnotationType::Text => "text",
                AnnotationType::Highlight => "highlight",
                AnnotationType::Underline => "underline",
                AnnotationType::StrikeOut => "strikeout",
                AnnotationType::FreeText => "freetext",
                AnnotationType::Square => "square",
                AnnotationType::Circle => "circle",
                AnnotationType::Line => "line",
                AnnotationType::Ink => "ink",
                AnnotationType::Stamp(_) => "stamp",
                AnnotationType::Link { .. } => "link",
            };

            let rect_str = format!(
                "{:.2},{:.2},{:.2},{:.2}",
                annot.rect[0], annot.rect[1], annot.rect[2], annot.rect[3]
            );

            let color_str = if let Some(c) = annot.color {
                format!(
                    "#{:02X}{:02X}{:02X}",
                    (c.0.clamp(0.0, 1.0) * 255.0).round() as u8,
                    (c.1.clamp(0.0, 1.0) * 255.0).round() as u8,
                    (c.2.clamp(0.0, 1.0) * 255.0).round() as u8
                )
            } else {
                "#000000".to_string()
            };

            let author_attr = annot
                .author
                .as_deref()
                .map(|a| format!(" title=\"{}\"", escape_xml(a)))
                .unwrap_or_default();

            xml.push_str(&format!(
                "    <{} page=\"{}\" rect=\"{}\" color=\"{}\" opacity=\"{:.2}\"{}>\n",
                tag, page_idx, rect_str, color_str, annot.opacity, author_attr
            ));

            if let Some(ref text) = annot.contents {
                xml.push_str(&format!(
                    "      <contents>{}</contents>\n",
                    escape_xml(text)
                ));
            }

            xml.push_str(&format!("    </{}>\n", tag));
        }
    }

    xml.push_str("  </annots>\n");
    xml.push_str("</xfdf>\n");
    Ok(xml)
}

/// Imports annotations from an XFDF XML string and applies them to the document (FR-08.3.6).
///
/// Returns the number of annotations successfully imported.
pub fn import_annotations_from_xfdf(doc: &mut PdfDocument, xfdf_content: &str) -> Result<usize> {
    let mut imported = 0;
    let total_pages = doc.page_count();
    if total_pages == 0 {
        return Ok(0);
    }

    let supported_tags = [
        "text",
        "highlight",
        "underline",
        "strikeout",
        "freetext",
        "square",
        "circle",
        "line",
        "ink",
        "stamp",
        "link",
    ];

    for tag in supported_tags {
        let open_pat = format!("<{}", tag);
        let close_pat = format!("</{}>", tag);

        let mut search_idx = 0;
        while let Some(start) = xfdf_content[search_idx..].find(&open_pat) {
            let abs_start = search_idx + start;
            let end_opt = xfdf_content[abs_start..].find(&close_pat);
            let single_tag_end = xfdf_content[abs_start..].find("/>");

            let block = if let Some(end) = end_opt {
                let abs_end = abs_start + end + close_pat.len();
                search_idx = abs_end;
                &xfdf_content[abs_start..abs_end]
            } else if let Some(end) = single_tag_end {
                let abs_end = abs_start + end + 2;
                search_idx = abs_end;
                &xfdf_content[abs_start..abs_end]
            } else {
                break;
            };

            // Parse page
            let page: usize = extract_xml_attr(block, "page")
                .and_then(|s| s.parse().ok())
                .unwrap_or(0)
                .min(total_pages - 1);

            // Parse rect
            let rect = if let Some(rect_s) = extract_xml_attr(block, "rect") {
                let parts: Vec<f64> = rect_s
                    .split(',')
                    .filter_map(|p| p.trim().parse::<f64>().ok())
                    .collect();
                if parts.len() >= 4 {
                    [parts[0], parts[1], parts[2], parts[3]]
                } else {
                    [50.0, 50.0, 150.0, 150.0]
                }
            } else {
                [50.0, 50.0, 150.0, 150.0]
            };

            // Parse color
            let color = extract_xml_attr(block, "color").and_then(|hex| {
                let hex = hex.trim_start_matches('#');
                if hex.len() == 6 {
                    let r = u8::from_str_radix(&hex[0..2], 16).ok()? as f32 / 255.0;
                    let g = u8::from_str_radix(&hex[2..4], 16).ok()? as f32 / 255.0;
                    let b = u8::from_str_radix(&hex[4..6], 16).ok()? as f32 / 255.0;
                    Some((r, g, b))
                } else {
                    None
                }
            });

            // Parse opacity
            let opacity: f32 = extract_xml_attr(block, "opacity")
                .and_then(|s| s.parse().ok())
                .unwrap_or(1.0);

            // Parse title / author
            let author = extract_xml_attr(block, "title").map(unescape_xml);

            // Parse contents
            let contents = extract_xml_inner(block, "contents").map(unescape_xml);

            let annot_type = match tag {
                "text" => AnnotationType::Text,
                "highlight" => AnnotationType::Highlight,
                "underline" => AnnotationType::Underline,
                "strikeout" => AnnotationType::StrikeOut,
                "freetext" => AnnotationType::FreeText,
                "square" => AnnotationType::Square,
                "circle" => AnnotationType::Circle,
                "line" => AnnotationType::Line,
                "ink" => AnnotationType::Ink,
                "stamp" => AnnotationType::Stamp("Approved".into()),
                _ => AnnotationType::Text,
            };

            let mut annot = Annotation::new(annot_type, rect);
            annot.color = color;
            annot.opacity = opacity;
            annot.author = author;
            annot.contents = contents;

            if add_annotation_to_page(doc, page, &annot).is_ok() {
                imported += 1;
            }
        }
    }

    Ok(imported)
}

fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn unescape_xml(s: &str) -> String {
    s.replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}

fn extract_xml_attr<'a>(tag_str: &'a str, attr_name: &str) -> Option<&'a str> {
    let pat = format!("{}=\"", attr_name);
    let start = tag_str.find(&pat)? + pat.len();
    let rest = &tag_str[start..];
    let end = rest.find('"')?;
    Some(&rest[..end])
}

fn extract_xml_inner<'a>(block: &'a str, tag_name: &str) -> Option<&'a str> {
    let open = format!("<{}>", tag_name);
    let close = format!("</{}>", tag_name);
    let start = block.find(&open)? + open.len();
    let rest = &block[start..];
    let end = rest.find(&close)?;
    Some(rest[..end].trim())
}
