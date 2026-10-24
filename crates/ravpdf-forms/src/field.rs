//! Interactive Form Fields conforming to ISO 32000-1 Section 12.7 (AcroForms).
//!
//! Traceability:
//! - FR-11.1 (Form Field Types: Text, Checkbox, Radio, Choice, ListBox, PushButton, Barcode, Signature)
//! - FR-11.2 (Form Field Properties: Name, Tooltip, Required, Read-Only, No-Export, Appearance, Validation, Calculation, Format)
//! - FR-11.3 (Form Management: Duplication, Modification)

use ravpdf_core::document::PdfDocument;
use ravpdf_core::error::{PdfError, Result};
use ravpdf_core::object::{Object, ObjectId, PdfDictionary, PdfName, PdfString};
use serde::{Deserialize, Serialize};

/// Types of AcroForm interactive fields (FR-11.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FormFieldType {
    Text,
    Checkbox,
    Radio,
    Choice,
    ListBox,
    PushButton,
    Barcode,
    Signature,
}

impl FormFieldType {
    pub fn ft_name(&self) -> &'static str {
        match self {
            FormFieldType::Text | FormFieldType::Barcode => "Tx",
            FormFieldType::Checkbox | FormFieldType::Radio | FormFieldType::PushButton => "Btn",
            FormFieldType::Choice | FormFieldType::ListBox => "Ch",
            FormFieldType::Signature => "Sig",
        }
    }
}

/// Checkbox check styles (FR-11.1.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum CheckboxStyle {
    #[default]
    Check,
    Circle,
    Cross,
    Diamond,
    Square,
    Star,
}

impl CheckboxStyle {
    pub fn zapf_char(&self) -> &'static str {
        match self {
            CheckboxStyle::Check => "4",
            CheckboxStyle::Circle => "l",
            CheckboxStyle::Cross => "8",
            CheckboxStyle::Diamond => "u",
            CheckboxStyle::Square => "n",
            CheckboxStyle::Star => "H",
        }
    }
}

/// 2D / 1D Barcode symbologies for interactive barcode fields (FR-11.1.8).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum BarcodeSymbology {
    #[default]
    QrCode,
    Code128,
    Pdf417,
}

/// An interactive AcroForm field representation (FR-11.1, FR-11.2).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormField {
    pub id: Option<ObjectId>,
    pub field_type: FormFieldType,
    pub name: String,
    pub value: Option<String>,
    pub default_value: Option<String>,
    pub rect: [f64; 4],
    pub read_only: bool,
    pub required: bool,
    pub no_export: bool,
    pub options: Vec<String>,

    // Type-specific options
    pub multiline: bool,
    pub password: bool,
    pub rich_text: bool,
    pub check_style: CheckboxStyle,
    pub radio_group: Option<String>,
    pub radio_export_value: Option<String>,
    pub is_multiselect: bool,
    pub barcode_symbology: Option<BarcodeSymbology>,

    // Appearance & behavior properties (FR-11.2)
    pub tooltip: Option<String>,
    pub max_len: Option<usize>,
    pub font_name: Option<String>,
    pub font_size: Option<f32>,
    pub text_color: Option<[f32; 3]>,
    pub border_color: Option<[f32; 3]>,
    pub background_color: Option<[f32; 3]>,
    pub validation_script: Option<String>,
    pub calculation_script: Option<String>,
    pub format_script: Option<String>,
}

impl FormField {
    /// Creates a single-line text field (FR-11.1.1).
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
            no_export: false,
            options: Vec::new(),
            multiline: false,
            password: false,
            rich_text: false,
            check_style: CheckboxStyle::default(),
            radio_group: None,
            radio_export_value: None,
            is_multiselect: false,
            barcode_symbology: None,
            tooltip: None,
            max_len: None,
            font_name: Some("Helv".into()),
            font_size: Some(12.0),
            text_color: Some([0.0, 0.0, 0.0]),
            border_color: None,
            background_color: None,
            validation_script: None,
            calculation_script: None,
            format_script: None,
        }
    }

    /// Creates a multi-line text field (FR-11.1.1).
    pub fn new_multiline_text(name: impl Into<String>, rect: [f64; 4]) -> Self {
        let mut field = Self::new_text(name, rect);
        field.multiline = true;
        field
    }

    /// Creates a password text field (FR-11.1.1).
    pub fn new_password(name: impl Into<String>, rect: [f64; 4]) -> Self {
        let mut field = Self::new_text(name, rect);
        field.password = true;
        field
    }

    /// Creates an interactive checkbox field (FR-11.1.2).
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
            no_export: false,
            options: Vec::new(),
            multiline: false,
            password: false,
            rich_text: false,
            check_style: CheckboxStyle::Check,
            radio_group: None,
            radio_export_value: None,
            is_multiselect: false,
            barcode_symbology: None,
            tooltip: None,
            max_len: None,
            font_name: Some("ZapfDingbats".into()),
            font_size: Some(12.0),
            text_color: Some([0.0, 0.0, 0.0]),
            border_color: None,
            background_color: None,
            validation_script: None,
            calculation_script: None,
            format_script: None,
        }
    }

    /// Creates a radio button belonging to a mutually exclusive group (FR-11.1.3).
    pub fn new_radio_button(
        group_name: impl Into<String>,
        export_value: impl Into<String>,
        rect: [f64; 4],
        selected: bool,
    ) -> Self {
        let grp = group_name.into();
        let exp = export_value.into();
        Self {
            id: None,
            field_type: FormFieldType::Radio,
            name: grp.clone(),
            value: if selected { Some(exp.clone()) } else { Some("Off".into()) },
            default_value: Some("Off".into()),
            rect,
            read_only: false,
            required: false,
            no_export: false,
            options: Vec::new(),
            multiline: false,
            password: false,
            rich_text: false,
            check_style: CheckboxStyle::Circle,
            radio_group: Some(grp),
            radio_export_value: Some(exp),
            is_multiselect: false,
            barcode_symbology: None,
            tooltip: None,
            max_len: None,
            font_name: Some("ZapfDingbats".into()),
            font_size: Some(12.0),
            text_color: Some([0.0, 0.0, 0.0]),
            border_color: None,
            background_color: None,
            validation_script: None,
            calculation_script: None,
            format_script: None,
        }
    }

    /// Creates a dropdown / choice combo box field (FR-11.1.4).
    pub fn new_choice(
        name: impl Into<String>,
        rect: [f64; 4],
        options: Vec<String>,
        selected: Option<String>,
    ) -> Self {
        let value = selected.or_else(|| options.first().cloned());
        Self {
            id: None,
            field_type: FormFieldType::Choice,
            name: name.into(),
            value: value.clone(),
            default_value: value,
            rect,
            read_only: false,
            required: false,
            no_export: false,
            options,
            multiline: false,
            password: false,
            rich_text: false,
            check_style: CheckboxStyle::default(),
            radio_group: None,
            radio_export_value: None,
            is_multiselect: false,
            barcode_symbology: None,
            tooltip: None,
            max_len: None,
            font_name: Some("Helv".into()),
            font_size: Some(12.0),
            text_color: Some([0.0, 0.0, 0.0]),
            border_color: None,
            background_color: None,
            validation_script: None,
            calculation_script: None,
            format_script: None,
        }
    }

    /// Creates a list box field (FR-11.1.5).
    pub fn new_listbox(
        name: impl Into<String>,
        rect: [f64; 4],
        options: Vec<String>,
        selected: Option<String>,
        multiselect: bool,
    ) -> Self {
        let value = selected.or_else(|| options.first().cloned());
        Self {
            id: None,
            field_type: FormFieldType::ListBox,
            name: name.into(),
            value: value.clone(),
            default_value: value,
            rect,
            read_only: false,
            required: false,
            no_export: false,
            options,
            multiline: false,
            password: false,
            rich_text: false,
            check_style: CheckboxStyle::default(),
            radio_group: None,
            radio_export_value: None,
            is_multiselect: multiselect,
            barcode_symbology: None,
            tooltip: None,
            max_len: None,
            font_name: Some("Helv".into()),
            font_size: Some(12.0),
            text_color: Some([0.0, 0.0, 0.0]),
            border_color: None,
            background_color: None,
            validation_script: None,
            calculation_script: None,
            format_script: None,
        }
    }

    /// Creates a push button field (FR-11.1.6).
    pub fn new_pushbutton(
        name: impl Into<String>,
        rect: [f64; 4],
        label: impl Into<String>,
    ) -> Self {
        let label_str = label.into();
        Self {
            id: None,
            field_type: FormFieldType::PushButton,
            name: name.into(),
            value: Some(label_str.clone()),
            default_value: Some(label_str),
            rect,
            read_only: false,
            required: false,
            no_export: false,
            options: Vec::new(),
            multiline: false,
            password: false,
            rich_text: false,
            check_style: CheckboxStyle::default(),
            radio_group: None,
            radio_export_value: None,
            is_multiselect: false,
            barcode_symbology: None,
            tooltip: None,
            max_len: None,
            font_name: Some("Helv".into()),
            font_size: Some(12.0),
            text_color: Some([0.0, 0.0, 0.0]),
            border_color: None,
            background_color: None,
            validation_script: None,
            calculation_script: None,
            format_script: None,
        }
    }

    /// Creates a barcode field (FR-11.1.8).
    pub fn new_barcode(
        name: impl Into<String>,
        rect: [f64; 4],
        symbology: BarcodeSymbology,
        initial_value: Option<String>,
    ) -> Self {
        Self {
            id: None,
            field_type: FormFieldType::Barcode,
            name: name.into(),
            value: initial_value.clone(),
            default_value: initial_value,
            rect,
            read_only: true,
            required: false,
            no_export: false,
            options: Vec::new(),
            multiline: false,
            password: false,
            rich_text: false,
            check_style: CheckboxStyle::default(),
            radio_group: None,
            radio_export_value: None,
            is_multiselect: false,
            barcode_symbology: Some(symbology),
            tooltip: None,
            max_len: None,
            font_name: Some("Courier".into()),
            font_size: Some(8.0),
            text_color: Some([0.0, 0.0, 0.0]),
            border_color: None,
            background_color: None,
            validation_script: None,
            calculation_script: None,
            format_script: None,
        }
    }

    /// Creates a digital signature field (FR-11.1.7).
    pub fn new_signature(name: impl Into<String>, rect: [f64; 4]) -> Self {
        Self {
            id: None,
            field_type: FormFieldType::Signature,
            name: name.into(),
            value: None,
            default_value: None,
            rect,
            read_only: false,
            required: false,
            no_export: false,
            options: Vec::new(),
            multiline: false,
            password: false,
            rich_text: false,
            check_style: CheckboxStyle::default(),
            radio_group: None,
            radio_export_value: None,
            is_multiselect: false,
            barcode_symbology: None,
            tooltip: None,
            max_len: None,
            font_name: None,
            font_size: None,
            text_color: None,
            border_color: None,
            background_color: None,
            validation_script: None,
            calculation_script: None,
            format_script: None,
        }
    }

    /// Serializes this form field into an ISO 32000-1 Widget annotation dictionary.
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
            match self.field_type {
                FormFieldType::Checkbox => {
                    dict.insert("V", PdfName::new(val.as_str()));
                    dict.insert("AS", PdfName::new(val.as_str()));
                }
                FormFieldType::Radio => {
                    dict.insert("V", PdfName::new(val.as_str()));
                    dict.insert("AS", PdfName::new(val.as_str()));
                }
                _ => {
                    dict.insert("V", PdfString::from_str(val));
                }
            }
        }

        if let Some(ref dval) = self.default_value {
            if self.field_type == FormFieldType::Checkbox || self.field_type == FormFieldType::Radio {
                dict.insert("DV", PdfName::new(dval.as_str()));
            } else {
                dict.insert("DV", PdfString::from_str(dval));
            }
        }

        // Appearance string (/DA)
        let font = self.font_name.as_deref().unwrap_or("Helv");
        let size = self.font_size.unwrap_or(12.0);
        let da_str = if let Some(tc) = self.text_color {
            if (tc[0] - tc[1]).abs() < 1e-4 && (tc[1] - tc[2]).abs() < 1e-4 {
                format!("/{} {:.1} Tf {:.2} g", font, size, tc[0])
            } else {
                format!("/{} {:.1} Tf {:.2} {:.2} {:.2} rg", font, size, tc[0], tc[1], tc[2])
            }
        } else {
            format!("/{} {:.1} Tf 0 g", font, size)
        };
        dict.insert("DA", PdfString::from_str(&da_str));

        // Field Flags (/Ff)
        let mut flags: i64 = 0;
        if self.read_only {
            flags |= 1 << 0; // Bit 1: ReadOnly
        }
        if self.required {
            flags |= 1 << 1; // Bit 2: Required
        }
        if self.no_export {
            flags |= 1 << 2; // Bit 3: NoExport
        }

        match self.field_type {
            FormFieldType::Text | FormFieldType::Barcode => {
                if self.multiline {
                    flags |= 1 << 12; // Bit 13: Multiline
                }
                if self.password {
                    flags |= 1 << 13; // Bit 14: Password
                }
                if self.rich_text {
                    flags |= 1 << 25; // Bit 26: RichText
                }
            }
            FormFieldType::PushButton => {
                flags |= 1 << 16; // Bit 17: Pushbutton
            }
            FormFieldType::Radio => {
                flags |= 1 << 15; // Bit 16: Radio
                flags |= 1 << 14; // Bit 15: NoToggleToOff
            }
            FormFieldType::Choice => {
                flags |= 1 << 17; // Bit 18: Combo
                if self.is_multiselect {
                    flags |= 1 << 21; // Bit 22: MultiSelect
                }
            }
            FormFieldType::ListBox if self.is_multiselect => {
                flags |= 1 << 21; // Bit 22: MultiSelect
            }
            _ => {}
        }
        dict.insert("Ff", flags);

        // Options for Choice / ListBox
        if !self.options.is_empty() {
            let opt_arr: Vec<Object> = self
                .options
                .iter()
                .map(|s| Object::String(PdfString::from_str(s)))
                .collect();
            dict.insert("Opt", Object::Array(opt_arr));
        }

        // Appearance Characteristics (/MK)
        let mut mk = PdfDictionary::new();
        let mut has_mk = false;
        if let Some(bc) = self.border_color {
            mk.insert("BC", vec![Object::Real(bc[0] as f64), Object::Real(bc[1] as f64), Object::Real(bc[2] as f64)]);
            has_mk = true;
        }
        if let Some(bg) = self.background_color {
            mk.insert("BG", vec![Object::Real(bg[0] as f64), Object::Real(bg[1] as f64), Object::Real(bg[2] as f64)]);
            has_mk = true;
        }
        if self.field_type == FormFieldType::PushButton {
            if let Some(ref label) = self.value {
                mk.insert("CA", PdfString::from_str(label));
                has_mk = true;
            }
        }
        if has_mk {
            dict.insert("MK", Object::Dictionary(mk));
        }

        // Tooltip (/TU)
        if let Some(ref tu) = self.tooltip {
            dict.insert("TU", PdfString::from_str(tu));
        }

        // Max Length (/MaxLen)
        if let Some(max_len) = self.max_len {
            dict.insert("MaxLen", max_len as i64);
        }

        // Additional Actions (/AA: validation, calculation, format scripts)
        let mut aa = PdfDictionary::new();
        let mut has_aa = false;

        if let Some(ref val_js) = self.validation_script {
            let mut v_dict = PdfDictionary::new();
            v_dict.insert("S", PdfName::new("JavaScript"));
            v_dict.insert("JS", PdfString::from_str(val_js));
            aa.insert("V", Object::Dictionary(v_dict));
            has_aa = true;
        }

        if let Some(ref calc_js) = self.calculation_script {
            let mut c_dict = PdfDictionary::new();
            c_dict.insert("S", PdfName::new("JavaScript"));
            c_dict.insert("JS", PdfString::from_str(calc_js));
            aa.insert("C", Object::Dictionary(c_dict));
            has_aa = true;
        }

        if let Some(ref fmt_js) = self.format_script {
            let mut f_dict = PdfDictionary::new();
            f_dict.insert("S", PdfName::new("JavaScript"));
            f_dict.insert("JS", PdfString::from_str(fmt_js));
            aa.insert("F", Object::Dictionary(f_dict));
            has_aa = true;
        }

        if has_aa {
            dict.insert("AA", Object::Dictionary(aa));
        }

        dict
    }

    /// Deserializes an AcroForm widget dictionary conforming to ISO 32000-1.
    pub fn from_dictionary(id: ObjectId, dict: &PdfDictionary) -> Option<Self> {
        let ft = dict.get_name("FT")?;
        let flags = dict.get_i64("Ff").unwrap_or(0);
        let read_only = (flags & (1 << 0)) != 0;
        let required = (flags & (1 << 1)) != 0;
        let no_export = (flags & (1 << 2)) != 0;

        let multiline = (flags & (1 << 12)) != 0;
        let password = (flags & (1 << 13)) != 0;
        let rich_text = (flags & (1 << 25)) != 0;
        let is_multiselect = (flags & (1 << 21)) != 0;

        let field_type = match ft {
            "Tx" => FormFieldType::Text,
            "Btn" => {
                if (flags & (1 << 16)) != 0 {
                    FormFieldType::PushButton
                } else if (flags & (1 << 15)) != 0 {
                    FormFieldType::Radio
                } else {
                    FormFieldType::Checkbox
                }
            }
            "Ch" => {
                if (flags & (1 << 17)) != 0 {
                    FormFieldType::Choice
                } else {
                    FormFieldType::ListBox
                }
            }
            "Sig" => FormFieldType::Signature,
            _ => return None,
        };

        let name = dict.get_str("T").unwrap_or_default();
        let default_value = dict.get_str("DV");

        let value = if field_type == FormFieldType::PushButton && dict.get_str("V").is_none() {
            if let Some(Object::Dictionary(mk)) = dict.get("MK") {
                mk.get_str("CA")
            } else {
                dict.get_str("V")
            }
        } else {
            dict.get_str("V")
        };

        let tooltip = dict.get_str("TU");
        let max_len = dict.get_i64("MaxLen").map(|n| n as usize);

        let mut options = Vec::new();
        if let Some(opt_arr) = dict.get_array("Opt") {
            for item in opt_arr {
                if let Some(s) = item.as_str() {
                    options.push(s.to_string());
                } else if let Object::Array(sub_arr) = item {
                    if let Some(first) = sub_arr.first().and_then(Object::as_str) {
                        options.push(first.to_string());
                    }
                }
            }
        }

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

        // Parse scripts from /AA
        let mut validation_script = None;
        let mut calculation_script = None;
        let mut format_script = None;

        if let Some(Object::Dictionary(aa)) = dict.get("AA") {
            if let Some(Object::Dictionary(v)) = aa.get("V") {
                validation_script = v.get_str("JS");
            }
            if let Some(Object::Dictionary(c)) = aa.get("C") {
                calculation_script = c.get_str("JS");
            }
            if let Some(Object::Dictionary(f)) = aa.get("F") {
                format_script = f.get_str("JS");
            }
        }

        Some(Self {
            id: Some(id),
            field_type,
            name,
            value,
            default_value,
            rect,
            read_only,
            required,
            no_export,
            options,
            multiline,
            password,
            rich_text,
            check_style: CheckboxStyle::Check,
            radio_group: None,
            radio_export_value: None,
            is_multiselect,
            barcode_symbology: None,
            tooltip,
            max_len,
            font_name: Some("Helv".into()),
            font_size: Some(12.0),
            text_color: Some([0.0, 0.0, 0.0]),
            border_color: None,
            background_color: None,
            validation_script,
            calculation_script,
            format_script,
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

/// Duplicates an interactive form field across multiple specified pages (FR-11.3.5).
pub fn duplicate_form_field(
    doc: &mut PdfDocument,
    field: &FormField,
    target_pages: &[usize],
) -> Result<Vec<ObjectId>> {
    let mut created_ids = Vec::new();
    for &page_idx in target_pages {
        let id = add_form_field(doc, page_idx, field)?;
        created_ids.push(id);
    }
    Ok(created_ids)
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
