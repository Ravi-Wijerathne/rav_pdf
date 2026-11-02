//! High-level PDF Document and Page representation conforming to ISO 32000-1 Section 7.7.
//!
//! Traceability: FR-01.1, FR-02.1, FR-02.3, FR-06, DR-01, DR-02.

use crate::error::{PdfError, Result};
use crate::object::{ObjectId, Object, PdfDictionary, PdfName, PdfStream};
use crate::parser::{Parser, XRefEntry};
use crate::writer::PdfWriter;
use std::collections::{BTreeMap, BTreeSet};
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

/// A high-level view of a single PDF page conforming to ISO 32000-1 Clause 7.7.3.3 and 14.11.2.
#[derive(Debug, Clone)]
pub struct Page {
    pub id: ObjectId,
    pub media_box: [f64; 4],
    pub crop_box: [f64; 4],
    pub bleed_box: Option<[f64; 4]>,
    pub trim_box: Option<[f64; 4]>,
    pub art_box: Option<[f64; 4]>,
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
    /// Verbatim byte buffer of the opened PDF (for incremental saves conforming to DR-02.5 / NFR-03.5)
    pub original_bytes: Option<Vec<u8>>,
    /// Byte offset of the previous startxref in `original_bytes`
    pub prev_startxref: Option<usize>,
    /// Set of ObjectIds modified or created since the document was loaded
    pub dirty_objects: BTreeSet<ObjectId>,
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
            original_bytes: None,
            prev_startxref: None,
            dirty_objects: BTreeSet::new(),
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
        let mut obj_stm_ids = BTreeSet::new();

        // Phase 1: Load top-level uncompressed objects and identify object streams
        for (&_id, entry) in &xref_map {
            match entry {
                XRefEntry::Uncompressed { offset, .. } => {
                    if *offset < data.len() {
                        if let Ok((parsed_id, obj)) = parser.parse_indirect_object_at(*offset) {
                            objects.insert(parsed_id, obj);
                        }
                    }
                }
                XRefEntry::Compressed { container_obj_num, .. } => {
                    obj_stm_ids.insert(*container_obj_num);
                }
            }
        }

        // Phase 2: Decode all /ObjStm containers and insert compressed objects into objects
        for container_id in obj_stm_ids {
            if let Some(Object::Stream(stream)) = objects.get(&ObjectId::new(container_id, 0)) {
                let is_obj_stm = stream
                    .dict
                    .get_name("Type")
                    .map(|t| t == "ObjStm")
                    .unwrap_or(false);
                if is_obj_stm {
                    if let Ok(compressed_objs) = Parser::parse_object_stream(stream) {
                        for (nested_id, nested_obj) in compressed_objs {
                            // Precedence check: only insert if xref_map indicates this object is in this container
                            if let Some(XRefEntry::Compressed { container_obj_num, .. }) = xref_map.get(&nested_id) {
                                if *container_obj_num == container_id {
                                    objects.insert(nested_id, nested_obj);
                                }
                            }
                        }
                    }
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
            original_bytes: Some(data.to_vec()),
            prev_startxref: Some(startxref_offset),
            dirty_objects: BTreeSet::new(),
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
        let trailer_size = self.trailer.get_i64("Size").unwrap_or(0) as u32;
        let base_num = max_num.max(trailer_size.saturating_sub(1));
        ObjectId::new(base_num + 1, 0)
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

        // Resolve page boundary boxes per ISO 32000-1 Clause 14.11.2
        let bleed_box = self.resolve_box(page_dict, "BleedBox");
        let trim_box = self.resolve_box(page_dict, "TrimBox");
        let art_box = self.resolve_box(page_dict, "ArtBox");

        // Resolve /Resources (inherited if not present on page)
        let resources = self.resolve_resources(page_dict);

        Some(Page {
            id: page_id,
            media_box,
            crop_box,
            bleed_box,
            trim_box,
            art_box,
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
        self.mark_dirty(page_id);

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
        self.mark_dirty(content_id);

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
        self.mark_dirty(page_id);
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
            self.mark_dirty(pages_id);
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

    /// Marks an object ID as modified for incremental save.
    pub fn mark_dirty(&mut self, id: ObjectId) {
        self.dirty_objects.insert(id);
    }

    /// Sets or updates an indirect object and marks it as dirty.
    pub fn set_object(&mut self, id: ObjectId, obj: Object) {
        self.objects.insert(id, obj);
        self.mark_dirty(id);
    }

    /// Saves pending dirty objects as an incremental update appended to `original_bytes`.
    ///
    /// Conforms to ISO 32000-1 Section 7.5.6, preserving existing bytes verbatim.
    /// Updates `original_bytes` and `prev_startxref` to reflect the new revision state.
    pub fn save_incremental(&mut self) -> Result<Vec<u8>> {
        let orig = self.original_bytes.as_ref().ok_or_else(|| {
            PdfError::InvalidStructure(
                "Cannot perform incremental save on document without original bytes".into(),
            )
        })?;
        let prev_startxref = self.prev_startxref.unwrap_or(0);

        let modified: Vec<(ObjectId, Object)> = self
            .dirty_objects
            .iter()
            .filter_map(|id| self.objects.get(id).map(|obj| (*id, obj.clone())))
            .collect();

        let updated_bytes =
            PdfWriter::write_incremental_update(orig, &modified, prev_startxref, &self.trailer)?;

        // Update document state for subsequent incremental saves
        let parser = Parser::new(&updated_bytes);
        let new_startxref = parser.find_startxref()?;
        let (_, new_trailer) = parser.parse_xref_and_trailer(new_startxref)?;
        self.prev_startxref = Some(new_startxref);
        self.trailer = new_trailer;
        self.original_bytes = Some(updated_bytes.clone());
        self.dirty_objects.clear();

        Ok(updated_bytes)
    }

    /// Explicit incremental update passing an external slice of modified objects.
    pub fn write_incremental_update(
        &self,
        modified_objects: &[(ObjectId, Object)],
    ) -> Result<Vec<u8>> {
        let orig = self.original_bytes.as_ref().ok_or_else(|| {
            PdfError::InvalidStructure(
                "Cannot perform incremental update on document without original bytes".into(),
            )
        })?;
        let prev_startxref = self.prev_startxref.unwrap_or(0);
        PdfWriter::write_incremental_update(orig, modified_objects, prev_startxref, &self.trailer)
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

    #[test]
    fn test_multiple_successive_incremental_saves() {
        let mut doc = PdfDocument::new();
        doc.add_blank_page(paper_sizes::A4.0, paper_sizes::A4.1);
        let base_bytes = doc.to_bytes().expect("to bytes");

        // Revision 1: add a new blank page
        let mut doc_rev1 = PdfDocument::load_from_bytes(&base_bytes).expect("load rev1");
        let new_page_id = doc_rev1.add_blank_page(paper_sizes::LETTER.0, paper_sizes::LETTER.1);
        doc_rev1.mark_dirty(new_page_id);
        doc_rev1.mark_dirty(doc_rev1.pages_root_id().unwrap());

        let rev1_bytes = doc_rev1.save_incremental().expect("save rev1");
        // Verify exact byte preservation of revision 0
        assert_eq!(&rev1_bytes[..base_bytes.len()], &base_bytes[..]);

        // Revision 2: add a custom object
        let mut doc_rev2 = PdfDocument::load_from_bytes(&rev1_bytes).expect("load rev2");
        assert_eq!(doc_rev2.page_count(), 2);

        let annot_id = doc_rev2.next_object_id();
        let mut annot_dict = PdfDictionary::new();
        annot_dict.insert("Type", PdfName::new("Annot"));
        annot_dict.insert("Subtype", PdfName::new("Text"));
        doc_rev2.set_object(annot_id, Object::Dictionary(annot_dict));

        let rev2_bytes = doc_rev2.save_incremental().expect("save rev2");
        // Verify exact byte preservation of revision 1
        assert_eq!(&rev2_bytes[..rev1_bytes.len()], &rev1_bytes[..]);

        let doc_final = PdfDocument::load_from_bytes(&rev2_bytes).expect("load final");
        assert_eq!(doc_final.page_count(), 2);
        assert!(doc_final.get_object(annot_id).is_some());
    }

    #[test]
    fn test_incremental_save_without_original_bytes_fails() {
        let mut doc = PdfDocument::new();
        let err = doc.save_incremental();
        assert!(err.is_err());
        assert!(matches!(err.unwrap_err(), PdfError::InvalidStructure(_)));
    }

    #[test]
    fn test_end_to_end_objstm_document_loading() {
        // Construct a synthetic PDF 1.5 document with an object stream and xref stream
        // Obj 1 (Catalog) & Obj 2 (Pages) are inside ObjStm 3
        // Obj 4 is a Page object (uncompressed)
        // Obj 5 is the XRef stream containing Type 2 entries for 1 and 2

        let mut buf = Vec::new();
        buf.extend_from_slice(b"%PDF-1.5\n%\xE2\xE3\xCF\xD3\n");

        // Object 4: Page
        let obj4_offset = buf.len();
        buf.extend_from_slice(b"4 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Resources <<>> >>\nendobj\n");

        // Object 3: ObjStm containing Catalog (1) and Pages (2)
        // Obj 1: << /Type /Catalog /Pages 2 0 R >>
        // Obj 2: << /Type /Pages /Count 1 /Kids [4 0 R] >>
        let objstm_payload = b"1 0 2 34\n<< /Type /Catalog /Pages 2 0 R >>\n<< /Type /Pages /Count 1 /Kids [4 0 R] >>\n";
        let obj3_offset = buf.len();
        let obj3_header = format!("3 0 obj\n<< /Type /ObjStm /N 2 /First 9 /Length {} >>\nstream\n", objstm_payload.len());
        buf.extend_from_slice(obj3_header.as_bytes());
        buf.extend_from_slice(objstm_payload);
        buf.extend_from_slice(b"\nendstream\nendobj\n");

        // Object 5: XRef stream
        // W = [1, 2, 1] -> entry size = 4
        // Entry 1 (Obj 1): Type 2, container 3, index 0 -> [2, 0, 3, 0]
        // Entry 2 (Obj 2): Type 2, container 3, index 1 -> [2, 0, 3, 1]
        // Entry 3 (Obj 3): Type 1, offset obj3_offset, gen 0 -> [1, (offset>>8), (offset&0xFF), 0]
        // Entry 4 (Obj 4): Type 1, offset obj4_offset, gen 0
        // Entry 5 (Obj 5): Type 1, offset obj5_offset, gen 0
        let obj5_offset = buf.len();
        let o3_hi = ((obj3_offset >> 8) & 0xFF) as u8;
        let o3_lo = (obj3_offset & 0xFF) as u8;
        let o4_hi = ((obj4_offset >> 8) & 0xFF) as u8;
        let o4_lo = (obj4_offset & 0xFF) as u8;
        let o5_hi = ((obj5_offset >> 8) & 0xFF) as u8;
        let o5_lo = (obj5_offset & 0xFF) as u8;

        let xref_data = vec![
            2, 0, 3, 0,         // Obj 1: in ObjStm 3 at index 0
            2, 0, 3, 1,         // Obj 2: in ObjStm 3 at index 1
            1, o3_hi, o3_lo, 0, // Obj 3: uncompressed
            1, o4_hi, o4_lo, 0, // Obj 4: uncompressed
            1, o5_hi, o5_lo, 0, // Obj 5: uncompressed
        ];

        let obj5_header = format!(
            "5 0 obj\n<< /Type /XRef /Size 6 /Index [1 5] /W [1 2 1] /Root 1 0 R /Length {} >>\nstream\n",
            xref_data.len()
        );
        buf.extend_from_slice(obj5_header.as_bytes());
        buf.extend_from_slice(&xref_data);
        buf.extend_from_slice(b"\nendstream\nendobj\n");

        // startxref and %%EOF
        let footer = format!("startxref\n{}\n%%EOF\n", obj5_offset);
        buf.extend_from_slice(footer.as_bytes());

        let doc = PdfDocument::load_from_bytes(&buf).expect("load doc with objstm and type 2 xref");
        assert_eq!(doc.page_count(), 1);
        let p = doc.get_page(0).expect("page 0");
        assert_eq!(p.media_box, [0.0, 0.0, 612.0, 792.0]);
    }
}
