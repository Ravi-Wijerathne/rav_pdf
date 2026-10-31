//! PDF Serializer and Writer conforming to ISO 32000-1 Section 7.5.
//!
//! Traceability: FR-02.1, FR-02.3, DR-02.5, DR-02.6, NFR-03.4.

use crate::error::Result;
use crate::object::{ObjectId, Object, PdfDictionary, PdfStream, PdfString};
use std::collections::BTreeMap;
use std::io::Write;

pub struct PdfWriter {
    buffer: Vec<u8>,
}

impl Default for PdfWriter {
    fn default() -> Self {
        Self::new()
    }
}

impl PdfWriter {
    pub fn new() -> Self {
        Self { buffer: Vec::new() }
    }

    pub fn buffer(&self) -> &[u8] {
        &self.buffer
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.buffer
    }

    /// Serializes an entire PDF document from its indirect objects, root catalog ID, and trailer.
    pub fn write_document(
        objects: &BTreeMap<ObjectId, Object>,
        root_id: ObjectId,
        info_id: Option<ObjectId>,
    ) -> Result<Vec<u8>> {
        let mut writer = PdfWriter::new();

        // 1. Header (ISO 32000-1 Section 7.5.2)
        writer.buffer.extend_from_slice(b"%PDF-1.7\n");
        // 4 binary characters > 127 to ensure binary file handling
        writer.buffer.extend_from_slice(b"%\xE2\xE3\xCF\xD3\n");

        let mut xref_offsets: BTreeMap<u32, (usize, u16)> = BTreeMap::new();

        // 2. Body - serialize indirect objects sorted by object number
        for (id, obj) in objects {
            let offset = writer.buffer.len();
            xref_offsets.insert(id.number, (offset, id.generation));

            let header = format!("{} {} obj\n", id.number, id.generation);
            writer.buffer.extend_from_slice(header.as_bytes());

            Self::write_object(&mut writer.buffer, obj)?;

            writer.buffer.extend_from_slice(b"\nendobj\n");
        }

        // 3. Cross-Reference Table (ISO 32000-1 Section 7.5.4)
        let xref_start_offset = writer.buffer.len();
        let max_id = xref_offsets.keys().copied().max().unwrap_or(0);
        let total_size = max_id + 1;

        writer.buffer.extend_from_slice(b"xref\n");
        let section_header = format!("0 {}\n", total_size);
        writer.buffer.extend_from_slice(section_header.as_bytes());

        // Entry 0 is always free with generation 65535
        writer
            .buffer
            .extend_from_slice(b"0000000000 65535 f \n");

        for num in 1..=max_id {
            if let Some(&(offset, gen)) = xref_offsets.get(&num) {
                let entry = format!("{:010} {:05} n \n", offset, gen);
                writer.buffer.extend_from_slice(entry.as_bytes());
            } else {
                writer
                    .buffer
                    .extend_from_slice(b"0000000000 65535 f \n");
            }
        }

        // 4. Trailer Dictionary (ISO 32000-1 Section 7.5.5)
        let mut trailer_dict = PdfDictionary::new();
        trailer_dict.insert("Size", total_size as i64);
        trailer_dict.insert("Root", root_id);
        if let Some(info) = info_id {
            trailer_dict.insert("Info", info);
        }

        // Generate consistent /ID array
        let doc_id = PdfString::from_hex(vec![0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC, 0xDE, 0xF0]);
        trailer_dict.insert(
            "ID",
            vec![
                Object::String(doc_id.clone()),
                Object::String(doc_id),
            ],
        );

        writer.buffer.extend_from_slice(b"trailer\n");
        Self::write_dictionary(&mut writer.buffer, &trailer_dict)?;

        // 5. startxref and EOF
        let footer = format!("\nstartxref\n{}\n%%EOF\n", xref_start_offset);
        writer.buffer.extend_from_slice(footer.as_bytes());

        Ok(writer.into_bytes())
    }

    /// Appends an incremental update to `original_bytes` conforming to ISO 32000-1 Section 7.5.6.
    ///
    /// Preserves all original bytes without altering a single byte.
    /// Serializes modified/new indirect objects, outputs subsection-clustered xref tables,
    /// chains the trailer dictionary via `/Prev <prev_startxref>`, and retains `/Root`, `/Info`,
    /// `/Encrypt`, and persistent document `/ID[0]`.
    pub fn write_incremental_update(
        original_bytes: &[u8],
        modified_objects: &[(ObjectId, Object)],
        prev_startxref: usize,
        trailer: &PdfDictionary,
    ) -> Result<Vec<u8>> {
        if original_bytes.is_empty() {
            return Err(crate::error::PdfError::InvalidStructure(
                "Original bytes cannot be empty for incremental update".into(),
            ));
        }

        let parser = crate::parser::Parser::new(original_bytes);
        let effective_prev_startxref = if prev_startxref == 0 {
            parser.find_startxref()?
        } else {
            prev_startxref
        };

        // Extract previous trailer to carry over keys
        let (_, prev_trailer) = parser.parse_xref_and_trailer(effective_prev_startxref)?;

        let mut buffer = original_bytes.to_vec();

        // Ensure newline separation
        if !buffer.ends_with(b"\n") && !buffer.ends_with(b"\r") {
            buffer.push(b'\n');
        }

        // Serialize modified indirect objects
        let mut xref_offsets: BTreeMap<u32, (usize, u16)> = BTreeMap::new();
        for (id, obj) in modified_objects {
            let offset = buffer.len();
            xref_offsets.insert(id.number, (offset, id.generation));

            let header = format!("{} {} obj\n", id.number, id.generation);
            buffer.extend_from_slice(header.as_bytes());

            Self::write_object(&mut buffer, obj)?;
            buffer.extend_from_slice(b"\nendobj\n");
        }

        // Write incremental xref table with contiguous subsections
        let xref_start_offset = buffer.len();
        buffer.extend_from_slice(b"xref\n");

        if !xref_offsets.is_empty() {
            let keys: Vec<u32> = xref_offsets.keys().copied().collect();
            let mut i = 0;
            while i < keys.len() {
                let start_num = keys[i];
                let mut run_len = 1;
                while i + run_len < keys.len() && keys[i + run_len] == start_num + run_len as u32 {
                    run_len += 1;
                }

                let sub_header = format!("{} {}\n", start_num, run_len);
                buffer.extend_from_slice(sub_header.as_bytes());

                for num in start_num..(start_num + run_len as u32) {
                    let &(offset, gen) = &xref_offsets[&num];
                    let entry = format!("{:010} {:05} n \n", offset, gen);
                    buffer.extend_from_slice(entry.as_bytes());
                }

                i += run_len;
            }
        }

        // Synthesize incremental trailer
        let prev_size = prev_trailer.get_i64("Size").unwrap_or(0);
        let max_mod_id = xref_offsets.keys().copied().max().unwrap_or(0);
        let explicit_size = trailer.get_i64("Size").unwrap_or(0);
        let new_size = prev_size.max(explicit_size).max((max_mod_id + 1) as i64);

        let mut trailer_dict = PdfDictionary::new();
        trailer_dict.insert("Size", new_size);
        trailer_dict.insert("Prev", effective_prev_startxref as i64);

        // Carry over /Root or check modified_objects / trailer
        if let Some(root_obj) = trailer.get("Root") {
            trailer_dict.insert("Root", root_obj.clone());
        } else if let Some(cat_id) = modified_objects.iter().find_map(|(id, obj)| {
            if let Object::Dictionary(d) = obj {
                if d.get_name("Type") == Some("Catalog") {
                    return Some(*id);
                }
            }
            None
        }) {
            trailer_dict.insert("Root", cat_id);
        } else if let Some(root_obj) = prev_trailer.get("Root") {
            trailer_dict.insert("Root", root_obj.clone());
        }

        // Carry over /Info
        if let Some(info_obj) = trailer.get("Info") {
            trailer_dict.insert("Info", info_obj.clone());
        } else if let Some(info_obj) = prev_trailer.get("Info") {
            trailer_dict.insert("Info", info_obj.clone());
        }

        // Carry over /Encrypt
        if let Some(encrypt_obj) = trailer.get("Encrypt") {
            trailer_dict.insert("Encrypt", encrypt_obj.clone());
        } else if let Some(encrypt_obj) = prev_trailer.get("Encrypt") {
            trailer_dict.insert("Encrypt", encrypt_obj.clone());
        }

        // Retain permanent ID[0], generate new random ID[1]
        let mut id1 = [0u8; 16];
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default();
        let seed_a = (now.as_nanos() ^ (buffer.len() as u128)) as u64;
        let seed_b = (now.as_secs() << 32) | (max_mod_id as u64);
        id1[..8].copy_from_slice(&seed_a.to_le_bytes());
        id1[8..].copy_from_slice(&seed_b.to_le_bytes());
        let new_instance_id = PdfString::from_hex(id1.to_vec());

        let permanent_id = if let Some(id_arr) = trailer.get_array("ID").or_else(|| prev_trailer.get_array("ID")) {
            id_arr.first().cloned().unwrap_or_else(|| {
                Object::String(PdfString::from_hex(vec![0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC, 0xDE, 0xF0]))
            })
        } else {
            Object::String(PdfString::from_hex(vec![0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC, 0xDE, 0xF0]))
        };

        trailer_dict.insert(
            "ID",
            vec![permanent_id, Object::String(new_instance_id)],
        );

        // Copy any additional keys from trailer or prev_trailer
        for (k, v) in trailer.map.iter() {
            if !trailer_dict.map.contains_key(k) {
                trailer_dict.map.insert(k.clone(), v.clone());
            }
        }

        buffer.extend_from_slice(b"trailer\n");
        Self::write_dictionary(&mut buffer, &trailer_dict)?;

        let footer = format!("\nstartxref\n{}\n%%EOF\n", xref_start_offset);
        buffer.extend_from_slice(footer.as_bytes());

        Ok(buffer)
    }

    /// Serializes an individual Object into target buffer.
    pub fn write_object(buf: &mut Vec<u8>, obj: &Object) -> Result<()> {
        match obj {
            Object::Null => {
                buf.extend_from_slice(b"null");
            }
            Object::Boolean(b) => {
                if *b {
                    buf.extend_from_slice(b"true");
                } else {
                    buf.extend_from_slice(b"false");
                }
            }
            Object::Integer(i) => {
                write!(buf, "{}", i)?;
            }
            Object::Real(r) => {
                if r.fract() == 0.0 {
                    write!(buf, "{:.1}", r)?;
                } else {
                    write!(buf, "{:.4}", r)?;
                }
            }
            Object::Name(name) => {
                buf.push(b'/');
                Self::write_escaped_name(buf, name.as_str());
            }
            Object::String(s) => {
                if s.is_hex {
                    buf.push(b'<');
                    for &byte in &s.bytes {
                        write!(buf, "{:02X}", byte)?;
                    }
                    buf.push(b'>');
                } else {
                    buf.push(b'(');
                    for &b in &s.bytes {
                        match b {
                            b'(' => buf.extend_from_slice(b"\\("),
                            b')' => buf.extend_from_slice(b"\\)"),
                            b'\\' => buf.extend_from_slice(b"\\\\"),
                            b'\r' => buf.extend_from_slice(b"\\r"),
                            b'\n' => buf.extend_from_slice(b"\\n"),
                            b'\t' => buf.extend_from_slice(b"\\t"),
                            _ => buf.push(b),
                        }
                    }
                    buf.push(b')');
                }
            }
            Object::Array(arr) => {
                buf.push(b'[');
                for (i, item) in arr.iter().enumerate() {
                    if i > 0 {
                        buf.push(b' ');
                    }
                    Self::write_object(buf, item)?;
                }
                buf.push(b']');
            }
            Object::Dictionary(dict) => {
                Self::write_dictionary(buf, dict)?;
            }
            Object::Stream(stream) => {
                Self::write_stream(buf, stream)?;
            }
            Object::Reference(id) => {
                write!(buf, "{} {} R", id.number, id.generation)?;
            }
        }
        Ok(())
    }

    pub fn write_dictionary(buf: &mut Vec<u8>, dict: &PdfDictionary) -> Result<()> {
        buf.extend_from_slice(b"<<\n");
        for (key, val) in &dict.map {
            buf.extend_from_slice(b"  /");
            Self::write_escaped_name(buf, key.as_str());
            buf.push(b' ');
            Self::write_object(buf, val)?;
            buf.push(b'\n');
        }
        buf.extend_from_slice(b">>");
        Ok(())
    }

    pub fn write_stream(buf: &mut Vec<u8>, stream: &PdfStream) -> Result<()> {
        // Ensure /Length is updated
        let mut dict = stream.dict.clone();
        dict.insert("Length", stream.raw_data.len() as i64);

        Self::write_dictionary(buf, &dict)?;
        buf.extend_from_slice(b"\nstream\n");
        buf.extend_from_slice(&stream.raw_data);
        buf.extend_from_slice(b"\nendstream");
        Ok(())
    }

    fn write_escaped_name(buf: &mut Vec<u8>, name: &str) {
        for &b in name.as_bytes() {
            if b.is_ascii_graphic() && !crate::lexer::is_pdf_delimiter(b) && b != b'#' {
                buf.push(b);
            } else {
                write!(buf, "#{:02X}", b).unwrap();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::object::PdfName;

    #[test]
    fn test_write_and_parse_roundtrip() {
        let mut objects = BTreeMap::new();
        let root_id = ObjectId::new(1, 0);
        let pages_id = ObjectId::new(2, 0);

        let mut catalog = PdfDictionary::new();
        catalog.insert("Type", PdfName::new("Catalog"));
        catalog.insert("Pages", pages_id);
        objects.insert(root_id, Object::Dictionary(catalog));

        let mut pages = PdfDictionary::new();
        pages.insert("Type", PdfName::new("Pages"));
        pages.insert("Count", 0);
        pages.insert("Kids", vec![]);
        objects.insert(pages_id, Object::Dictionary(pages));

        let pdf_bytes = PdfWriter::write_document(&objects, root_id, None).expect("write doc");
        assert!(pdf_bytes.starts_with(b"%PDF-1.7\n"));
        assert!(pdf_bytes.ends_with(b"%%EOF\n"));

        // Validate that parser reads it back
        let parser = crate::parser::Parser::new(&pdf_bytes);
        let startxref = parser.find_startxref().expect("find startxref");
        let (xref_map, trailer) = parser.parse_xref_and_trailer(startxref).expect("parse xref");

        assert_eq!(trailer.get_ref("Root"), Some(root_id));
        assert!(xref_map.contains_key(&root_id));
        assert!(xref_map.contains_key(&pages_id));
    }

    #[test]
    fn test_incremental_save_preserves_exact_bytes_and_parses() {
        // 1. Create base document
        let mut objects = BTreeMap::new();
        let root_id = ObjectId::new(1, 0);
        let pages_id = ObjectId::new(2, 0);

        let mut catalog = PdfDictionary::new();
        catalog.insert("Type", PdfName::new("Catalog"));
        catalog.insert("Pages", pages_id);
        objects.insert(root_id, Object::Dictionary(catalog));

        let mut pages = PdfDictionary::new();
        pages.insert("Type", PdfName::new("Pages"));
        pages.insert("Count", 0);
        pages.insert("Kids", vec![]);
        objects.insert(pages_id, Object::Dictionary(pages.clone()));

        let base_bytes = PdfWriter::write_document(&objects, root_id, None).expect("write base");
        let base_len = base_bytes.len();

        let parser = crate::parser::Parser::new(&base_bytes);
        let base_startxref = parser.find_startxref().expect("base startxref");

        // 2. Perform incremental update: modify pages dictionary (Count = 1)
        let mut updated_pages = pages.clone();
        updated_pages.insert("Count", 1);
        let modified = vec![(pages_id, Object::Dictionary(updated_pages))];

        let trailer = PdfDictionary::new();
        let inc_bytes = PdfWriter::write_incremental_update(&base_bytes, &modified, base_startxref, &trailer)
            .expect("write incremental");

        // 3. Verify byte-level invariance
        assert!(inc_bytes.len() > base_len);
        assert_eq!(&inc_bytes[..base_len], &base_bytes[..]);

        // 4. Verify incremental parser reading
        let inc_parser = crate::parser::Parser::new(&inc_bytes);
        let new_startxref = inc_parser.find_startxref().expect("new startxref");
        assert!(new_startxref >= base_len);

        let (xref_map, inc_trailer) = inc_parser
            .parse_xref_and_trailer(new_startxref)
            .expect("parse inc xref");
        assert_eq!(inc_trailer.get_i64("Prev"), Some(base_startxref as i64));
        assert_eq!(inc_trailer.get_ref("Root"), Some(root_id));
        assert!(xref_map.contains_key(&root_id));
        assert!(xref_map.contains_key(&pages_id));

        // Verify modified object resolved
        let offset = xref_map[&pages_id].offset().expect("pages uncompressed offset");
        let (_, obj) = inc_parser.parse_indirect_object_at(offset).expect("parse pages");
        let pages_dict = obj.as_dict().expect("pages dict");
        assert_eq!(pages_dict.get_i64("Count"), Some(1));
    }
}
