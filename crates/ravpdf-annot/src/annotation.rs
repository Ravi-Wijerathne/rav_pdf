//! PDF Annotations conforming to ISO 32000-1 Section 12.5.
//!
//! Traceability: FR-08.1-3, FR-09.1-4, FR-10.1-4.

use ravpdf_core::document::PdfDocument;
use ravpdf_core::error::{PdfError, Result};
use ravpdf_core::object::{ObjectId, Object, PdfDictionary, PdfName, PdfString};
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
    Text,        // Sticky Note
    FreeText,    // Text box on page
    Highlight,   // Highlight
    Underline,   // Underline
    StrikeOut,   // Strikeout
    Square,      // Rectangle
    Circle,      // Oval / Circle
    Line,        // Line
    Ink,         // Freehand drawing
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
            "Stamp" => Some(AnnotationType::Stamp(stamp_name.unwrap_or_else(|| "Draft".into()))),
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
                vec![Object::Real(r as f64), Object::Real(g as f64), Object::Real(b as f64)],
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

        if let AnnotationType::Link { ref action, border_width, highlight } = self.annot_type {
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
                } else {
                    LinkAction::Uri("https://".into())
                }
            } else if let Some(dest) = dict.get("Dest") {
                match dest {
                    Object::Array(d_arr) => {
                        let page_idx = d_arr.first().and_then(Object::as_i64).unwrap_or(0).max(0) as usize;
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
    let Ok(re) = Regex::new(r#"https?://[^\s<>"'{}|\\^`]+[^\s`!()\[\]{};:'".,<>?«»“”‘’]"#) else {
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
