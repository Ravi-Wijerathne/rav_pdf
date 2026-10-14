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
}
