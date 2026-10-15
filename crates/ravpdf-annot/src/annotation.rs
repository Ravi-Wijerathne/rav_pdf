//! PDF Annotations conforming to ISO 32000-1 Section 12.5.
//!
//! Traceability: FR-08.1-3, FR-09.1-4, FR-10.1-4.

use ravpdf_core::document::PdfDocument;
use ravpdf_core::error::{PdfError, Result};
use ravpdf_core::object::{ObjectId, Object, PdfDictionary, PdfName, PdfString};

/// Subtypes of PDF annotations.
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

        dict
    }

    /// Deserializes an annotation from a PDF dictionary.
    pub fn from_dictionary(id: ObjectId, dict: &PdfDictionary) -> Option<Self> {
        let subtype = dict.get_name("Subtype")?;
        let stamp_name = dict.get_name("Name").map(|s| s.to_string());
        let annot_type = AnnotationType::from_subtype_name(subtype, stamp_name)?;

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
