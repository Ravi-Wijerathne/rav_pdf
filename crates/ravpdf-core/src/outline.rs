//! Document Outlines (Bookmarks) conforming to ISO 32000-1 Section 12.3.3.
//!
//! Traceability: FR-01.2.4, FR-19.1 - FR-19.8.

use crate::document::PdfDocument;
use crate::error::{PdfError, Result};
use crate::object::{Object, ObjectId, PdfDictionary, PdfName, PdfString};
use serde::{Deserialize, Serialize};

/// A hierarchical outline item / bookmark in a PDF document.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Bookmark {
    pub title: String,
    pub page_index: Option<usize>,
    pub children: Vec<Bookmark>,
    pub is_bold: bool,
    pub is_italic: bool,
    pub color: Option<(f32, f32, f32)>,
}

impl Bookmark {
    pub fn new(title: impl Into<String>, page_index: Option<usize>) -> Self {
        Self {
            title: title.into(),
            page_index,
            children: Vec::new(),
            is_bold: false,
            is_italic: false,
            color: None,
        }
    }
}

/// Extracts all document outlines / bookmarks in hierarchical order.
pub fn get_document_bookmarks(doc: &PdfDocument) -> Result<Vec<Bookmark>> {
    let catalog = match doc.catalog() {
        Some(c) => c,
        None => return Ok(Vec::new()),
    };

    let outlines_ref = match catalog.get_ref("Outlines") {
        Some(r) => r,
        None => return Ok(Vec::new()),
    };

    let outlines_dict = match doc.objects.get(&outlines_ref).and_then(Object::as_dict) {
        Some(d) => d,
        None => return Ok(Vec::new()),
    };

    let page_ids = doc.page_ids();
    let mut bookmarks = Vec::new();

    if let Some(first_ref) = outlines_dict.get_ref("First") {
        collect_outline_siblings(doc, first_ref, &page_ids, &mut bookmarks);
    }

    Ok(bookmarks)
}

fn collect_outline_siblings(
    doc: &PdfDocument,
    mut curr_id: ObjectId,
    page_ids: &[ObjectId],
    out: &mut Vec<Bookmark>,
) {
    let mut visited = std::collections::HashSet::new();

    loop {
        if !visited.insert(curr_id) {
            break; // Prevent circular reference loops
        }

        let item_dict = match doc.objects.get(&curr_id).and_then(Object::as_dict) {
            Some(d) => d,
            None => break,
        };

        let title = item_dict
            .get_str("Title")
            .unwrap_or_else(|| "Untitled Bookmark".into());

        let page_index = resolve_destination_page(doc, item_dict, page_ids);

        let flags = item_dict.get_i64("F").unwrap_or(0);
        let is_italic = (flags & 1) != 0;
        let is_bold = (flags & 2) != 0;

        let color = if let Some(c_arr) = item_dict.get_array("C") {
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

        let mut children = Vec::new();
        if let Some(first_child) = item_dict.get_ref("First") {
            collect_outline_siblings(doc, first_child, page_ids, &mut children);
        }

        out.push(Bookmark {
            title,
            page_index,
            children,
            is_bold,
            is_italic,
            color,
        });

        // Move to next sibling
        match item_dict.get_ref("Next") {
            Some(next_id) => curr_id = next_id,
            None => break,
        }
    }
}

fn resolve_destination_page(
    doc: &PdfDocument,
    dict: &PdfDictionary,
    page_ids: &[ObjectId],
) -> Option<usize> {
    // 1. Check direct /Dest
    if let Some(dest_obj) = dict.get("Dest") {
        if let Some(idx) = extract_page_from_dest(dest_obj, page_ids) {
            return Some(idx);
        }
    }

    // 2. Check Action /A << /S /GoTo /D [...] >>
    if let Some(a_obj) = dict.get("A") {
        let action_dict = match a_obj {
            Object::Dictionary(d) => Some(d),
            Object::Reference(id) => doc.objects.get(id).and_then(Object::as_dict),
            _ => None,
        };

        if let Some(act) = action_dict {
            if act.get_name("S") == Some("GoTo") {
                if let Some(d_obj) = act.get("D") {
                    if let Some(idx) = extract_page_from_dest(d_obj, page_ids) {
                        return Some(idx);
                    }
                }
            }
        }
    }

    None
}

fn extract_page_from_dest(dest_obj: &Object, page_ids: &[ObjectId]) -> Option<usize> {
    match dest_obj {
        Object::Array(arr) => {
            if let Some(first) = arr.first() {
                if let Some(page_ref) = first.as_reference() {
                    return page_ids.iter().position(|&id| id == page_ref);
                }
                if let Some(num) = first.as_i64() {
                    if num >= 0 && (num as usize) < page_ids.len() {
                        return Some(num as usize);
                    }
                }
            }
        }
        Object::Reference(id) => {
            return page_ids.iter().position(|r| r == id);
        }
        _ => {}
    }
    None
}

/// Adds a root-level or child bookmark pointing to `page_index`.
pub fn add_bookmark(doc: &mut PdfDocument, title: &str, page_index: usize) -> Result<ObjectId> {
    let page_ids = doc.page_ids();
    let page_id = *page_ids
        .get(page_index)
        .ok_or_else(|| PdfError::InvalidStructure(format!("Page {} out of range", page_index)))?;

    let root_id = doc.root_id;
    let existing_outlines_id = doc
        .objects
        .get(&root_id)
        .and_then(Object::as_dict)
        .and_then(|d| d.get_ref("Outlines"));

    let outlines_id = if let Some(id) = existing_outlines_id {
        id
    } else {
        let new_id = doc.next_object_id();
        let mut ol_dict = PdfDictionary::new();
        ol_dict.insert("Type", PdfName::new("Outlines"));
        ol_dict.insert("Count", 0);
        doc.objects.insert(new_id, Object::Dictionary(ol_dict));

        if let Some(root_obj) = doc.objects.get_mut(&root_id) {
            if let Some(root_dict) = root_obj.as_dict_mut() {
                root_dict.insert("Outlines", new_id);
            }
        }
        new_id
    };

    let item_id = doc.next_object_id();

    // Create the bookmark item
    let mut item_dict = PdfDictionary::new();
    item_dict.insert("Title", PdfString::from_str(title));
    item_dict.insert("Parent", outlines_id);
    item_dict.insert(
        "Dest",
        vec![
            Object::Reference(page_id),
            Object::Name(PdfName::new("Fit")),
        ],
    );

    // Link into Outlines list (/First, /Last, /Count)
    let outlines_obj = doc
        .objects
        .get_mut(&outlines_id)
        .ok_or(PdfError::ObjectNotFound(outlines_id.number, outlines_id.generation))?;
    let outlines_dict = outlines_obj
        .as_dict_mut()
        .ok_or_else(|| PdfError::InvalidStructure("Expected outlines dict".into()))?;

    let last_id = outlines_dict.get_ref("Last");

    if let Some(prev_last_id) = last_id {
        item_dict.insert("Prev", prev_last_id);
        outlines_dict.insert("Last", item_id);

        let count = outlines_dict.get_i64("Count").unwrap_or(0);
        outlines_dict.insert("Count", count + 1);

        // Update previous item's /Next
        doc.objects.insert(item_id, Object::Dictionary(item_dict));
        if let Some(prev_obj) = doc.objects.get_mut(&prev_last_id) {
            if let Some(prev_dict) = prev_obj.as_dict_mut() {
                prev_dict.insert("Next", item_id);
            }
        }
    } else {
        // First item
        outlines_dict.insert("First", item_id);
        outlines_dict.insert("Last", item_id);
        outlines_dict.insert("Count", 1);
        doc.objects.insert(item_id, Object::Dictionary(item_dict));
    }

    Ok(item_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_and_get_bookmarks() {
        let mut doc = PdfDocument::new();
        doc.add_blank_page(612.0, 792.0);
        doc.add_blank_page(612.0, 792.0);

        add_bookmark(&mut doc, "Chapter 1: Introduction", 0).expect("add bm 1");
        add_bookmark(&mut doc, "Chapter 2: Methods", 1).expect("add bm 2");

        let bookmarks = get_document_bookmarks(&doc).expect("get bookmarks");
        assert_eq!(bookmarks.len(), 2);
        assert_eq!(bookmarks[0].title, "Chapter 1: Introduction");
        assert_eq!(bookmarks[0].page_index, Some(0));
        assert_eq!(bookmarks[1].title, "Chapter 2: Methods");
        assert_eq!(bookmarks[1].page_index, Some(1));
    }
}
