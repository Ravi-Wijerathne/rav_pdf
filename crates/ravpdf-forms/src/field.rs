//! Interactive Form Fields conforming to ISO 32000-1 Section 12.7 (AcroForms).
//!
//! Traceability: FR-11.1 - FR-11.6.

use ravpdf_core::document::PdfDocument;
use ravpdf_core::error::{PdfError, Result};
use ravpdf_core::object::{ObjectId, Object, PdfDictionary, PdfName, PdfString};

/// Types of AcroForm interactive fields.
#[derive(Debug, Clone, PartialEq)]
pub enum FormFieldType {
    Text,
    Checkbox,
    Choice,
    Signature,
}

impl FormFieldType {
    pub fn ft_name(&self) -> &'static str {
        match self {
            FormFieldType::Text => "Tx",
            FormFieldType::Checkbox => "Btn",
            FormFieldType::Choice => "Ch",
            FormFieldType::Signature => "Sig",
        }
    }
}

/// An interactive form field.
#[derive(Debug, Clone)]
pub struct FormField {
    pub id: Option<ObjectId>,
    pub field_type: FormFieldType,
    pub name: String,
    pub value: Option<String>,
    pub default_value: Option<String>,
    pub rect: [f64; 4],
    pub read_only: bool,
    pub required: bool,
}

impl FormField {
    pub fn new_text(name: impl Into<String>, rect: [f64; 4]) -> Self {
        Self {
            id: None,
            field_type: FormFieldType::Text,
            name: name.into(),
            value: None,
            default_value: None,
            rect,
            read_only: false,
            required: false,
        }
    }

    pub fn new_checkbox(name: impl Into<String>, rect: [f64; 4], checked: bool) -> Self {
        Self {
            id: None,
            field_type: FormFieldType::Checkbox,
            name: name.into(),
            value: Some(if checked { "Yes".into() } else { "Off".into() }),
            default_value: Some("Off".into()),
            rect,
            read_only: false,
            required: false,
        }
    }

    pub fn to_widget_dictionary(&self, page_id: ObjectId) -> PdfDictionary {
        let mut dict = PdfDictionary::new();
        dict.insert("Type", PdfName::new("Annot"));
        dict.insert("Subtype", PdfName::new("Widget"));
        dict.insert("FT", PdfName::new(self.field_type.ft_name()));
        dict.insert("T", PdfString::from_str(&self.name));
        dict.insert("P", page_id);
        dict.insert(
            "Rect",
            vec![
                Object::Real(self.rect[0]),
                Object::Real(self.rect[1]),
                Object::Real(self.rect[2]),
                Object::Real(self.rect[3]),
            ],
        );

        if let Some(ref val) = self.value {
            if self.field_type == FormFieldType::Checkbox {
                dict.insert("V", PdfName::new(val.as_str()));
                dict.insert("AS", PdfName::new(val.as_str()));
            } else {
                dict.insert("V", PdfString::from_str(val));
            }
        }

        // Standard default appearance string (/DA)
        dict.insert("DA", PdfString::from_str("/Helv 12 Tf 0 g"));

        let mut flags: i64 = 0;
        if self.read_only {
            flags |= 1;
        }
        if self.required {
            flags |= 2;
        }
        dict.insert("Ff", flags);

        dict
    }

    pub fn from_dictionary(id: ObjectId, dict: &PdfDictionary) -> Option<Self> {
        let ft = dict.get_name("FT")?;
        let field_type = match ft {
            "Tx" => FormFieldType::Text,
            "Btn" => FormFieldType::Checkbox,
            "Ch" => FormFieldType::Choice,
            "Sig" => FormFieldType::Signature,
            _ => return None,
        };

        let name = dict.get_str("T").unwrap_or_default();
        let value = dict.get_str("V");
        let default_value = dict.get_str("DV");

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

        let flags = dict.get_i64("Ff").unwrap_or(0);
        let read_only = (flags & 1) != 0;
        let required = (flags & 2) != 0;

        Some(Self {
            id: Some(id),
            field_type,
            name,
            value,
            default_value,
            rect,
            read_only,
            required,
        })
    }
}

/// Adds a form field to a specific page and registers it in the document AcroForm dictionary.
pub fn add_form_field(
    doc: &mut PdfDocument,
    page_index: usize,
    field: &FormField,
) -> Result<ObjectId> {
    let page_ids = doc.page_ids();
    let page_id = *page_ids
        .get(page_index)
        .ok_or_else(|| PdfError::InvalidStructure(format!("Page {} out of range", page_index)))?;

    let field_id = doc.next_object_id();
    let widget_dict = field.to_widget_dictionary(page_id);
    doc.objects.insert(field_id, Object::Dictionary(widget_dict));

    // 1. Add to page /Annots
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
        arr.push(Object::Reference(field_id));
    }

    // 2. Add to Catalog /AcroForm /Fields
    let root_id = doc.root_id;
    let root_obj = doc
        .objects
        .get_mut(&root_id)
        .ok_or(PdfError::ObjectNotFound(root_id.number, root_id.generation))?;
    let root_dict = root_obj
        .as_dict_mut()
        .ok_or_else(|| PdfError::InvalidStructure("Expected root catalog".into()))?;

    let acroform = root_dict
        .map
        .entry(PdfName::new("AcroForm"))
        .or_insert_with(|| Object::Dictionary(PdfDictionary::new()));

    if let Object::Dictionary(af_dict) = acroform {
        let fields = af_dict
            .map
            .entry(PdfName::new("Fields"))
            .or_insert_with(|| Object::Array(Vec::new()));
        if let Object::Array(arr) = fields {
            arr.push(Object::Reference(field_id));
        }
    }

    Ok(field_id)
}

/// Retrieves all form fields registered in the document.
pub fn get_form_fields(doc: &PdfDocument) -> Result<Vec<FormField>> {
    let root_dict = doc
        .catalog()
        .ok_or_else(|| PdfError::InvalidStructure("Missing catalog".into()))?;

    let mut fields = Vec::new();

    if let Some(af_dict) = root_dict.get_dict("AcroForm") {
        if let Some(arr) = af_dict.get_array("Fields") {
            for item in arr {
                if let Some(field_id) = item.as_reference() {
                    if let Some(f_dict) = doc.objects.get(&field_id).and_then(Object::as_dict) {
                        if let Some(field) = FormField::from_dictionary(field_id, f_dict) {
                            fields.push(field);
                        }
                    }
                }
            }
        }
    }

    Ok(fields)
}

/// Retrieves form fields associated with a specific page.
pub fn get_page_form_fields(doc: &PdfDocument, page_index: usize) -> Result<Vec<FormField>> {
    let page_ids = doc.page_ids();
    let page_id = match page_ids.get(page_index) {
        Some(id) => *id,
        None => return Ok(Vec::new()),
    };

    let page_obj = match doc.objects.get(&page_id).and_then(Object::as_dict) {
        Some(d) => d,
        None => return Ok(Vec::new()),
    };

    let mut fields = Vec::new();
    if let Some(annots) = page_obj.get_array("Annots") {
        for item in annots {
            if let Some(ref_id) = item.as_reference() {
                if let Some(dict) = doc.objects.get(&ref_id).and_then(Object::as_dict) {
                    if dict.get_name("Subtype") == Some("Widget") {
                        if let Some(field) = FormField::from_dictionary(ref_id, dict) {
                            fields.push(field);
                        }
                    }
                }
            }
        }
    }

    Ok(fields)
}

/// Updates the value of an existing interactive form field conforming to ISO 32000-1 Section 12.7.4.3.
pub fn set_form_field_value(doc: &mut PdfDocument, field_id: ObjectId, value: &str) -> Result<()> {
    let field_obj = doc
        .objects
        .get_mut(&field_id)
        .ok_or(PdfError::ObjectNotFound(field_id.number, field_id.generation))?;
    let field_dict = field_obj
        .as_dict_mut()
        .ok_or_else(|| PdfError::InvalidStructure("Expected field dictionary".into()))?;

    let ft = field_dict.get_name("FT").unwrap_or("Tx");
    if ft == "Btn" {
        field_dict.insert("V", PdfName::new(value));
        field_dict.insert("AS", PdfName::new(value));
    } else {
        field_dict.insert("V", PdfString::from_str(value));
    }

    Ok(())
}

