//! Embedded File Attachments conforming to ISO 32000-1 Section 7.7.4 & Section 14.13.
//!
//! Traceability:
//! - FR-29.1.1 (Support attaching arbitrary files: documents, images, data files to a PDF)
//! - FR-29.1.2 (Display attached files with filename, description, size)
//! - FR-29.1.3 (Allow opening, saving/extracting, and deleting attachments)

use anyhow::{anyhow, Result};
use ravpdf_core::document::PdfDocument;
use ravpdf_core::object::{Object, ObjectId, PdfDictionary, PdfName, PdfStream, PdfString};
use serde::{Deserialize, Serialize};

/// Represents an attached file extracted from or to be embedded in a PDF (FR-29.1.2).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FileAttachment {
    pub filespec_id: ObjectId,
    pub stream_id: ObjectId,
    pub filename: String,
    pub description: Option<String>,
    pub size: usize,
    pub data: Vec<u8>,
}

/// Retrieves all embedded file attachments present in the document (FR-29.1.2).
pub fn get_attachments(doc: &PdfDocument) -> Result<Vec<FileAttachment>> {
    let mut attachments = Vec::new();

    // 1. Resolve Document Catalog (Root)
    let catalog_obj = doc
        .get_object(doc.root_id)
        .ok_or_else(|| anyhow!("Missing document catalog"))?;
    let catalog = catalog_obj
        .as_dict()
        .ok_or_else(|| anyhow!("Catalog is not a dictionary"))?;

    // 2. Resolve /Names dictionary in Catalog
    let names_dict_obj = match catalog.get("Names") {
        Some(obj) => match doc.resolve(obj) {
            Some(resolved) => resolved,
            None => return Ok(attachments),
        },
        None => return Ok(attachments),
    };
    let names_dict = match names_dict_obj.as_dict() {
        Some(d) => d,
        None => return Ok(attachments),
    };

    // 3. Resolve /EmbeddedFiles dictionary
    let ef_dict_obj = match names_dict.get("EmbeddedFiles") {
        Some(obj) => match doc.resolve(obj) {
            Some(resolved) => resolved,
            None => return Ok(attachments),
        },
        None => return Ok(attachments),
    };
    let ef_dict = match ef_dict_obj.as_dict() {
        Some(d) => d,
        None => return Ok(attachments),
    };

    // 4. Resolve /Names array (name tree leaf node: [ key1, filespec_ref1, key2, filespec_ref2, ... ])
    let names_arr = match ef_dict.get_array("Names") {
        Some(arr) => arr,
        None => return Ok(attachments),
    };

    let mut i = 0;
    while i + 1 < names_arr.len() {
        let _key_name = match &names_arr[i] {
            Object::String(s) => Some(s.to_string_lossy()),
            _ => None,
        };
        let val_obj = &names_arr[i + 1];

        if let Some(filespec_id) = val_obj.as_reference() {
            if let Some(filespec_obj) = doc.get_object(filespec_id) {
                if let Some(filespec_dict) = filespec_obj.as_dict() {
                    let filename = filespec_dict
                        .get_str("UF")
                        .or_else(|| filespec_dict.get_str("F"))
                        .unwrap_or_else(|| format!("attachment_{}", filespec_id.number));

                    let description = filespec_dict.get_str("Desc");

                    // Find /EF << /F stream_id >>
                    if let Some(ef_map_obj) = filespec_dict.get("EF") {
                        if let Some(ef_map) = doc.resolve(ef_map_obj).and_then(Object::as_dict) {
                            if let Some(stream_id) = ef_map.get_ref("F") {
                                if let Some(Object::Stream(stream)) = doc.get_object(stream_id) {
                                    let data = stream.decode().unwrap_or_else(|_| stream.raw_data.clone());
                                    let size = data.len();

                                    attachments.push(FileAttachment {
                                        filespec_id,
                                        stream_id,
                                        filename,
                                        description,
                                        size,
                                        data,
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }
        i += 2;
    }

    Ok(attachments)
}

/// Embeds a new file into the PDF document (FR-29.1.1).
pub fn add_attachment(
    doc: &mut PdfDocument,
    filename: &str,
    description: Option<&str>,
    data: &[u8],
) -> Result<()> {
    // 1. Create EmbeddedFile stream object
    let stream_id = doc.next_object_id();
    let mut ef_stream_dict = PdfDictionary::new();
    ef_stream_dict.insert("Type", PdfName::new("EmbeddedFile"));

    let mut params_dict = PdfDictionary::new();
    params_dict.insert("Size", data.len() as i64);
    ef_stream_dict.insert("Params", Object::Dictionary(params_dict));

    let mut stream = PdfStream::with_plain_data(ef_stream_dict, data.to_vec());
    let _ = stream.compress_flate();
    doc.objects.insert(stream_id, Object::Stream(stream));

    // 2. Create /Filespec dictionary object
    let filespec_id = doc.next_object_id();
    let mut filespec = PdfDictionary::new();
    filespec.insert("Type", PdfName::new("Filespec"));
    filespec.insert("F", PdfString::from_str(filename));
    filespec.insert("UF", PdfString::from_str(filename));

    if let Some(desc) = description {
        filespec.insert("Desc", PdfString::from_str(desc));
    }

    let mut ef_entry = PdfDictionary::new();
    ef_entry.insert("F", stream_id);
    filespec.insert("EF", Object::Dictionary(ef_entry));

    doc.objects.insert(filespec_id, Object::Dictionary(filespec));

    // 3. Ensure Catalog has /Names -> /EmbeddedFiles -> /Names array
    let catalog = doc
        .objects
        .get_mut(&doc.root_id)
        .and_then(Object::as_dict_mut)
        .ok_or_else(|| anyhow!("Invalid catalog"))?;

    if !catalog.contains_key("Names") {
        catalog.insert("Names", Object::Dictionary(PdfDictionary::new()));
    }
    let names_dict = catalog.get_dict_mut("Names").unwrap();

    if !names_dict.contains_key("EmbeddedFiles") {
        let mut ef_tree = PdfDictionary::new();
        ef_tree.insert("Names", Object::Array(Vec::new()));
        names_dict.insert("EmbeddedFiles", Object::Dictionary(ef_tree));
    }
    let ef_tree = names_dict.get_dict_mut("EmbeddedFiles").unwrap();

    if !ef_tree.contains_key("Names") {
        ef_tree.insert("Names", Object::Array(Vec::new()));
    }
    let names_arr = ef_tree.get_array_mut("Names").unwrap();

    // Append key and reference to /Names array
    names_arr.push(Object::String(PdfString::from_str(filename)));
    names_arr.push(Object::Reference(filespec_id));

    Ok(())
}

/// Removes an attachment matching `filename` from the document (FR-29.1.3).
pub fn remove_attachment(doc: &mut PdfDocument, filename: &str) -> Result<bool> {
    let attachments = get_attachments(doc)?;
    let target = attachments.iter().find(|a| a.filename == filename);

    let (filespec_id, stream_id) = match target {
        Some(a) => (a.filespec_id, a.stream_id),
        None => return Ok(false),
    };

    // Remove stream and filespec objects
    doc.objects.remove(&filespec_id);
    doc.objects.remove(&stream_id);

    // Remove from name tree
    if let Some(catalog) = doc.objects.get_mut(&doc.root_id).and_then(Object::as_dict_mut) {
        if let Some(names_dict) = catalog.get_dict_mut("Names") {
            if let Some(ef_tree) = names_dict.get_dict_mut("EmbeddedFiles") {
                if let Some(names_arr) = ef_tree.get_array_mut("Names") {
                    let mut i = 0;
                    while i + 1 < names_arr.len() {
                        let matches = match &names_arr[i] {
                            Object::String(s) => s.to_string_lossy() == filename,
                            _ => false,
                        };
                        if matches {
                            names_arr.remove(i); // remove key
                            names_arr.remove(i); // remove reference
                            return Ok(true);
                        }
                        i += 2;
                    }
                }
            }
        }
    }

    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_attachments_lifecycle() {
        let mut doc = PdfDocument::new();
        doc.add_blank_page(612.0, 792.0);

        // Initially no attachments
        let initial = get_attachments(&doc).expect("get attachments");
        assert!(initial.is_empty());

        // Add 1st attachment
        let report_csv = b"Year,Revenue,Expenses\n2025,1200000,850000\n2026,1650000,980000";
        add_attachment(&mut doc, "annual_report.csv", Some("Financial data"), report_csv)
            .expect("add attachment");

        let after_first = get_attachments(&doc).expect("get after first");
        assert_eq!(after_first.len(), 1);
        assert_eq!(after_first[0].filename, "annual_report.csv");
        assert_eq!(after_first[0].description.as_deref(), Some("Financial data"));
        assert_eq!(after_first[0].data, report_csv);

        // Add 2nd attachment
        let readme = b"Confidential document. Do not distribute without approval.";
        add_attachment(&mut doc, "README.txt", None, readme).expect("add second");

        let after_second = get_attachments(&doc).expect("get after second");
        assert_eq!(after_second.len(), 2);

        // Serialization roundtrip
        let bytes = doc.to_bytes().expect("serialize");
        let mut reloaded = PdfDocument::load_from_bytes(&bytes).expect("reload");
        let reloaded_atts = get_attachments(&reloaded).expect("get from reloaded");
        assert_eq!(reloaded_atts.len(), 2);
        assert_eq!(reloaded_atts[0].data, report_csv);
        assert_eq!(reloaded_atts[1].data, readme);

        // Remove 1st attachment
        let removed = remove_attachment(&mut reloaded, "annual_report.csv").expect("remove attachment");
        assert!(removed);

        let final_atts = get_attachments(&reloaded).expect("get final");
        assert_eq!(final_atts.len(), 1);
        assert_eq!(final_atts[0].filename, "README.txt");
    }
}
