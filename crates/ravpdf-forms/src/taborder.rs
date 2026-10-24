//! Tab Order Configuration conforming to ISO 32000-1 Clause 12.7.4.4.
//!
//! Traceability:
//! - FR-11.2.7 (Tab order configuration: row-based, column-based, document structure, manual)

use crate::field::{get_page_form_fields, FormField};
use ravpdf_core::document::PdfDocument;
use ravpdf_core::error::{PdfError, Result};
use ravpdf_core::object::{Object, PdfName};
use serde::{Deserialize, Serialize};

/// Page tab order navigation modes (FR-11.2.7, ISO 32000-1 Section 12.7.4.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum TabOrder {
    /// Row order (/R): Top-to-bottom, left-to-right across lines
    #[default]
    Row,
    /// Column order (/C): Left-to-right, top-to-bottom down vertical columns
    Column,
    /// Structure order (/S): Conforms to the document logical structure tree
    Structure,
    /// Manual / Array order (/A): Explicitly preserves custom widget array order
    Manual,
}

impl TabOrder {
    pub fn name(&self) -> &'static str {
        match self {
            TabOrder::Row => "R",
            TabOrder::Column => "C",
            TabOrder::Structure => "S",
            TabOrder::Manual => "A",
        }
    }

    pub fn from_name(name: &str) -> Self {
        match name {
            "R" => TabOrder::Row,
            "C" => TabOrder::Column,
            "S" => TabOrder::Structure,
            "A" => TabOrder::Manual,
            _ => TabOrder::Row,
        }
    }
}

/// Sorts a slice of form fields according to the specified tab order algorithm (FR-11.2.7).
pub fn sort_fields_by_tab_order(fields: &mut [FormField], order: TabOrder) {
    match order {
        TabOrder::Row => {
            // Group fields by row with a vertical band tolerance of 12.0 points
            fields.sort_by(|a, b| {
                let y_a = a.rect[1];
                let y_b = b.rect[1];
                let diff_y = (y_a - y_b).abs();

                if diff_y < 12.0 {
                    // Same visual row -> order left-to-right (X ascending)
                    a.rect[0].partial_cmp(&b.rect[0]).unwrap_or(std::cmp::Ordering::Equal)
                } else {
                    // Different rows -> order top-to-bottom (Y descending in PDF coordinate space)
                    y_b.partial_cmp(&y_a).unwrap_or(std::cmp::Ordering::Equal)
                }
            });
        }
        TabOrder::Column => {
            // Group fields by column with a horizontal band tolerance of 15.0 points
            fields.sort_by(|a, b| {
                let x_a = a.rect[0];
                let x_b = b.rect[0];
                let diff_x = (x_a - x_b).abs();

                if diff_x < 15.0 {
                    // Same visual column -> order top-to-bottom (Y descending)
                    b.rect[1].partial_cmp(&a.rect[1]).unwrap_or(std::cmp::Ordering::Equal)
                } else {
                    // Different columns -> order left-to-right (X ascending)
                    x_a.partial_cmp(&x_b).unwrap_or(std::cmp::Ordering::Equal)
                }
            });
        }
        TabOrder::Structure | TabOrder::Manual => {
            // Keep existing order
        }
    }
}

/// Applies tab order mode and reorders widget annotations for the specified page (FR-11.2.7).
pub fn apply_page_tab_order(
    doc: &mut PdfDocument,
    page_index: usize,
    order: TabOrder,
) -> Result<()> {
    let page_ids = doc.page_ids();
    let page_id = *page_ids
        .get(page_index)
        .ok_or_else(|| PdfError::InvalidStructure(format!("Page {} out of range", page_index)))?;

    // 1. Get and sort page form fields
    let mut page_fields = get_page_form_fields(doc, page_index)?;
    sort_fields_by_tab_order(&mut page_fields, order);

    let sorted_widget_ids: Vec<ravpdf_core::object::ObjectId> =
        page_fields.iter().filter_map(|f| f.id).collect();

    // 2. Update page dictionary
    let page_obj = doc
        .objects
        .get_mut(&page_id)
        .ok_or(PdfError::ObjectNotFound(page_id.number, page_id.generation))?;
    let page_dict = page_obj
        .as_dict_mut()
        .ok_or_else(|| PdfError::InvalidStructure("Expected page dictionary".into()))?;

    // Set /Tabs entry conforming to ISO 32000-1 Table 30
    page_dict.insert("Tabs", PdfName::new(order.name()));

    // Reorder /Annots: place sorted widgets first, followed by non-widget annotations
    if let Some(annots) = page_dict.get_array_mut("Annots") {
        let mut non_widgets = Vec::new();
        for item in annots.iter() {
            if let Some(id) = item.as_reference() {
                if !sorted_widget_ids.contains(&id) {
                    non_widgets.push(item.clone());
                }
            } else {
                non_widgets.push(item.clone());
            }
        }

        let mut new_annots: Vec<Object> = sorted_widget_ids
            .into_iter()
            .map(Object::Reference)
            .collect();
        new_annots.extend(non_widgets);

        page_dict.insert("Annots", Object::Array(new_annots));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::field::{add_form_field, FormField};

    #[test]
    fn test_tab_order_row_and_column_sorting() {
        let mut doc = PdfDocument::new();
        doc.add_blank_page(612.0, 792.0);

        // Row 1 (y = 700): Left (x = 50), Right (x = 300)
        let f_r1_right = FormField::new_text("R1_Right", [300.0, 700.0, 450.0, 720.0]);
        let f_r1_left = FormField::new_text("R1_Left", [50.0, 700.0, 200.0, 720.0]);

        // Row 2 (y = 650): Left (x = 50), Right (x = 300)
        let f_r2_right = FormField::new_text("R2_Right", [300.0, 650.0, 450.0, 670.0]);
        let f_r2_left = FormField::new_text("R2_Left", [50.0, 650.0, 200.0, 670.0]);

        // Add them out of order
        add_form_field(&mut doc, 0, &f_r2_right).unwrap();
        add_form_field(&mut doc, 0, &f_r1_right).unwrap();
        add_form_field(&mut doc, 0, &f_r2_left).unwrap();
        add_form_field(&mut doc, 0, &f_r1_left).unwrap();

        // Apply Row-based Tab Order
        apply_page_tab_order(&mut doc, 0, TabOrder::Row).expect("apply row tab order");

        let fields = get_page_form_fields(&doc, 0).expect("get page fields");
        let mut sorted = fields;
        sort_fields_by_tab_order(&mut sorted, TabOrder::Row);

        let names: Vec<&str> = sorted.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, vec!["R1_Left", "R1_Right", "R2_Left", "R2_Right"]);

        // Apply Column-based Tab Order
        sort_fields_by_tab_order(&mut sorted, TabOrder::Column);
        let col_names: Vec<&str> = sorted.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(col_names, vec!["R1_Left", "R2_Left", "R1_Right", "R2_Right"]);
    }
}
