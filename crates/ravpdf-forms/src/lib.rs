//! RavPDF Interactive Forms (AcroForms), Form Data, and Flattening.
//!
//! Traceability:
//! - FR-11.1 (Form Field Types: Text, Checkbox, Radio, Choice, ListBox, PushButton, Barcode, Signature)
//! - FR-11.2 (Form Field Properties: Appearance, Validation, Calculation, Format, Tab Order)
//! - FR-11.3 (Form Management: Auto-detection, Import/Export, Duplication, Flattening)

pub mod autodetect;
pub mod data;
pub mod field;
pub mod flatten;
pub mod taborder;

pub use autodetect::{apply_auto_detected_fields, auto_detect_form_fields, AutoDetectedField};
pub use data::{export_form_data, import_form_data, FormDataFormat};
pub use field::{
    add_form_field, duplicate_form_field, get_form_fields, get_page_form_fields,
    set_form_field_value, BarcodeSymbology, CheckboxStyle, FormField, FormFieldType,
};
pub use flatten::flatten_form;
pub use taborder::{apply_page_tab_order, sort_fields_by_tab_order, TabOrder};

#[cfg(test)]
mod tests {
    use super::*;
    use ravpdf_core::document::PdfDocument;

    #[test]
    fn test_add_and_flatten_forms() {
        let mut doc = PdfDocument::new();
        doc.add_blank_page(612.0, 792.0);

        let mut field = FormField::new_text("CustomerName", [100.0, 500.0, 250.0, 520.0]);
        field.value = Some("Alice Smith".into());

        let field_id = add_form_field(&mut doc, 0, &field).expect("add form field");
        assert!(doc.objects.contains_key(&field_id));

        let retrieved = get_form_fields(&doc).expect("get form fields");
        assert_eq!(retrieved.len(), 1);
        assert_eq!(retrieved[0].name, "CustomerName");
        assert_eq!(retrieved[0].value.as_deref(), Some("Alice Smith"));

        flatten_form(&mut doc).expect("flatten form");
        let fields_after = get_form_fields(&doc).expect("get form fields after flatten");
        assert_eq!(fields_after.len(), 0);
    }

    #[test]
    fn test_page_fields_and_value_update() {
        let mut doc = PdfDocument::new();
        doc.add_blank_page(612.0, 792.0);

        let field = FormField::new_text("Email", [50.0, 400.0, 200.0, 420.0]);
        let field_id = add_form_field(&mut doc, 0, &field).expect("add field");

        let page_fields = get_page_form_fields(&doc, 0).expect("get page fields");
        assert_eq!(page_fields.len(), 1);
        assert_eq!(page_fields[0].name, "Email");

        set_form_field_value(&mut doc, field_id, "test@example.com").expect("set value");
        let updated_page_fields = get_page_form_fields(&doc, 0).expect("get updated page fields");
        assert_eq!(updated_page_fields[0].value.as_deref(), Some("test@example.com"));
    }

    #[test]
    fn test_choice_and_pushbutton_fields() {
        let mut doc = PdfDocument::new();
        doc.add_blank_page(612.0, 792.0);

        // Choice field
        let opts = vec!["Standard".into(), "Express".into(), "Overnight".into()];
        let choice = FormField::new_choice(
            "ShippingMethod",
            [100.0, 600.0, 250.0, 620.0],
            opts.clone(),
            Some("Express".into()),
        );
        let choice_id = add_form_field(&mut doc, 0, &choice).expect("add choice");
        assert!(doc.objects.contains_key(&choice_id));

        // Push button field
        let btn = FormField::new_pushbutton(
            "SubmitOrder",
            [100.0, 550.0, 180.0, 580.0],
            "Submit Order",
        );
        let btn_id = add_form_field(&mut doc, 0, &btn).expect("add button");
        assert!(doc.objects.contains_key(&btn_id));

        let fields = get_form_fields(&doc).expect("get form fields");
        assert_eq!(fields.len(), 2);

        let retrieved_choice = fields
            .iter()
            .find(|f| f.name == "ShippingMethod")
            .expect("choice found");
        assert_eq!(retrieved_choice.field_type, FormFieldType::Choice);
        assert_eq!(retrieved_choice.value.as_deref(), Some("Express"));
        assert_eq!(retrieved_choice.options, opts);

        let retrieved_btn = fields
            .iter()
            .find(|f| f.name == "SubmitOrder")
            .expect("btn found");
        assert_eq!(retrieved_btn.field_type, FormFieldType::PushButton);
        assert_eq!(retrieved_btn.value.as_deref(), Some("Submit Order"));
    }

    #[test]
    fn test_radio_listbox_barcode_and_duplicate() {
        let mut doc = PdfDocument::new();
        doc.add_blank_page(612.0, 792.0);
        doc.add_blank_page(612.0, 792.0);

        // Radio button
        let radio = FormField::new_radio_button("Gender", "Female", [100.0, 500.0, 116.0, 516.0], true);
        add_form_field(&mut doc, 0, &radio).expect("add radio");

        // List box
        let listbox = FormField::new_listbox(
            "Skills",
            [100.0, 400.0, 200.0, 480.0],
            vec!["Rust".into(), "Python".into(), "C++".into()],
            Some("Rust".into()),
            true,
        );
        add_form_field(&mut doc, 0, &listbox).expect("add listbox");

        // Barcode
        let barcode = FormField::new_barcode(
            "TrackingCode",
            [100.0, 300.0, 250.0, 350.0],
            BarcodeSymbology::QrCode,
            Some("https://ravpdf.org/track/12345".into()),
        );
        add_form_field(&mut doc, 0, &barcode).expect("add barcode");

        // Duplicate barcode to page 1 (FR-11.3.5)
        let dup_ids = duplicate_form_field(&mut doc, &barcode, &[1]).expect("duplicate");
        assert_eq!(dup_ids.len(), 1);

        let page1_fields = get_page_form_fields(&doc, 1).expect("page 1 fields");
        assert_eq!(page1_fields.len(), 1);
        assert_eq!(page1_fields[0].name, "TrackingCode");
    }
}
