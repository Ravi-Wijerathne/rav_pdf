//! Document Metadata and Properties conforming to ISO 32000-1 Section 14.3.3.
//!
//! Traceability: FR-33.1 - FR-33.3.

use chrono::Utc;
use ravpdf_core::document::PdfDocument;
use ravpdf_core::error::{PdfError, Result};
use ravpdf_core::object::{Object, PdfDictionary, PdfString};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DocumentInfo {
    pub title: Option<String>,
    pub author: Option<String>,
    pub subject: Option<String>,
    pub keywords: Option<String>,
    pub creator: Option<String>,
    pub producer: Option<String>,
    pub creation_date: Option<String>,
    pub mod_date: Option<String>,
}

/// Reads the `/Info` metadata dictionary from the PDF document.
pub fn get_document_info(doc: &PdfDocument) -> Result<DocumentInfo> {
    let info_id = match doc.info_id {
        Some(id) => id,
        None => return Ok(DocumentInfo::default()),
    };

    let dict = doc
        .objects
        .get(&info_id)
        .and_then(Object::as_dict)
        .ok_or_else(|| PdfError::InvalidStructure("Missing /Info object".into()))?;

    Ok(DocumentInfo {
        title: dict.get_str("Title"),
        author: dict.get_str("Author"),
        subject: dict.get_str("Subject"),
        keywords: dict.get_str("Keywords"),
        creator: dict.get_str("Creator"),
        producer: dict.get_str("Producer"),
        creation_date: dict.get_str("CreationDate"),
        mod_date: dict.get_str("ModDate"),
    })
}

/// Sets or updates the `/Info` metadata dictionary.
pub fn set_document_info(doc: &mut PdfDocument, info: &DocumentInfo) -> Result<()> {
    let info_id = doc.info_id.unwrap_or_else(|| {
        let id = doc.next_object_id();
        doc.info_id = Some(id);
        doc.trailer.insert("Info", id);
        id
    });

    let mut dict = PdfDictionary::new();
    if let Some(ref t) = info.title {
        dict.insert("Title", PdfString::from_str(t));
    }
    if let Some(ref a) = info.author {
        dict.insert("Author", PdfString::from_str(a));
    }
    if let Some(ref s) = info.subject {
        dict.insert("Subject", PdfString::from_str(s));
    }
    if let Some(ref k) = info.keywords {
        dict.insert("Keywords", PdfString::from_str(k));
    }
    dict.insert("Creator", PdfString::from_str(info.creator.as_deref().unwrap_or("RavPDF")));
    dict.insert("Producer", PdfString::from_str(info.producer.as_deref().unwrap_or("RavPDF 1.0 (Rust)")));

    let now_str = Utc::now().format("D:%Y%m%d%H%M%SZ").to_string();
    dict.insert("ModDate", PdfString::from_str(&now_str));
    dict.insert(
        "CreationDate",
        PdfString::from_str(info.creation_date.as_deref().unwrap_or(&now_str)),
    );

    doc.objects.insert(info_id, Object::Dictionary(dict));
    Ok(())
}
