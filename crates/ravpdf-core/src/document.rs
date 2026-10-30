//! High-level PDF Document and Page representation conforming to ISO 32000-1 Section 7.7.
//!
//! Traceability: FR-01.1, FR-02.1, FR-02.3, FR-06, DR-01, DR-02.

use crate::error::{PdfError, Result};
use crate::object::{ObjectId, Object, PdfDictionary, PdfName, PdfStream};
use crate::parser::Parser;
use crate::writer::PdfWriter;
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

/// Standard ISO paper dimensions in points (1 point = 1/72 inch).
pub mod paper_sizes {
    pub const LETTER: (f64, f64) = (612.0, 792.0);
    pub const A4: (f64, f64) = (595.28, 841.89);
    pub const LEGAL: (f64, f64) = (612.0, 1008.0);
    pub const TABLOID: (f64, f64) = (792.0, 1224.0);
    pub const A3: (f64, f64) = (841.89, 1190.55);
}

/// A high-level view of a single PDF page.
#[derive(Debug, Clone)]
pub struct Page {
    pub id: ObjectId,
    pub media_box: [f64; 4],
    pub crop_box: [f64; 4],
    pub rotate: i64,
    pub contents_ids: Vec<ObjectId>,
    pub resources: Option<PdfDictionary>,
}

/// Represents an opened or newly created PDF document.
#[derive(Debug, Clone)]
pub struct PdfDocument {
    pub version: (u8, u8),
    pub root_id: ObjectId,
    pub info_id: Option<ObjectId>,
    pub objects: BTreeMap<ObjectId, Object>,
    pub trailer: PdfDictionary,
}

impl Default for PdfDocument {
    fn default() -> Self {
        Self::new()
    }
}

impl PdfDocument {
    /// Creates a brand new, valid PDF document conforming to ISO 32000-1 with an empty page tree.
    pub fn new() -> Self {
        let mut objects = BTreeMap::new();
        let root_id = ObjectId::new(1, 0);
        let pages_id = ObjectId::new(2, 0);

        // 1. Catalog dictionary (ISO 32000-1 Section 7.7.2)
        let mut catalog = PdfDictionary::new();
        catalog.insert("Type", PdfName::new("Catalog"));
        catalog.insert("Pages", pages_id);
        objects.insert(root_id, Object::Dictionary(catalog));

        // 2. Pages root node (ISO 32000-1 Section 7.7.3)
        let mut pages = PdfDictionary::new();
        pages.insert("Type", PdfName::new("Pages"));
        pages.insert("Count", 0);
        pages.insert("Kids", vec![]);
        objects.insert(pages_id, Object::Dictionary(pages));

        let mut trailer = PdfDictionary::new();
        trailer.insert("Size", 3);
        trailer.insert("Root", root_id);

        Self {
            version: (1, 7),
            root_id,
            info_id: None,
            objects,
            trailer,
        }
    }

    /// Loads and parses a PDF document from a byte buffer.
    pub fn load_from_bytes(data: &[u8]) -> Result<Self> {
        // Validate header
        if !data.starts_with(b"%PDF-") {
            return Err(PdfError::InvalidHeader(
                "Missing %PDF- header magic bytes".into(),
            ));
        }

        let header_str = String::from_utf8_lossy(&data[..8.min(data.len())]);
        let mut version = (1, 7);
        if let Some(pos) = header_str.find("%PDF-") {
            let ver_part = &header_str[pos + 5..];
            if ver_part.len() >= 3 {
                let major = ver_part.chars().next().and_then(|c| c.to_digit(10)).unwrap_or(1) as u8;
                let minor = ver_part.chars().nth(2).and_then(|c| c.to_digit(10)).unwrap_or(7) as u8;
                version = (major, minor);
            }
        }

        let parser = Parser::new(data);
        let startxref_offset = parser.find_startxref()?;
        let (xref_map, trailer) = parser.parse_xref_and_trailer(startxref_offset)?;

        let mut objects = BTreeMap::new();

        for (_id, offset) in xref_map {
            if offset < data.len() {
                if let Ok((parsed_id, obj)) = parser.parse_indirect_object_at(offset) {
                    objects.insert(parsed_id, obj);
                }
            }
        }

        let root_id = trailer
            .get_ref("Root")
            .ok_or_else(|| PdfError::InvalidStructure("Missing /Root in PDF trailer".into()))?;

        let info_id = trailer.get_ref("Info");

        Ok(Self {
            version,
            root_id,
            info_id,
            objects,
            trailer,
        })
    }

    /// Loads and parses a PDF document from a file path on disk.
    pub fn load_from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let bytes = fs::read(path)?;
        Self::load_from_bytes(&bytes)
    }

    /// Generates the next available object ID for new objects.
    pub fn next_object_id(&self) -> ObjectId {
        let max_num = self.objects.keys().map(|k| k.number).max().unwrap_or(0);
        ObjectId::new(max_num + 1, 0)
    }

    /// Resolves an indirect object by its ObjectId.
    pub fn get_object(&self, id: ObjectId) -> Option<&Object> {
        self.objects.get(&id)
    }

    /// Resolves an indirect object mutably by its ObjectId.
    pub fn get_object_mut(&mut self, id: ObjectId) -> Option<&mut Object> {
        self.objects.get_mut(&id)
    }

    /// Resolves an object: if it is a reference, look it up in objects; otherwise return itself.
    pub fn resolve<'a>(&'a self, obj: &'a Object) -> Option<&'a Object> {
        match obj {
            Object::Reference(id) => self.objects.get(id),
            other => Some(other),
        }
    }

    /// Resolves the root Catalog dictionary.
    pub fn catalog(&self) -> Option<&PdfDictionary> {
        self.get_object(self.root_id).and_then(Object::as_dict)
    }

    /// Resolves mutable reference to the root Catalog dictionary.
    pub fn catalog_mut(&mut self) -> Option<&mut PdfDictionary> {
        let root_id = self.root_id;
        self.objects.get_mut(&root_id).and_then(Object::as_dict_mut)
    }

    /// Resolves the root `/Pages` dictionary ID.
    pub fn pages_root_id(&self) -> Option<ObjectId> {
        self.catalog()?.get_ref("Pages")
    }

    /// Retrieves an ordered list of all Page object IDs by traversing the Pages tree hierarchy.
    pub fn page_ids(&self) -> Vec<ObjectId> {
        let mut ids = Vec::new();
        if let Some(pages_id) = self.pages_root_id() {
            self.collect_page_ids(pages_id, &mut ids);
        }
        ids
    }

    fn collect_page_ids(&self, node_id: ObjectId, out: &mut Vec<ObjectId>) {
        if let Some(obj) = self.get_object(node_id) {
            if let Some(dict) = obj.as_dict() {
                let node_type = dict.get_name("Type").unwrap_or("");
                if node_type == "Page" {
                    out.push(node_id);
                } else if node_type == "Pages" {
                    if let Some(kids) = dict.get_array("Kids") {
                        for kid in kids {
                            if let Some(kid_id) = kid.as_reference() {
                                self.collect_page_ids(kid_id, out);
                            }
                        }
                    }
                }
            }
        }
    }

    /// Returns the total number of pages in the document.
    pub fn page_count(&self) -> usize {
        self.page_ids().len()
    }

    /// Retrieves structured information for the page at 0-based `index`.
    pub fn get_page(&self, index: usize) -> Option<Page> {
        let page_ids = self.page_ids();
        let page_id = *page_ids.get(index)?;
        let page_dict = self.get_object(page_id)?.as_dict()?;

        let media_box = self
            .resolve_box(page_dict, "MediaBox")
            .unwrap_or([0.0, 0.0, paper_sizes::LETTER.0, paper_sizes::LETTER.1]);

        let crop_box = self
            .resolve_box(page_dict, "CropBox")
            .unwrap_or(media_box);

        let rotate = page_dict.get_i64("Rotate").unwrap_or(0);

        // Collect content stream IDs
        let mut contents_ids = Vec::new();
        if let Some(contents_obj) = page_dict.get("Contents") {
            match contents_obj {
                Object::Reference(id) => contents_ids.push(*id),
                Object::Array(arr) => {
                    for item in arr {
                        if let Some(id) = item.as_reference() {
                            contents_ids.push(id);
                        }
                    }
                }
                _ => {}
            }
        }

        // Resolve /Resources (inherited if not present on page)
        let resources = self.resolve_resources(page_dict);

        Some(Page {
            id: page_id,
            media_box,
            crop_box,
            rotate,
            contents_ids,
            resources,
        })
    }

    fn resolve_box(&self, page_dict: &PdfDictionary, box_name: &str) -> Option<[f64; 4]> {
        if let Some(arr) = page_dict.get_array(box_name) {
            if arr.len() >= 4 {
                let x1 = arr[0].as_f64()?;
                let y1 = arr[1].as_f64()?;
                let x2 = arr[2].as_f64()?;
                let y2 = arr[3].as_f64()?;
                return Some([x1, y1, x2, y2]);
            }
        }
        // Fallback to parent /Pages node
        if let Some(parent_id) = page_dict.get_ref("Parent") {
            if let Some(parent_dict) = self.get_object(parent_id).and_then(Object::as_dict) {
                return self.resolve_box(parent_dict, box_name);
            }
        }
        None
    }

    fn resolve_resources(&self, page_dict: &PdfDictionary) -> Option<PdfDictionary> {
        if let Some(res) = page_dict.get_dict("Resources") {
            return Some(res.clone());
        }
        if let Some(res_ref) = page_dict.get_ref("Resources") {
            if let Some(res_dict) = self.get_object(res_ref).and_then(Object::as_dict) {
                return Some(res_dict.clone());
            }
        }
        // Inherit from parent
        if let Some(parent_id) = page_dict.get_ref("Parent") {
            if let Some(parent_dict) = self.get_object(parent_id).and_then(Object::as_dict) {
                return self.resolve_resources(parent_dict);
            }
        }
        None
    }

    /// Extracts and decodes the complete concatenated content stream for a page.
    pub fn get_page_content(&self, index: usize) -> Result<Vec<u8>> {
        let page = self
            .get_page(index)
            .ok_or_else(|| PdfError::InvalidStructure(format!("Page index {} out of bounds", index)))?;

        let mut full_content = Vec::new();

        for content_id in page.contents_ids {
            if let Some(obj) = self.get_object(content_id) {
                if let Some(stream) = obj.as_stream() {
                    let decoded = stream.decode()?;
                    if !full_content.is_empty() {
                        full_content.push(b'\n');
                    }
                    full_content.extend_from_slice(&decoded);
                }
            }
        }

        Ok(full_content)
    }

    /// Adds a new blank page to the document with specified width and height.
    pub fn add_blank_page(&mut self, width: f64, height: f64) -> ObjectId {
        let page_id = self.next_object_id();
        let pages_id = self.pages_root_id().unwrap_or(ObjectId::new(2, 0));

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

        self.objects.insert(page_id, Object::Dictionary(page_dict));

        // Update Pages tree
        self.append_page_to_root(page_id);

        page_id
    }

    /// Adds a page with given dimensions and raw PDF content stream instructions.
    pub fn add_page_with_content(
        &mut self,
        width: f64,
        height: f64,
        content_stream: Vec<u8>,
    ) -> ObjectId {
        let content_id = self.next_object_id();
        let mut stream = PdfStream::with_plain_data(PdfDictionary::new(), content_stream);
        let _ = stream.compress_flate();
        self.objects.insert(content_id, Object::Stream(stream));

        let page_id = self.next_object_id();
        let pages_id = self.pages_root_id().unwrap_or(ObjectId::new(2, 0));

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
        page_dict.insert("Contents", content_id);
        page_dict.insert("Resources", PdfDictionary::new());

        self.objects.insert(page_id, Object::Dictionary(page_dict));
        self.append_page_to_root(page_id);

        page_id
    }

    fn append_page_to_root(&mut self, page_id: ObjectId) {
        if let Some(pages_id) = self.pages_root_id() {
            if let Some(pages_obj) = self.objects.get_mut(&pages_id) {
                if let Some(dict) = pages_obj.as_dict_mut() {
                    let mut count = dict.get_i64("Count").unwrap_or(0);
                    count += 1;
                    dict.insert("Count", count);

                    let kids = dict.map.entry(PdfName::new("Kids")).or_insert_with(|| {
                        Object::Array(Vec::new())
                    });

                    if let Object::Array(arr) = kids {
                        arr.push(Object::Reference(page_id));
                    }
                }
            }
        }
    }

    /// Serializes this document into standard PDF bytes.
    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        PdfWriter::write_document(&self.objects, self.root_id, self.info_id)
    }

    /// Saves this document to a file path.
    pub fn save_to_file<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let bytes = self.to_bytes()?;
        fs::write(path, bytes)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_and_load_blank_document() {
        let mut doc = PdfDocument::new();
        assert_eq!(doc.page_count(), 0);

        doc.add_blank_page(paper_sizes::A4.0, paper_sizes::A4.1);
        doc.add_blank_page(paper_sizes::LETTER.0, paper_sizes::LETTER.1);

        assert_eq!(doc.page_count(), 2);

        let bytes = doc.to_bytes().expect("serialize doc");
        let reloaded = PdfDocument::load_from_bytes(&bytes).expect("reload doc");

        assert_eq!(reloaded.page_count(), 2);
        let p1 = reloaded.get_page(0).expect("page 0");
        assert_eq!(p1.media_box, [0.0, 0.0, paper_sizes::A4.0, paper_sizes::A4.1]);
    }
}
