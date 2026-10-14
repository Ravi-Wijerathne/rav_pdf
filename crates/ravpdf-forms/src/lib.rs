//! RavPDF Interactive Forms (AcroForms) and Flattening.
//!
//! Traceability:
//! - FR-11 (Forms and Interactive Fields: text fields, checkboxes, choices)
//! - FR-11.6 & FR-37.2 (Form Flattening)

pub mod field;
pub mod flatten;

pub use field::{add_form_field, get_form_fields, FormField, FormFieldType};
pub use flatten::flatten_form;

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
}
