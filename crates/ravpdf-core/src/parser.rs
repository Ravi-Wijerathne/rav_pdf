//! PDF Parser conforming to ISO 32000-1 Section 7.3 and 7.5.
//!
//! Traceability: Section 7.1 PDF Internal Structure, DR-01, NFR-03.3, NFR-04.2.

use crate::error::{PdfError, Result};
use crate::lexer::{Lexer, Token};
use crate::object::{ObjectId, Object, PdfDictionary, PdfName, PdfStream, PdfString};
use std::collections::BTreeMap;

pub struct Parser<'a> {
    data: &'a [u8],
}

impl<'a> Parser<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self { data }
    }

    /// Parses an object at the current lexer position.
    pub fn parse_object(&self, lexer: &mut Lexer<'a>) -> Result<Object> {
        let token = lexer.next_token()?;

        match token {
            Token::Keyword(kw) => match kw.as_str() {
                "true" => Ok(Object::Boolean(true)),
                "false" => Ok(Object::Boolean(false)),
                "null" => Ok(Object::Null),
                other => Err(PdfError::ParseError(format!("Unexpected keyword: {}", other))),
            },
            Token::Integer(val) => {
                // Check if this integer is part of an indirect reference: N G R
                let saved_pos = lexer.pos();
                if let Ok(Token::Integer(gen)) = lexer.next_token() {
                    if let Ok(Token::Keyword(r)) = lexer.next_token() {
                        if r == "R" {
                            return Ok(Object::Reference(ObjectId::new(val as u32, gen as u16)));
                        }
                    }
                }
                // Backtrack if not an indirect reference
                lexer.set_pos(saved_pos);
                Ok(Object::Integer(val))
            }
            Token::Real(val) => Ok(Object::Real(val)),
            Token::StringLiteral(bytes) => Ok(Object::String(PdfString::from_bytes(bytes))),
            Token::HexString(bytes) => Ok(Object::String(PdfString::from_hex(bytes))),
            Token::Name(name) => Ok(Object::Name(PdfName::new(name))),
            Token::ArrayOpen => self.parse_array(lexer),
            Token::DictOpen => {
                let dict = self.parse_dictionary(lexer)?;

                // Check if this dictionary is followed by a `stream` keyword
                let saved_pos = lexer.pos();
                lexer.skip_whitespace_and_comments();
                if let Ok(Token::Keyword(kw)) = lexer.next_token() {
                    if kw == "stream" {
                        return self.parse_stream(lexer, dict);
                    }
                }
                lexer.set_pos(saved_pos);
                Ok(Object::Dictionary(dict))
            }
            Token::ArrayClose | Token::DictClose => {
                Err(PdfError::ParseError("Unexpected closing delimiter".into()))
            }
            Token::Eof => Err(PdfError::UnexpectedEof),
        }
    }

    /// Parses an array: `[ item1 item2 ... ]`
    fn parse_array(&self, lexer: &mut Lexer<'a>) -> Result<Object> {
        let mut items = Vec::new();

        loop {
            lexer.skip_whitespace_and_comments();
            if lexer.peek_byte() == Some(b']') {
                lexer.next_byte();
                break;
            }

            let item = self.parse_object(lexer)?;
            items.push(item);
        }

        Ok(Object::Array(items))
    }

    /// Parses a dictionary: `<< /Key1 Val1 /Key2 Val2 ... >>`
    fn parse_dictionary(&self, lexer: &mut Lexer<'a>) -> Result<PdfDictionary> {
        let mut dict = PdfDictionary::new();

        loop {
            lexer.skip_whitespace_and_comments();

            // Check for closing `>>`
            if lexer.remaining().starts_with(b">>") {
                lexer.set_pos(lexer.pos() + 2);
                break;
            }

            let key_token = lexer.next_token()?;
            let key = match key_token {
                Token::Name(n) => n,
                Token::DictClose => break,
                Token::Eof => return Err(PdfError::UnexpectedEof),
                other => {
                    return Err(PdfError::ParseError(format!(
                        "Expected dictionary key (Name), found: {}",
                        other
                    )))
                }
            };

            let val = self.parse_object(lexer)?;
            dict.insert(key, val);
        }

        Ok(dict)
    }

    /// Parses stream content following `stream\r\n` or `stream\n`.
    fn parse_stream(&self, lexer: &mut Lexer<'a>, dict: PdfDictionary) -> Result<Object> {
        // Stream data begins after CRLF or LF following the `stream` token
        if lexer.peek_byte() == Some(b'\r') {
            lexer.next_byte();
        }
        if lexer.peek_byte() == Some(b'\n') {
            lexer.next_byte();
        }

        let stream_start = lexer.pos();

        // If /Length is specified as an explicit integer in dict, we can read it directly
        let length_hint = dict.get_i64("Length").map(|l| l as usize);

        let stream_bytes = if let Some(len) = length_hint {
            if stream_start + len <= self.data.len() {
                let slice = &self.data[stream_start..stream_start + len];
                lexer.set_pos(stream_start + len);
                slice.to_vec()
            } else {
                self.scan_for_endstream(lexer, stream_start)?
            }
        } else {
            self.scan_for_endstream(lexer, stream_start)?
        };

        // Consume trailing whitespace and `endstream`
        lexer.skip_whitespace_and_comments();
        if let Ok(Token::Keyword(kw)) = lexer.next_token() {
            if kw != "endstream" {
                // If not exactly endstream, try scanning forward to resynchronize
                tracing::warn!("Expected 'endstream', found keyword: {}", kw);
            }
        }

        Ok(Object::Stream(PdfStream::new(dict, stream_bytes)))
    }

    /// Fallback stream content reader by scanning for `endstream`.
    fn scan_for_endstream(&self, lexer: &mut Lexer<'a>, stream_start: usize) -> Result<Vec<u8>> {
        let rem = &self.data[stream_start..];
        let endstream_marker = b"endstream";

        if let Some(pos) = rem.windows(endstream_marker.len()).position(|w| w == endstream_marker) {
            let mut end_pos = pos;
            // Trim trailing \r, \n immediately preceding endstream
            while end_pos > 0 && (rem[end_pos - 1] == b'\r' || rem[end_pos - 1] == b'\n') {
                end_pos -= 1;
            }
            lexer.set_pos(stream_start + pos);
            Ok(rem[..end_pos].to_vec())
        } else {
            Err(PdfError::ParseError("Missing 'endstream' marker".into()))
        }
    }

    /// Parses an indirect object definition: `N G obj <Object> endobj`
    pub fn parse_indirect_object_at(&self, offset: usize) -> Result<(ObjectId, Object)> {
        let mut lexer = Lexer::new_at(self.data, offset);

        let num = match lexer.next_token()? {
            Token::Integer(n) => n as u32,
            other => {
                return Err(PdfError::ParseError(format!(
                    "Expected object number at offset {}, found {}",
                    offset, other
                )))
            }
        };

        let gen = match lexer.next_token()? {
            Token::Integer(g) => g as u16,
            other => {
                return Err(PdfError::ParseError(format!(
                    "Expected generation number at offset {}, found {}",
                    offset, other
                )))
            }
        };

        match lexer.next_token()? {
            Token::Keyword(kw) if kw == "obj" => {}
            other => {
                return Err(PdfError::ParseError(format!(
                    "Expected 'obj' keyword, found {}",
                    other
                )))
            }
        }

        let obj = self.parse_object(&mut lexer)?;

        // Consume optional `endobj`
        lexer.skip_whitespace_and_comments();
        let _ = lexer.next_token();

        Ok((ObjectId::new(num, gen), obj))
    }

    /// Finds the byte offset of the last `startxref` keyword from the end of the file.
    pub fn find_startxref(&self) -> Result<usize> {
        let search_window = 2048.min(self.data.len());
        let tail = &self.data[self.data.len() - search_window..];
        let marker = b"startxref";

        if let Some(idx) = tail.windows(marker.len()).rposition(|w| w == marker) {
            let abs_pos = (self.data.len() - search_window) + idx + marker.len();
            let mut lexer = Lexer::new_at(self.data, abs_pos);
            match lexer.next_token()? {
                Token::Integer(offset) => Ok(offset as usize),
                other => Err(PdfError::InvalidXRef(format!(
                    "Expected startxref offset, found {}",
                    other
                ))),
            }
        } else {
            Err(PdfError::InvalidXRef("Cannot find 'startxref' in file tail".into()))
        }
    }

    /// Parses cross-reference table or stream starting at `startxref_offset`.
    /// Returns map of ObjectId -> byte offset, and trailer dictionary.
    pub fn parse_xref_and_trailer(
        &self,
        start_offset: usize,
    ) -> Result<(BTreeMap<ObjectId, usize>, PdfDictionary)> {
        let mut lexer = Lexer::new_at(self.data, start_offset);
        lexer.skip_whitespace_and_comments();

        let token = lexer.next_token()?;
        match token {
            Token::Keyword(kw) if kw == "xref" => {
                self.parse_classic_xref_table(&mut lexer)
            }
            Token::Integer(num) => {
                // XRef stream: N G obj << /Type /XRef ... >> stream ... endstream
                let gen = match lexer.next_token()? {
                    Token::Integer(g) => g as u16,
                    _ => return Err(PdfError::InvalidXRef("Invalid XRef stream header".into())),
                };
                let _ = lexer.next_token(); // consume 'obj'
                let obj = self.parse_object(&mut lexer)?;
                self.parse_xref_stream(ObjectId::new(num as u32, gen), obj)
            }
            other => Err(PdfError::InvalidXRef(format!(
                "Expected 'xref' or XRef stream object, found {}",
                other
            ))),
        }
    }

    /// Parses a classic `xref` table subsection and its associated `trailer`.
    fn parse_classic_xref_table(
        &self,
        lexer: &mut Lexer<'a>,
    ) -> Result<(BTreeMap<ObjectId, usize>, PdfDictionary)> {
        let mut xref_map = BTreeMap::new();

        loop {
            lexer.skip_whitespace_and_comments();
            let saved = lexer.pos();
            let tok = lexer.next_token()?;

            match tok {
                Token::Keyword(kw) if kw == "trailer" => {
                    break;
                }
                Token::Integer(start_obj_num) => {
                    let count = match lexer.next_token()? {
                        Token::Integer(c) => c as usize,
                        other => {
                            return Err(PdfError::InvalidXRef(format!(
                                "Expected xref section count, found {}",
                                other
                            )))
                        }
                    };

                    for i in 0..count {
                        let obj_num = (start_obj_num as usize + i) as u32;
                        let offset = match lexer.next_token()? {
                            Token::Integer(off) => off as usize,
                            other => {
                                return Err(PdfError::InvalidXRef(format!(
                                    "Expected xref entry offset, found {}",
                                    other
                                )))
                            }
                        };
                        let gen = match lexer.next_token()? {
                            Token::Integer(g) => g as u16,
                            other => {
                                return Err(PdfError::InvalidXRef(format!(
                                    "Expected xref entry gen, found {}",
                                    other
                                )))
                            }
                        };
                        let state = match lexer.next_token()? {
                            Token::Keyword(s) => s,
                            other => {
                                return Err(PdfError::InvalidXRef(format!(
                                    "Expected xref entry flag (n/f), found {}",
                                    other
                                )))
                            }
                        };

                        if state == "n" && offset > 0 {
                            xref_map.insert(ObjectId::new(obj_num, gen), offset);
                        }
                    }
                }
                other => {
                    lexer.set_pos(saved);
                    return Err(PdfError::InvalidXRef(format!(
                        "Expected xref section start or trailer, found {}",
                        other
                    )));
                }
            }
        }

        // Now parse the trailer dictionary
        let trailer_obj = self.parse_object(lexer)?;
        let trailer_dict = match trailer_obj {
            Object::Dictionary(d) => d,
            other => {
                return Err(PdfError::InvalidXRef(format!(
                    "Expected trailer dictionary, found {}",
                    other.type_name()
                )))
            }
        };

        // If /Prev exists, recursively merge earlier xref entries (incremental saves)
        if let Some(prev_offset) = trailer_dict.get_i64("Prev") {
            if prev_offset > 0 && (prev_offset as usize) < self.data.len() {
                if let Ok((prev_map, _)) = self.parse_xref_and_trailer(prev_offset as usize) {
                    for (id, offset) in prev_map {
                        xref_map.entry(id).or_insert(offset);
                    }
                }
            }
        }

        Ok((xref_map, trailer_dict))
    }

    /// Parses an ISO 32000-1 Section 7.5.8 Cross-Reference Stream (`/Type /XRef`).
    fn parse_xref_stream(
        &self,
        _stream_id: ObjectId,
        obj: Object,
    ) -> Result<(BTreeMap<ObjectId, usize>, PdfDictionary)> {
        let stream = match obj {
            Object::Stream(s) => s,
            _ => return Err(PdfError::InvalidXRef("Expected XRef stream object".into())),
        };

        let dict = stream.dict.clone();
        let decoded = stream.decode()?;

        let w_arr = dict
            .get_array("W")
            .ok_or_else(|| PdfError::InvalidXRef("Missing /W in XRef stream".into()))?;
        if w_arr.len() < 3 {
            return Err(PdfError::InvalidXRef("Invalid /W length in XRef stream".into()));
        }

        let w0 = w_arr[0].as_i64().unwrap_or(1) as usize;
        let w1 = w_arr[1].as_i64().unwrap_or(0) as usize;
        let w2 = w_arr[2].as_i64().unwrap_or(0) as usize;
        let entry_size = w0 + w1 + w2;

        let size = dict.get_i64("Size").unwrap_or(0) as usize;
        let index_pairs = if let Some(idx_arr) = dict.get_array("Index") {
            let mut pairs = Vec::new();
            let mut i = 0;
            while i + 1 < idx_arr.len() {
                let start = idx_arr[i].as_i64().unwrap_or(0) as u32;
                let count = idx_arr[i + 1].as_i64().unwrap_or(0) as usize;
                pairs.push((start, count));
                i += 2;
            }
            pairs
        } else {
            vec![(0, size)]
        };

        let mut xref_map = BTreeMap::new();
        let mut byte_idx = 0;

        for (start_id, count) in index_pairs {
            for i in 0..count {
                if byte_idx + entry_size > decoded.len() {
                    break;
                }
                let entry = &decoded[byte_idx..byte_idx + entry_size];
                byte_idx += entry_size;

                let entry_type = if w0 == 0 {
                    1 // Default entry type is 1
                } else {
                    read_big_endian_uint(&entry[..w0])
                };

                let f1 = read_big_endian_uint(&entry[w0..w0 + w1]);
                let f2 = read_big_endian_uint(&entry[w0 + w1..entry_size]);

                let obj_num = start_id + i as u32;

                match entry_type {
                    0 => {
                        // Free entry
                    }
                    1 => {
                        // Uncompressed object at byte offset f1
                        let offset = f1 as usize;
                        let gen = f2 as u16;
                        xref_map.insert(ObjectId::new(obj_num, gen), offset);
                    }
                    2 => {
                        // Compressed object inside object stream f1 at index f2
                        // For now record object ID
                    }
                    _ => {}
                }
            }
        }

        // Follow /Prev if present
        if let Some(prev_offset) = dict.get_i64("Prev") {
            if prev_offset > 0 && (prev_offset as usize) < self.data.len() {
                if let Ok((prev_map, _)) = self.parse_xref_and_trailer(prev_offset as usize) {
                    for (id, offset) in prev_map {
                        xref_map.entry(id).or_insert(offset);
                    }
                }
            }
        }

        Ok((xref_map, dict))
    }
}

fn read_big_endian_uint(bytes: &[u8]) -> u64 {
    let mut val = 0u64;
    for &b in bytes {
        val = (val << 8) | (b as u64);
    }
    val
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_pdf_objects() {
        let pdf_data = b"5 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n";
        let parser = Parser::new(pdf_data);
        let (id, obj) = parser.parse_indirect_object_at(0).unwrap();

        assert_eq!(id, ObjectId::new(5, 0));
        let dict = obj.as_dict().unwrap();
        assert_eq!(dict.get_name("Type"), Some("Catalog"));
        assert_eq!(dict.get_ref("Pages"), Some(ObjectId::new(2, 0)));
    }
}
