//! PDF Portfolios conforming to ISO 32000-1 Clause 12.7.8 (Collections).
//!
//! Traceability:
//! - FR-29.2.1 (Bundle multiple heterogeneous files into a single portfolio container)
//! - FR-29.2.2 (Customizable portfolio layouts: Details, Tile, Custom)
//! - FR-29.2.3 (Sorting, searching, and filtering files within a portfolio)
//! - FR-29.2.4 (Extracting individual files from a portfolio)
//! - FR-29.2.5 (Adding and removing files from existing portfolios)

use anyhow::{anyhow, Result};
use ravpdf_core::document::PdfDocument;
use ravpdf_core::object::{Object, PdfDictionary, PdfName, PdfString};
use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::attachments::{add_attachment, get_attachments, remove_attachment, FileAttachment};

/// Portfolio layout mode conforming to ISO 32000-1 Clause 12.7.8 Table 155.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PortfolioLayout {
    /// Details / Table view with columns for filename, description, size, etc.
    Details,
    /// Tile / Card view with thumbnails
    Tile,
    /// Hidden / Custom view
    Custom,
}

impl PortfolioLayout {
    pub fn to_pdf_name(&self) -> &'static str {
        match self {
            PortfolioLayout::Details => "D",
            PortfolioLayout::Tile => "T",
            PortfolioLayout::Custom => "C",
        }
    }

    pub fn from_pdf_name(name: &str) -> Self {
        match name {
            "T" => PortfolioLayout::Tile,
            "C" => PortfolioLayout::Custom,
            _ => PortfolioLayout::Details,
        }
    }
}

/// Portfolio sorting criteria (FR-29.2.3).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PortfolioSort {
    pub field_name: String,
    pub ascending: bool,
}

impl Default for PortfolioSort {
    fn default() -> Self {
        Self {
            field_name: "FileName".into(),
            ascending: true,
        }
    }
}

/// A structured item in a PDF Portfolio (FR-29.2.1).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PortfolioItem {
    pub filename: String,
    pub description: Option<String>,
    pub size: usize,
    pub data: Vec<u8>,
    pub mime_type: Option<String>,
}

impl PortfolioItem {
    pub fn from_file_path(path: &Path, description: Option<&str>) -> Result<Self> {
        let filename = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("file.dat")
            .to_string();
        let data = std::fs::read(path)?;
        let size = data.len();

        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();
        let mime_type = match ext.as_str() {
            "pdf" => Some("application/pdf".into()),
            "png" => Some("image/png".into()),
            "jpg" | "jpeg" => Some("image/jpeg".into()),
            "txt" => Some("text/plain".into()),
            "csv" => Some("text/csv".into()),
            "xlsx" => Some("application/vnd.openxmlformats-officedocument.spreadsheetml.sheet".into()),
            "docx" => Some("application/vnd.openxmlformats-officedocument.wordprocessingml.document".into()),
            "mp4" => Some("video/mp4".into()),
            _ => Some("application/octet-stream".into()),
        };

        Ok(Self {
            filename,
            description: description.map(|s| s.to_string()),
            size,
            data,
            mime_type,
        })
    }
}

/// High-level representation of a PDF Portfolio (FR-29.2).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Portfolio {
    pub title: String,
    pub description: Option<String>,
    pub layout: PortfolioLayout,
    pub sort: PortfolioSort,
    pub items: Vec<PortfolioItem>,
}

/// Checks whether a PDF document is a PDF Portfolio conforming to ISO 32000-1 Clause 12.7.8.
pub fn is_portfolio(doc: &PdfDocument) -> bool {
    let catalog = match doc.objects.get(&doc.root_id).and_then(Object::as_dict) {
        Some(d) => d,
        None => return false,
    };
    catalog.contains_key("Collection")
}

/// Inspects and loads portfolio details and items from a PDF document (FR-29.2.1, FR-29.2.3).
pub fn get_portfolio(doc: &PdfDocument) -> Result<Portfolio> {
    let catalog = doc
        .objects
        .get(&doc.root_id)
        .and_then(Object::as_dict)
        .ok_or_else(|| anyhow!("Invalid document catalog"))?;

    let coll_dict = catalog
        .get_dict("Collection")
        .or_else(|| {
            catalog.get("Collection").and_then(|obj| doc.resolve(obj)).and_then(Object::as_dict)
        });

    let (layout, sort) = if let Some(coll) = coll_dict {
        let layout_str = coll.get_name("D").unwrap_or("D");
        let layout = PortfolioLayout::from_pdf_name(layout_str);

        let sort = if let Some(sort_dict) = coll.get_dict("Sort") {
            let field_name = sort_dict.get_name("S").unwrap_or("FileName").to_string();
            let ascending = sort_dict.get_bool("A").unwrap_or(true);
            PortfolioSort { field_name, ascending }
        } else {
            PortfolioSort::default()
        };
        (layout, sort)
    } else {
        (PortfolioLayout::Details, PortfolioSort::default())
    };

    // Extract title from DocumentInfo
    let title = doc
        .info_id
        .and_then(|id| doc.objects.get(&id))
        .and_then(Object::as_dict)
        .and_then(|d| d.get_str("Title"))
        .unwrap_or_else(|| "PDF Portfolio".into());

    let description = doc
        .info_id
        .and_then(|id| doc.objects.get(&id))
        .and_then(Object::as_dict)
        .and_then(|d| d.get_str("Subject"));

    // Extract all attached files
    let attachments: Vec<FileAttachment> = get_attachments(doc)?;
    let mut items: Vec<PortfolioItem> = attachments
        .into_iter()
        .map(|att| PortfolioItem {
            filename: att.filename,
            description: att.description,
            size: att.size,
            data: att.data,
            mime_type: None,
        })
        .collect();

    // Sort items according to portfolio sort specification
    sort_items(&mut items, &sort);

    Ok(Portfolio {
        title,
        description,
        layout,
        sort,
        items,
    })
}

/// Sorts portfolio items by field name (FR-29.2.3).
pub fn sort_items(items: &mut [PortfolioItem], sort: &PortfolioSort) {
    items.sort_by(|a, b| {
        let cmp = match sort.field_name.to_lowercase().as_str() {
            "size" => a.size.cmp(&b.size),
            "description" | "desc" => a
                .description
                .as_deref()
                .unwrap_or("")
                .cmp(b.description.as_deref().unwrap_or("")),
            _ => a.filename.to_lowercase().cmp(&b.filename.to_lowercase()),
        };
        if sort.ascending {
            cmp
        } else {
            cmp.reverse()
        }
    });
}

/// Filters / searches items in a portfolio by text query across filename and description (FR-29.2.3).
pub fn search_portfolio<'a>(portfolio: &'a Portfolio, query: &str) -> Vec<&'a PortfolioItem> {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return portfolio.items.iter().collect();
    }
    portfolio
        .items
        .iter()
        .filter(|item| {
            item.filename.to_lowercase().contains(&q)
                || item
                    .description
                    .as_deref()
                    .unwrap_or("")
                    .to_lowercase()
                    .contains(&q)
        })
        .collect()
}

/// Creates a new PDF Portfolio container document bundling multiple heterogeneous files (FR-29.2.1, FR-29.2.2).
pub fn create_portfolio(
    items: &[PortfolioItem],
    layout: PortfolioLayout,
    title: &str,
    description: Option<&str>,
) -> Result<PdfDocument> {
    let mut doc = PdfDocument::new();

    // 1. Create elegant cover sheet informing users that this is a PDF Portfolio
    doc.add_blank_page(612.0, 792.0);
    let page_id = doc.page_ids()[0];

    let mut stream_content = String::new();
    // Header Banner
    stream_content.push_str("q 0.08 0.18 0.36 rg 0 680 612 112 re f Q\n"); // Navy blue banner
    stream_content.push_str("BT /F1 24 Tf 1 1 1 rg 50 730 Td (PDF Portfolio) Tj ET\n");
    let safe_title = title.replace('(', "\\(").replace(')', "\\)");
    stream_content.push_str(&format!(
        "BT /F1 14 Tf 0.85 0.90 1.0 rg 50 705 Td ({}) Tj ET\n",
        safe_title
    ));

    // Notice card
    stream_content.push_str("q 0.95 0.96 0.98 rg 50 560 512 90 re f Q\n");
    stream_content.push_str("q 0.80 0.82 0.86 RG 1 w 50 560 512 90 re s Q\n");
    stream_content.push_str("BT /F1 12 Tf 0.15 0.15 0.2 rg 70 625 Td (This document is a PDF Portfolio bundling multiple files.) Tj ET\n");
    stream_content.push_str("BT /F1 10 Tf 0.35 0.35 0.4 rg 70 605 Td (Open the Portfolio or Attachments panel in your PDF viewer to view,) Tj ET\n");
    stream_content.push_str("BT /F1 10 Tf 0.35 0.35 0.4 rg 70 590 Td (extract, or inspect the embedded documents.) Tj ET\n");

    // Table of embedded files
    stream_content.push_str("BT /F1 13 Tf 0.1 0.1 0.1 rg 50 520 Td (Bundled Documents) Tj ET\n");
    stream_content.push_str("q 0.2 0.2 0.2 RG 0.5 w 50 510 512 0 re s Q\n");

    let mut y = 490.0;
    for (idx, item) in items.iter().take(15).enumerate() {
        let safe_name = item.filename.replace('(', "\\(").replace(')', "\\)");
        let size_kb = (item.size as f64) / 1024.0;
        let line = format!(
            "{}. {}  ({:.1} KB)",
            idx + 1,
            safe_name,
            size_kb
        );
        stream_content.push_str(&format!(
            "BT /F1 10 Tf 0.2 0.2 0.2 rg 60 {:.1} Td ({}) Tj ET\n",
            y, line
        ));
        y -= 22.0;
    }

    if items.len() > 15 {
        stream_content.push_str(&format!(
            "BT /F1 9 Tf 0.5 0.5 0.5 rg 60 {:.1} Td (... and {} more files) Tj ET\n",
            y,
            items.len() - 15
        ));
    }

    // Set page content stream
    let stream_obj = ravpdf_core::object::PdfStream::with_plain_data(
        PdfDictionary::new(),
        stream_content.into_bytes(),
    );
    let stream_id = doc.next_object_id();
    doc.objects.insert(stream_id, Object::Stream(stream_obj));

    if let Some(Object::Dictionary(page_dict)) = doc.objects.get_mut(&page_id) {
        page_dict.insert("Contents", stream_id);
    }

    // 2. Set Document Info
    let mut info = PdfDictionary::new();
    info.insert("Title", PdfString::from_str(title));
    if let Some(desc) = description {
        info.insert("Subject", PdfString::from_str(desc));
    }
    info.insert("Creator", PdfString::from_str("RavPDF Portfolio Studio"));
    let info_id = doc.next_object_id();
    doc.objects.insert(info_id, Object::Dictionary(info));
    doc.info_id = Some(info_id);

    // 3. Configure /Collection dictionary on Root (Catalog) conforming to ISO 32000-1 Clause 12.7.8
    let mut collection = PdfDictionary::new();
    collection.insert("Type", PdfName::new("Collection"));
    collection.insert("D", PdfName::new(layout.to_pdf_name()));

    // /Schema dictionary
    let mut schema = PdfDictionary::new();

    let mut col_fn = PdfDictionary::new();
    col_fn.insert("Type", PdfName::new("CollectionField"));
    col_fn.insert("Subtype", PdfName::new("F"));
    col_fn.insert("N", PdfString::from_str("File Name"));
    col_fn.insert("O", Object::Integer(1));
    col_fn.insert("V", Object::Boolean(true));
    schema.insert("FileName", Object::Dictionary(col_fn));

    let mut col_desc = PdfDictionary::new();
    col_desc.insert("Type", PdfName::new("CollectionField"));
    col_desc.insert("Subtype", PdfName::new("Desc"));
    col_desc.insert("N", PdfString::from_str("Description"));
    col_desc.insert("O", Object::Integer(2));
    col_desc.insert("V", Object::Boolean(true));
    schema.insert("Description", Object::Dictionary(col_desc));

    let mut col_size = PdfDictionary::new();
    col_size.insert("Type", PdfName::new("CollectionField"));
    col_size.insert("Subtype", PdfName::new("Size"));
    col_size.insert("N", PdfString::from_str("Size"));
    col_size.insert("O", Object::Integer(3));
    col_size.insert("V", Object::Boolean(true));
    schema.insert("Size", Object::Dictionary(col_size));

    collection.insert("Schema", Object::Dictionary(schema));

    // /Sort dictionary
    let mut sort_dict = PdfDictionary::new();
    sort_dict.insert("S", PdfName::new("FileName"));
    sort_dict.insert("A", Object::Boolean(true));
    collection.insert("Sort", Object::Dictionary(sort_dict));

    let catalog = doc
        .objects
        .get_mut(&doc.root_id)
        .and_then(Object::as_dict_mut)
        .ok_or_else(|| anyhow!("Invalid document catalog"))?;
    catalog.insert("Collection", Object::Dictionary(collection));

    // 4. Embed all items as attachments (FR-29.2.1)
    for item in items {
        add_attachment(
            &mut doc,
            &item.filename,
            item.description.as_deref(),
            &item.data,
        )?;
    }

    Ok(doc)
}

/// Adds a new file to an existing portfolio document (FR-29.2.5).
pub fn add_to_portfolio(doc: &mut PdfDocument, item: &PortfolioItem) -> Result<()> {
    // Ensure document has /Collection in catalog
    if !is_portfolio(doc) {
        let mut collection = PdfDictionary::new();
        collection.insert("Type", PdfName::new("Collection"));
        collection.insert("D", PdfName::new("D"));
        let catalog = doc
            .objects
            .get_mut(&doc.root_id)
            .and_then(Object::as_dict_mut)
            .ok_or_else(|| anyhow!("Invalid document catalog"))?;
        catalog.insert("Collection", Object::Dictionary(collection));
    }

    add_attachment(doc, &item.filename, item.description.as_deref(), &item.data)
}

/// Removes a file from an existing portfolio document (FR-29.2.5).
pub fn remove_from_portfolio(doc: &mut PdfDocument, filename: &str) -> Result<bool> {
    remove_attachment(doc, filename)
}

/// Extracts a specific file's bytes from a portfolio document (FR-29.2.4).
pub fn extract_portfolio_item(doc: &PdfDocument, filename: &str) -> Result<Option<Vec<u8>>> {
    let attachments = get_attachments(doc)?;
    for att in attachments {
        if att.filename == filename {
            return Ok(Some(att.data));
        }
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_portfolio_creation_and_lifecycle() {
        let item1 = PortfolioItem {
            filename: "quarterly_budget.csv".into(),
            description: Some("Q3 Fiscal Budget".into()),
            size: 38,
            data: b"Department,Cost\nEngineering,1200000\n".to_vec(),
            mime_type: Some("text/csv".into()),
        };

        let item2 = PortfolioItem {
            filename: "architecture_diagram.png".into(),
            description: Some("System Diagram".into()),
            size: 16,
            data: vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A],
            mime_type: Some("image/png".into()),
        };

        let items = vec![item1.clone(), item2.clone()];

        // Create portfolio
        let mut doc = create_portfolio(
            &items,
            PortfolioLayout::Tile,
            "Corporate Project Portfolio",
            Some("Q3 deliverables package"),
        )
        .expect("create portfolio");

        assert!(is_portfolio(&doc));

        // Read portfolio back
        let portfolio = get_portfolio(&doc).expect("get portfolio");
        assert_eq!(portfolio.title, "Corporate Project Portfolio");
        assert_eq!(portfolio.layout, PortfolioLayout::Tile);
        assert_eq!(portfolio.items.len(), 2);

        // Search portfolio
        let budget_matches = search_portfolio(&portfolio, "budget");
        assert_eq!(budget_matches.len(), 1);
        assert_eq!(budget_matches[0].filename, "quarterly_budget.csv");

        // Extract item
        let extracted = extract_portfolio_item(&doc, "quarterly_budget.csv")
            .expect("extract")
            .expect("found data");
        assert_eq!(extracted, item1.data);

        // Add 3rd item
        let item3 = PortfolioItem {
            filename: "summary.txt".into(),
            description: Some("Executive Summary".into()),
            size: 14,
            data: b"All goals met.".to_vec(),
            mime_type: Some("text/plain".into()),
        };
        add_to_portfolio(&mut doc, &item3).expect("add to portfolio");

        let updated = get_portfolio(&doc).expect("get updated");
        assert_eq!(updated.items.len(), 3);

        // Remove item
        let removed = remove_from_portfolio(&mut doc, "architecture_diagram.png").expect("remove item");
        assert!(removed);

        let final_port = get_portfolio(&doc).expect("get final");
        assert_eq!(final_port.items.len(), 2);
    }
}
