//! Named Destinations conforming to ISO 32000-1 Section 12.3.2.3.
//!
//! Traceability: FR-20.4 (The system SHALL support creating and managing named destinations for cross-document linking).

use crate::document::PdfDocument;
use crate::error::{PdfError, Result};
use crate::object::{Object, ObjectId, PdfDictionary, PdfName};
use serde::{Deserialize, Serialize};

/// Type of destination view conforming to ISO 32000-1 Table 151.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum DestinationView {
    /// /XYZ left top zoom
    Xyz {
        left: Option<f64>,
        top: Option<f64>,
        zoom: Option<f64>,
    },
    /// /Fit (fits page to window)
    Fit,
    /// /FitH top (fits page width to window at vertical coordinate top)
    FitH { top: Option<f64> },
    /// /FitV left (fits page height to window at horizontal coordinate left)
    FitV { left: Option<f64> },
    /// /FitR left bottom right top (fits rectangle to window)
    FitR { rect: [f64; 4] },
    /// /FitB (fits bounding box to window)
    FitB,
    /// /FitBH top (fits bounding box width to window)
    FitBh { top: Option<f64> },
    /// /FitBV left (fits bounding box height to window)
    FitBv { left: Option<f64> },
}

/// A named destination in a PDF document (FR-20.4).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NamedDestination {
    pub name: String,
    pub page_index: usize,
    pub view: DestinationView,
}

impl NamedDestination {
    pub fn new(name: impl Into<String>, page_index: usize, view: DestinationView) -> Self {
        Self {
            name: name.into(),
            page_index,
            view,
        }
    }
}

impl DestinationView {
    /// Serializes destination view into an ISO 32000-1 destination array.
    pub fn to_array(&self, page_id: ObjectId) -> Vec<Object> {
        let mut arr = vec![Object::Reference(page_id)];
        match self {
            DestinationView::Xyz { left, top, zoom } => {
                arr.push(Object::Name(PdfName::new("XYZ")));
                arr.push(left.map(Object::Real).unwrap_or(Object::Null));
                arr.push(top.map(Object::Real).unwrap_or(Object::Null));
                arr.push(zoom.map(Object::Real).unwrap_or(Object::Null));
            }
            DestinationView::Fit => {
                arr.push(Object::Name(PdfName::new("Fit")));
            }
            DestinationView::FitH { top } => {
                arr.push(Object::Name(PdfName::new("FitH")));
                arr.push(top.map(Object::Real).unwrap_or(Object::Null));
            }
            DestinationView::FitV { left } => {
                arr.push(Object::Name(PdfName::new("FitV")));
                arr.push(left.map(Object::Real).unwrap_or(Object::Null));
            }
            DestinationView::FitR { rect } => {
                arr.push(Object::Name(PdfName::new("FitR")));
                arr.push(Object::Real(rect[0]));
                arr.push(Object::Real(rect[1]));
                arr.push(Object::Real(rect[2]));
                arr.push(Object::Real(rect[3]));
            }
            DestinationView::FitB => {
                arr.push(Object::Name(PdfName::new("FitB")));
            }
            DestinationView::FitBh { top } => {
                arr.push(Object::Name(PdfName::new("FitBH")));
                arr.push(top.map(Object::Real).unwrap_or(Object::Null));
            }
            DestinationView::FitBv { left } => {
                arr.push(Object::Name(PdfName::new("FitBV")));
                arr.push(left.map(Object::Real).unwrap_or(Object::Null));
            }
        }
        arr
    }

    /// Parses destination array conforming to ISO 32000-1 Clause 12.3.2.
    pub fn from_array(arr: &[Object]) -> Option<Self> {
        if arr.len() < 2 {
            return None;
        }
        let mode_name = match &arr[1] {
            Object::Name(n) => n.as_str(),
            _ => return None,
        };
        match mode_name {
            "XYZ" => {
                let left = arr.get(2).and_then(Object::as_f64);
                let top = arr.get(3).and_then(Object::as_f64);
                let zoom = arr.get(4).and_then(Object::as_f64);
                Some(DestinationView::Xyz { left, top, zoom })
            }
            "Fit" => Some(DestinationView::Fit),
            "FitH" => {
                let top = arr.get(2).and_then(Object::as_f64);
                Some(DestinationView::FitH { top })
            }
            "FitV" => {
                let left = arr.get(2).and_then(Object::as_f64);
                Some(DestinationView::FitV { left })
            }
            "FitR" => {
                if arr.len() >= 6 {
                    let r0 = arr[2].as_f64().unwrap_or(0.0);
                    let r1 = arr[3].as_f64().unwrap_or(0.0);
                    let r2 = arr[4].as_f64().unwrap_or(0.0);
                    let r3 = arr[5].as_f64().unwrap_or(0.0);
                    Some(DestinationView::FitR {
                        rect: [r0, r1, r2, r3],
                    })
                } else {
                    Some(DestinationView::Fit)
                }
            }
            "FitB" => Some(DestinationView::FitB),
            "FitBH" => {
                let top = arr.get(2).and_then(Object::as_f64);
                Some(DestinationView::FitBh { top })
            }
            "FitBV" => {
                let left = arr.get(2).and_then(Object::as_f64);
                Some(DestinationView::FitBv { left })
            }
            _ => Some(DestinationView::Fit),
        }
    }
}

/// Retrieves all named destinations defined in the document (FR-20.4).
pub fn get_named_destinations(doc: &PdfDocument) -> Result<Vec<NamedDestination>> {
    let catalog = match doc.catalog() {
        Some(c) => c,
        None => return Ok(Vec::new()),
    };

    let page_ids = doc.page_ids();
    let mut results = Vec::new();

    // 1. Check Catalog /Dests dictionary (PDF 1.1+)
    if let Some(dests_dict) = catalog.get_dict("Dests") {
        for (name, val) in &dests_dict.map {
            if let Some(dest_arr) = val.as_array() {
                if let Some(dest) = parse_destination_array(dest_arr, &page_ids, name.as_str()) {
                    results.push(dest);
                }
            } else if let Some(dest_obj_id) = val.as_reference() {
                if let Some(Object::Array(arr)) = doc.objects.get(&dest_obj_id) {
                    if let Some(dest) = parse_destination_array(arr, &page_ids, name.as_str()) {
                        results.push(dest);
                    }
                }
            }
        }
    }

    // 2. Check Catalog /Names /Dests name tree (PDF 1.2+)
    if let Some(names_dict) = catalog.get_dict("Names") {
        if let Some(dests_obj) = names_dict.get("Dests") {
            let dests_tree_dict = match dests_obj {
                Object::Dictionary(d) => Some(d),
                Object::Reference(id) => doc.objects.get(id).and_then(Object::as_dict),
                _ => None,
            };
            if let Some(tree) = dests_tree_dict {
                if let Some(arr) = tree.get_array("Names") {
                    let mut i = 0;
                    while i + 1 < arr.len() {
                        let name_str = match &arr[i] {
                            Object::String(s) => s.to_string_lossy(),
                            Object::Name(n) => n.as_str().to_string(),
                            _ => String::new(),
                        };
                        if !name_str.is_empty() {
                            let dest_val = &arr[i + 1];
                            if let Some(d_arr) = dest_val.as_array() {
                                if let Some(dest) = parse_destination_array(d_arr, &page_ids, &name_str) {
                                    if !results.iter().any(|r| r.name == name_str) {
                                        results.push(dest);
                                    }
                                }
                            } else if let Some(id) = dest_val.as_reference() {
                                if let Some(Object::Array(d_arr)) = doc.objects.get(&id) {
                                    if let Some(dest) = parse_destination_array(d_arr, &page_ids, &name_str) {
                                        if !results.iter().any(|r| r.name == name_str) {
                                            results.push(dest);
                                        }
                                    }
                                }
                            }
                        }
                        i += 2;
                    }
                }
            }
        }
    }

    results.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(results)
}

fn parse_destination_array(
    arr: &[Object],
    page_ids: &[ObjectId],
    name: &str,
) -> Option<NamedDestination> {
    if arr.is_empty() {
        return None;
    }
    let target_page_id = arr[0].as_reference()?;
    let page_index = page_ids.iter().position(|&id| id == target_page_id)?;
    let view = DestinationView::from_array(arr)?;
    Some(NamedDestination {
        name: name.to_string(),
        page_index,
        view,
    })
}

/// Adds or updates a named destination in the document catalog (FR-20.4).
pub fn add_named_destination(
    doc: &mut PdfDocument,
    name: &str,
    page_index: usize,
    view: &DestinationView,
) -> Result<()> {
    let page_ids = doc.page_ids();
    let page_id = *page_ids
        .get(page_index)
        .ok_or_else(|| PdfError::InvalidStructure(format!("Page index {} out of range", page_index)))?;

    let dest_arr = view.to_array(page_id);
    let root_id = doc.root_id;

    let root_obj = doc
        .objects
        .get_mut(&root_id)
        .ok_or_else(|| PdfError::InvalidStructure("Missing document catalog".into()))?;

    let catalog = root_obj
        .as_dict_mut()
        .ok_or_else(|| PdfError::InvalidStructure("Catalog is not a dictionary".into()))?;

    // Update /Dests dictionary in catalog
    if !catalog.contains_key("Dests") {
        catalog.insert("Dests", Object::Dictionary(PdfDictionary::new()));
    }

    if let Some(Object::Dictionary(ref mut dests_dict)) = catalog.get_mut("Dests") {
        dests_dict.insert(name, Object::Array(dest_arr));
    }

    doc.mark_dirty(root_id);
    Ok(())
}

/// Removes a named destination by name (FR-20.4). Returns true if it was found and removed.
pub fn remove_named_destination(doc: &mut PdfDocument, name: &str) -> Result<bool> {
    let root_id = doc.root_id;
    let root_obj = match doc.objects.get_mut(&root_id) {
        Some(o) => o,
        None => return Ok(false),
    };
    let catalog = match root_obj.as_dict_mut() {
        Some(c) => c,
        None => return Ok(false),
    };

    let mut removed = false;

    // Check /Dests
    if let Some(Object::Dictionary(ref mut dests_dict)) = catalog.get_mut("Dests") {
        if dests_dict.map.remove(&PdfName::new(name)).is_some() {
            removed = true;
        }
    }

    // Check /Names /Dests
    if let Some(Object::Dictionary(ref mut names_dict)) = catalog.get_mut("Names") {
        if let Some(Object::Dictionary(ref mut dests_tree)) = names_dict.get_mut("Dests") {
            if let Some(Object::Array(ref mut arr)) = dests_tree.get_mut("Names") {
                let mut idx = None;
                for i in (0..arr.len()).step_by(2) {
                    if let Some(s) = arr.get(i).and_then(Object::as_str) {
                        if s == name {
                            idx = Some(i);
                            break;
                        }
                    }
                }
                if let Some(i) = idx {
                    arr.remove(i); // remove value
                    if i < arr.len() {
                        arr.remove(i); // remove key
                    }
                    removed = true;
                }
            }
        }
    }

    if removed {
        doc.mark_dirty(root_id);
    }

    Ok(removed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_named_destinations_lifecycle() {
        let mut doc = PdfDocument::new();
        doc.add_blank_page(612.0, 792.0); // Page 0
        doc.add_blank_page(612.0, 792.0); // Page 1
        doc.add_blank_page(612.0, 792.0); // Page 2

        // 1. Add Named Destinations
        add_named_destination(&mut doc, "Chapter1", 0, &DestinationView::Fit).expect("add ch1");
        add_named_destination(
            &mut doc,
            "Section2",
            1,
            &DestinationView::Xyz {
                left: Some(50.0),
                top: Some(700.0),
                zoom: Some(1.5),
            },
        )
        .expect("add sec2");
        add_named_destination(&mut doc, "Appendix", 2, &DestinationView::FitH { top: Some(600.0) })
            .expect("add app");

        // 2. Retrieve Destinations
        let dests = get_named_destinations(&doc).expect("get dests");
        assert_eq!(dests.len(), 3);
        assert_eq!(dests[0].name, "Appendix");
        assert_eq!(dests[0].page_index, 2);
        assert_eq!(dests[1].name, "Chapter1");
        assert_eq!(dests[1].page_index, 0);
        assert_eq!(dests[1].view, DestinationView::Fit);
        assert_eq!(dests[2].name, "Section2");
        assert_eq!(dests[2].page_index, 1);

        // 3. Remove Destination
        let removed = remove_named_destination(&mut doc, "Chapter1").expect("remove ch1");
        assert!(removed);
        let dests_after = get_named_destinations(&doc).expect("get dests after remove");
        assert_eq!(dests_after.len(), 2);
        assert!(!dests_after.iter().any(|d| d.name == "Chapter1"));
    }
}
