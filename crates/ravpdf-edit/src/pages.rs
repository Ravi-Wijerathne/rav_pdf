//! Page Management operations conforming to ISO 32000-1 Section 7.7.3.
//!
//! Traceability: FR-06.1 - FR-06.11, FR-07.

use ravpdf_core::document::PdfDocument;
use ravpdf_core::error::{PdfError, Result};
use ravpdf_core::object::{ObjectId, Object, PdfDictionary, PdfName};
use serde::{Deserialize, Serialize};

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

/// Page boundary box types conforming to ISO 32000-1 Clause 14.11.2 (FR-06.8).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PageBoxType {
    MediaBox,
    CropBox,
    BleedBox,
    TrimBox,
    ArtBox,
}

impl PageBoxType {
    pub fn pdf_key(&self) -> &'static str {
        match self {
            PageBoxType::MediaBox => "MediaBox",
            PageBoxType::CropBox => "CropBox",
            PageBoxType::BleedBox => "BleedBox",
            PageBoxType::TrimBox => "TrimBox",
            PageBoxType::ArtBox => "ArtBox",
        }
    }
}

/// The set of all 5 standard page boundary boxes for a page (ISO 32000-1 Clause 14.11.2 / FR-06.8).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PageBoxes {
    pub media_box: [f64; 4],
    pub crop_box: [f64; 4],
    pub bleed_box: [f64; 4],
    pub trim_box: [f64; 4],
    pub art_box: [f64; 4],
}

/// Sets any standard page boundary box on a page (FR-06.8).
pub fn set_page_box(
    doc: &mut PdfDocument,
    index: usize,
    box_type: PageBoxType,
    rect: [f64; 4],
) -> Result<()> {
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
        box_type.pdf_key(),
        vec![
            Object::Real(rect[0]),
            Object::Real(rect[1]),
            Object::Real(rect[2]),
            Object::Real(rect[3]),
        ],
    );

    doc.mark_dirty(page_id);
    Ok(())
}

/// Retrieves all standard page boundary boxes for a page, applying ISO 32000-1 defaults (FR-06.8).
pub fn get_page_boxes(doc: &PdfDocument, index: usize) -> Result<PageBoxes> {
    let page = doc
        .get_page(index)
        .ok_or_else(|| PdfError::InvalidStructure(format!("Page index {} out of range", index)))?;

    let media = page.media_box;
    let crop = page.crop_box;
    let bleed = page.bleed_box.unwrap_or(crop);
    let trim = page.trim_box.unwrap_or(crop);
    let art = page.art_box.unwrap_or(crop);

    Ok(PageBoxes {
        media_box: media,
        crop_box: crop,
        bleed_box: bleed,
        trim_box: trim,
        art_box: art,
    })
}

/// Sets the `/CropBox` for a page (FR-06.8).
pub fn crop_page(doc: &mut PdfDocument, index: usize, crop_box: [f64; 4]) -> Result<()> {
    set_page_box(doc, index, PageBoxType::CropBox, crop_box)
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

/// Duplicates the page at `index` (0-based) and inserts the duplicate immediately after it (at `index + 1`).
///
/// Deep-copies the page dictionary (MediaBox, CropBox, Rotate, Resources, and cloned content streams).
/// Conforms to ISO 32000-1 Clause 7.7.3.3. Traceability: FR-06.11.
pub fn duplicate_page(doc: &mut PdfDocument, index: usize) -> Result<ObjectId> {
    let page_ids = doc.page_ids();
    let src_id = *page_ids
        .get(index)
        .ok_or_else(|| PdfError::InvalidStructure(format!("Page index {} out of range", index)))?;

    let src_dict = doc
        .objects
        .get(&src_id)
        .and_then(Object::as_dict)
        .ok_or_else(|| PdfError::InvalidStructure("Expected page dictionary".into()))?
        .clone();

    // Clone content streams so edits on one do not unintentionally modify the other
    let mut new_contents = Vec::new();
    if let Some(contents_obj) = src_dict.get("Contents") {
        match contents_obj {
            Object::Reference(id) => {
                if let Some(obj) = doc.objects.get(id) {
                    let new_id = doc.next_object_id();
                    doc.objects.insert(new_id, obj.clone());
                    new_contents.push(Object::Reference(new_id));
                }
            }
            Object::Array(arr) => {
                for item in arr {
                    if let Some(id) = item.as_reference() {
                        if let Some(obj) = doc.objects.get(&id) {
                            let new_id = doc.next_object_id();
                            doc.objects.insert(new_id, obj.clone());
                            new_contents.push(Object::Reference(new_id));
                        }
                    }
                }
            }
            _ => {}
        }
    }

    let new_page_id = doc.next_object_id();
    let mut new_page_dict = src_dict;

    if !new_contents.is_empty() {
        if new_contents.len() == 1 {
            new_page_dict.insert("Contents", new_contents[0].clone());
        } else {
            new_page_dict.insert("Contents", new_contents);
        }
    }

    doc.objects.insert(new_page_id, Object::Dictionary(new_page_dict));

    insert_page_id_at(doc, index + 1, new_page_id)?;

    Ok(new_page_id)
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

/// Inserts pages from another PDF document into this document at `target_index` (FR-06.3).
pub fn insert_pages_from_document(
    target_doc: &mut PdfDocument,
    target_index: usize,
    source_doc: &PdfDocument,
    source_indices: &[usize],
) -> Result<Vec<ObjectId>> {
    let pages_id = target_doc
        .pages_root_id()
        .ok_or_else(|| PdfError::InvalidStructure("Missing target /Pages root".into()))?;

    let mut inserted_ids = Vec::new();
    let src_page_ids = source_doc.page_ids();

    for (offset, &src_idx) in source_indices.iter().enumerate() {
        let src_page_id = *src_page_ids
            .get(src_idx)
            .ok_or_else(|| PdfError::InvalidStructure(format!("Source page {} out of range", src_idx)))?;

        let src_obj = source_doc
            .objects
            .get(&src_page_id)
            .ok_or(PdfError::ObjectNotFound(src_page_id.number, src_page_id.generation))?;

        let src_dict = src_obj
            .as_dict()
            .ok_or_else(|| PdfError::InvalidStructure("Expected source page dictionary".into()))?
            .clone();

        let mut new_page_dict = src_dict;
        new_page_dict.insert("Parent", pages_id);

        // Copy and remap content streams
        let mut new_contents = Vec::new();
        if let Some(contents_obj) = new_page_dict.get("Contents") {
            match contents_obj {
                Object::Reference(cid) => {
                    if let Some(c_stream) = source_doc.objects.get(cid) {
                        let new_cid = target_doc.next_object_id();
                        target_doc.objects.insert(new_cid, c_stream.clone());
                        new_contents.push(Object::Reference(new_cid));
                    }
                }
                Object::Array(arr) => {
                    for item in arr {
                        if let Some(cid) = item.as_reference() {
                            if let Some(c_stream) = source_doc.objects.get(&cid) {
                                let new_cid = target_doc.next_object_id();
                                target_doc.objects.insert(new_cid, c_stream.clone());
                                new_contents.push(Object::Reference(new_cid));
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        if !new_contents.is_empty() {
            if new_contents.len() == 1 {
                new_page_dict.insert("Contents", new_contents[0].clone());
            } else {
                new_page_dict.insert("Contents", new_contents);
            }
        }

        // Copy resources if indirect
        if let Some(Object::Reference(rid)) = new_page_dict.get("Resources") {
            if let Some(r_obj) = source_doc.objects.get(rid) {
                let new_rid = target_doc.next_object_id();
                target_doc.objects.insert(new_rid, r_obj.clone());
                new_page_dict.insert("Resources", new_rid);
            }
        }

        let new_page_id = target_doc.next_object_id();
        target_doc.objects.insert(new_page_id, Object::Dictionary(new_page_dict));

        insert_page_id_at(target_doc, target_index + offset, new_page_id)?;
        inserted_ids.push(new_page_id);
    }

    Ok(inserted_ids)
}

/// Replaces a page at `target_index` with a page from `source_doc` (FR-06.10).
pub fn replace_page(
    target_doc: &mut PdfDocument,
    target_index: usize,
    source_doc: &PdfDocument,
    source_page_index: usize,
) -> Result<()> {
    if target_index >= target_doc.page_count() {
        return Err(PdfError::InvalidStructure(format!(
            "Target page index {} out of range",
            target_index
        )));
    }

    insert_pages_from_document(target_doc, target_index, source_doc, &[source_page_index])?;
    delete_page(target_doc, target_index + 1)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_duplicate_and_manage_pages() {
        let mut doc = PdfDocument::new();
        doc.add_blank_page(612.0, 792.0);
        assert_eq!(doc.page_count(), 1);

        // Test insert blank page
        insert_blank_page(&mut doc, 1, 595.0, 842.0).expect("insert blank page");
        assert_eq!(doc.page_count(), 2);

        // Test duplicate page
        let dup_id = duplicate_page(&mut doc, 0).expect("duplicate page");
        assert_eq!(doc.page_count(), 3);
        assert!(doc.objects.contains_key(&dup_id));

        // Test rotate page
        rotate_page(&mut doc, 0, 90).expect("rotate page");
        let page0 = doc.get_page(0).expect("page 0");
        assert_eq!(page0.rotate, 90);

        // Test crop page
        crop_page(&mut doc, 0, [10.0, 10.0, 600.0, 780.0]).expect("crop page");
        let page0_cropped = doc.get_page(0).expect("page 0 cropped");
        assert_eq!(page0_cropped.crop_box, [10.0, 10.0, 600.0, 780.0]);

        // Test delete page
        delete_page(&mut doc, 1).expect("delete page");
        assert_eq!(doc.page_count(), 2);

        // Test all 5 Page Boundary Boxes (FR-06.8)
        set_page_box(&mut doc, 0, PageBoxType::BleedBox, [5.0, 5.0, 607.0, 787.0]).expect("bleed box");
        set_page_box(&mut doc, 0, PageBoxType::TrimBox, [15.0, 15.0, 597.0, 777.0]).expect("trim box");
        set_page_box(&mut doc, 0, PageBoxType::ArtBox, [25.0, 25.0, 587.0, 767.0]).expect("art box");

        let boxes = get_page_boxes(&doc, 0).expect("get boxes");
        assert_eq!(boxes.media_box, [0.0, 0.0, 612.0, 792.0]);
        assert_eq!(boxes.crop_box, [10.0, 10.0, 600.0, 780.0]);
        assert_eq!(boxes.bleed_box, [5.0, 5.0, 607.0, 787.0]);
        assert_eq!(boxes.trim_box, [15.0, 15.0, 597.0, 777.0]);
        assert_eq!(boxes.art_box, [25.0, 25.0, 587.0, 767.0]);

        // Test insert pages from another document (FR-06.3)
        let mut doc2 = PdfDocument::new();
        doc2.add_blank_page(500.0, 500.0);
        doc2.add_blank_page(400.0, 400.0);
        let inserted = insert_pages_from_document(&mut doc, 1, &doc2, &[0, 1]).expect("insert pages");
        assert_eq!(inserted.len(), 2);
        assert_eq!(doc.page_count(), 4);

        // Test replace page (FR-06.10)
        let mut replacement_src = PdfDocument::new();
        replacement_src.add_blank_page(300.0, 300.0);
        replace_page(&mut doc, 1, &replacement_src, 0).expect("replace page");
        assert_eq!(doc.page_count(), 4);
        let replaced_page = doc.get_page(1).expect("get replaced page");
        assert_eq!(replaced_page.media_box, [0.0, 0.0, 300.0, 300.0]);
    }
}

