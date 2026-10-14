//! Form Flattening conforming to ISO 32000-1.
//!
//! Traceability: FR-11.6, FR-37.2 (ravpdf flatten).

use crate::field::{get_form_fields, FormFieldType};
use ravpdf_core::document::PdfDocument;
use ravpdf_core::error::Result;
use ravpdf_core::object::{Object, PdfDictionary, PdfStream};

/// Flattens all interactive form fields into permanent page content and removes AcroForms.
pub fn flatten_form(doc: &mut PdfDocument) -> Result<()> {
    let fields = get_form_fields(doc)?;
    let page_ids = doc.page_ids();

    for field in &fields {
        if let Some(ref val) = field.value {
            // Find which page contains this widget
            let mut target_page_idx = None;
            for (idx, &p_id) in page_ids.iter().enumerate() {
                if let Some(p_dict) = doc.objects.get(&p_id).and_then(Object::as_dict) {
                    if let Some(annots) = p_dict.get_array("Annots") {
                        if annots.iter().any(|a| a.as_reference() == field.id) {
                            target_page_idx = Some(idx);
                            break;
                        }
                    }
                }
            }

            if let Some(idx) = target_page_idx {
                let x = field.rect[0] + 2.0;
                let y = field.rect[1] + 2.0;
                let text_val = match field.field_type {
                    FormFieldType::Checkbox => {
                        if val == "Yes" {
                            "[X]"
                        } else {
                            "[ ]"
                        }
                    }
                    _ => val.as_str(),
                };

                let stream_cmd = format!(
                    "\nq\nBT\n/F1 10 Tf\n0 0 0 rg\n{:.2} {:.2} Td\n({}) Tj\nET\nQ\n",
                    x, y, text_val
                );

                let stream_id = doc.next_object_id();
                let mut stream =
                    PdfStream::with_plain_data(PdfDictionary::new(), stream_cmd.into_bytes());
                let _ = stream.compress_flate();
                doc.objects.insert(stream_id, Object::Stream(stream));

                let page_id = page_ids[idx];
                if let Some(p_dict) = doc
                    .objects
                    .get_mut(&page_id)
                    .and_then(Object::as_dict_mut)
                {
                    match p_dict.get_mut("Contents") {
                        Some(Object::Array(arr)) => {
                            arr.push(Object::Reference(stream_id));
                        }
                        Some(Object::Reference(orig)) => {
                            let orig_id = *orig;
                            p_dict.insert(
                                "Contents",
                                vec![Object::Reference(orig_id), Object::Reference(stream_id)],
                            );
                        }
                        _ => {
                            p_dict.insert("Contents", stream_id);
                        }
                    }

                    // Remove widget from page /Annots
                    if let Some(field_id) = field.id {
                        if let Some(annots) = p_dict.get_array_mut("Annots") {
                            annots.retain(|a| a.as_reference() != Some(field_id));
                        }
                    }
                }
            }
        }

        // Remove field object itself
        if let Some(field_id) = field.id {
            doc.objects.remove(&field_id);
        }
    }

    // Remove /AcroForm from catalog
    let root_id = doc.root_id;
    if let Some(catalog) = doc
        .objects
        .get_mut(&root_id)
        .and_then(Object::as_dict_mut)
    {
        catalog.remove("AcroForm");
    }

    Ok(())
}
