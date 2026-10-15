//! Page Management operations conforming to ISO 32000-1 Section 7.7.3.
//!
//! Traceability: FR-06.1 - FR-06.11, FR-07.

use ravpdf_core::document::PdfDocument;
use ravpdf_core::error::{PdfError, Result};
use ravpdf_core::object::{ObjectId, Object, PdfDictionary, PdfName};

/// Inserts a new blank page at `index` (0-based). If `index >= page_count()`, it appends.
pub fn insert_blank_page(
    doc: &mut PdfDocument,
    index: usize,
    width: f64,
    height: f64,
) -> Result<ObjectId> {
    let page_id = doc.next_object_id();
    let pages_id = doc
        .pages_root_id()
        .ok_or_else(|| PdfError::InvalidStructure("Missing /Pages root".into()))?;

    let mut page_dict = PdfDictionary::new();
    page_dict.insert("Type", PdfName::new("Page"));
    page_dict.insert("Parent", pages_id);
    page_dict.insert(
        "MediaBox",
        vec![
            Object::Real(0.0),
            Object::Real(0.0),
            Object::Real(width),
            Object::Real(height),
        ],
    );
    page_dict.insert("Resources", PdfDictionary::new());

    doc.objects.insert(page_id, Object::Dictionary(page_dict));

    // Update Kids array in root Pages
    insert_page_id_at(doc, index, page_id)?;
    Ok(page_id)
}

/// Rotates a page by a specified angle (90, 180, 270, or 0 degrees).
pub fn rotate_page(doc: &mut PdfDocument, index: usize, angle: i64) -> Result<()> {
    let page_ids = doc.page_ids();
    let page_id = *page_ids
        .get(index)
        .ok_or_else(|| PdfError::InvalidStructure(format!("Page index {} out of range", index)))?;

    let page_obj = doc
        .objects
        .get_mut(&page_id)
        .ok_or(PdfError::ObjectNotFound(page_id.number, page_id.generation))?;

    let page_dict = page_obj
        .as_dict_mut()
        .ok_or_else(|| PdfError::InvalidStructure("Expected page dictionary".into()))?;

    let current_rot = page_dict.get_i64("Rotate").unwrap_or(0);
    let new_rot = (current_rot + angle).rem_euclid(360);
    page_dict.insert("Rotate", new_rot);

    Ok(())
}

/// Sets the `/CropBox` for a page.
pub fn crop_page(doc: &mut PdfDocument, index: usize, crop_box: [f64; 4]) -> Result<()> {
    let page_ids = doc.page_ids();
    let page_id = *page_ids
        .get(index)
        .ok_or_else(|| PdfError::InvalidStructure(format!("Page index {} out of range", index)))?;

    let page_obj = doc
        .objects
        .get_mut(&page_id)
        .ok_or(PdfError::ObjectNotFound(page_id.number, page_id.generation))?;

    let page_dict = page_obj
        .as_dict_mut()
        .ok_or_else(|| PdfError::InvalidStructure("Expected page dictionary".into()))?;

    page_dict.insert(
        "CropBox",
        vec![
            Object::Real(crop_box[0]),
            Object::Real(crop_box[1]),
            Object::Real(crop_box[2]),
            Object::Real(crop_box[3]),
        ],
    );

    Ok(())
}

/// Deletes the page at `index`. Cannot delete the last remaining page of a document.
pub fn delete_page(doc: &mut PdfDocument, index: usize) -> Result<()> {
    let page_ids = doc.page_ids();
    if page_ids.len() <= 1 {
        return Err(PdfError::InvalidStructure(
            "Cannot delete the only page in document".into(),
        ));
    }

    let page_id = *page_ids
        .get(index)
        .ok_or_else(|| PdfError::InvalidStructure(format!("Page index {} out of range", index)))?;

    // Remove from Pages Kids array
    let pages_id = doc
        .pages_root_id()
        .ok_or_else(|| PdfError::InvalidStructure("Missing /Pages root".into()))?;

    let pages_obj = doc
        .objects
        .get_mut(&pages_id)
        .ok_or(PdfError::ObjectNotFound(pages_id.number, pages_id.generation))?;

    let dict = pages_obj
        .as_dict_mut()
        .ok_or_else(|| PdfError::InvalidStructure("Expected pages dictionary".into()))?;

    if let Some(kids) = dict.get_array_mut("Kids") {
        kids.retain(|k| k.as_reference() != Some(page_id));
        let count = kids.len() as i64;
        dict.insert("Count", count);
    }

    // Clean up page object
    doc.objects.remove(&page_id);

    Ok(())
}

/// Reorders pages according to the provided index array.
pub fn reorder_pages(doc: &mut PdfDocument, new_order: &[usize]) -> Result<()> {
    let current_page_ids = doc.page_ids();
    if new_order.len() != current_page_ids.len() {
        return Err(PdfError::InvalidStructure(
            "New order length must match page count".into(),
        ));
    }

    let mut reordered_ids = Vec::with_capacity(new_order.len());
    for &idx in new_order {
        let id = current_page_ids.get(idx).ok_or_else(|| {
            PdfError::InvalidStructure(format!("Page index {} out of bounds", idx))
        })?;
        reordered_ids.push(*id);
    }

    let pages_id = doc
        .pages_root_id()
        .ok_or_else(|| PdfError::InvalidStructure("Missing /Pages root".into()))?;

    let pages_obj = doc
        .objects
        .get_mut(&pages_id)
        .ok_or(PdfError::ObjectNotFound(pages_id.number, pages_id.generation))?;

    let dict = pages_obj
        .as_dict_mut()
        .ok_or_else(|| PdfError::InvalidStructure("Expected pages dictionary".into()))?;

    let kids_array: Vec<Object> = reordered_ids.into_iter().map(Object::Reference).collect();
    dict.insert("Kids", kids_array);

    Ok(())
}

/// Extracts specified page indices into a new standalone PdfDocument.
pub fn extract_pages(src_doc: &PdfDocument, page_indices: &[usize]) -> Result<PdfDocument> {
    let mut new_doc = PdfDocument::new();
    let src_page_ids = src_doc.page_ids();

    for &idx in page_indices {
        let _src_page_id = *src_page_ids
            .get(idx)
            .ok_or_else(|| PdfError::InvalidStructure(format!("Page index {} out of range", idx)))?;

        let src_page = src_doc
            .get_page(idx)
            .ok_or_else(|| PdfError::InvalidStructure(format!("Cannot get page {}", idx)))?;

        let content = src_doc.get_page_content(idx).unwrap_or_default();
        let box_w = (src_page.media_box[2] - src_page.media_box[0]).abs();
        let box_h = (src_page.media_box[3] - src_page.media_box[1]).abs();

        new_doc.add_page_with_content(box_w, box_h, content);

        if src_page.rotate != 0 {
            let last_idx = new_doc.page_count() - 1;
            rotate_page(&mut new_doc, last_idx, src_page.rotate)?;
        }
    }

    Ok(new_doc)
}

fn insert_page_id_at(doc: &mut PdfDocument, index: usize, page_id: ObjectId) -> Result<()> {
    let pages_id = doc
        .pages_root_id()
        .ok_or_else(|| PdfError::InvalidStructure("Missing /Pages root".into()))?;

    let pages_obj = doc
        .objects
        .get_mut(&pages_id)
        .ok_or(PdfError::ObjectNotFound(pages_id.number, pages_id.generation))?;

    let dict = pages_obj
        .as_dict_mut()
        .ok_or_else(|| PdfError::InvalidStructure("Expected pages dictionary".into()))?;

    let count = {
        let kids = dict
            .map
            .entry(PdfName::new("Kids"))
            .or_insert_with(|| Object::Array(Vec::new()));

        if let Object::Array(arr) = kids {
            let insert_idx = index.min(arr.len());
            arr.insert(insert_idx, Object::Reference(page_id));
            Some(arr.len() as i64)
        } else {
            None
        }
    };

    if let Some(c) = count {
        dict.insert("Count", c);
    }

    Ok(())
}
