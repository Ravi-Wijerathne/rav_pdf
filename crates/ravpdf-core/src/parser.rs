//! PDF Parser conforming to ISO 32000-1 Section 7.3 and 7.5.
//!
//! Traceability: Section 7.1 PDF Internal Structure, DR-01, NFR-03.3, NFR-04.2.

use crate::error::{PdfError, Result};
use crate::lexer::{Lexer, Token};
use crate::object::{ObjectId, Object, PdfDictionary, PdfName, PdfStream, PdfString};
use std::collections::{BTreeMap, BTreeSet, HashSet};

/// An entry in a PDF cross-reference table or stream (ISO 32000-1 Section 7.5.4 & 7.5.8).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum XRefEntry {
    /// Type 1: An uncompressed indirect object located at a byte offset in the PDF file.
    Uncompressed {
        offset: usize,
        generation: u16,
    },
    /// Type 2: A compressed indirect object stored inside an object stream (`/ObjStm`).
    Compressed {
        container_obj_num: u32,
        index_in_stream: usize,
    },
}

impl XRefEntry {
    /// Returns byte offset for uncompressed objects, or None for compressed objects.
    pub fn offset(&self) -> Option<usize> {
        match self {
            XRefEntry::Uncompressed { offset, .. } => Some(*offset),
            XRefEntry::Compressed { .. } => None,
        }
    }
}

pub type XRefMap = BTreeMap<ObjectId, XRefEntry>;

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

        // If /Length is specified as an explicit non-negative integer in dict, we can read it directly
        let length_hint = dict
            .get_i64("Length")
            .and_then(|l| if l >= 0 { usize::try_from(l).ok() } else { None });

        let stream_bytes = match length_hint.and_then(|len| stream_start.checked_add(len)) {
            Some(stream_end) if stream_end <= self.data.len() => {
                let slice = &self.data[stream_start..stream_end];
                lexer.set_pos(stream_end);
                slice.to_vec()
            }
            _ => self.scan_for_endstream(lexer, stream_start)?,
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
    /// Returns map of ObjectId -> XRefEntry, and trailer dictionary.
    pub fn parse_xref_and_trailer(
        &self,
        start_offset: usize,
    ) -> Result<(XRefMap, PdfDictionary)> {
        let mut visited = HashSet::new();
        self.parse_xref_and_trailer_inner(start_offset, &mut visited)
    }

    fn parse_xref_and_trailer_inner(
        &self,
        start_offset: usize,
        visited: &mut HashSet<usize>,
    ) -> Result<(XRefMap, PdfDictionary)> {
        if !visited.insert(start_offset) {
            return Err(PdfError::InvalidXRef(format!(
                "Circular /Prev cross-reference loop detected at offset {}",
                start_offset
            )));
        }

        let mut lexer = Lexer::new_at(self.data, start_offset);
        lexer.skip_whitespace_and_comments();

        let token = lexer.next_token()?;
        match token {
            Token::Keyword(kw) if kw == "xref" => {
                self.parse_classic_xref_table(&mut lexer, visited)
            }
            Token::Integer(num) => {
                // XRef stream: N G obj << /Type /XRef ... >> stream ... endstream
                let gen = match lexer.next_token()? {
                    Token::Integer(g) => g as u16,
                    _ => return Err(PdfError::InvalidXRef("Invalid XRef stream header".into())),
                };
                let _ = lexer.next_token(); // consume 'obj'
                let obj = self.parse_object(&mut lexer)?;
                self.parse_xref_stream(ObjectId::new(num as u32, gen), obj, visited)
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
        visited: &mut HashSet<usize>,
    ) -> Result<(XRefMap, PdfDictionary)> {
        let mut xref_map = BTreeMap::new();
        let mut tombstoned: BTreeSet<u32> = BTreeSet::new();

        loop {
            lexer.skip_whitespace_and_comments();
            let saved = lexer.pos();
            let tok = lexer.next_token()?;

            match tok {
                Token::Keyword(kw) if kw == "trailer" => {
                    break;
                }
                Token::Integer(start_obj_num) => {
                    if start_obj_num < 0 {
                        return Err(PdfError::InvalidXRef(
                            "Negative start object number in xref section".into(),
                        ));
                    }
                    let start_u32 = match u32::try_from(start_obj_num) {
                        Ok(n) => n,
                        Err(_) => {
                            return Err(PdfError::InvalidXRef(
                                "Start object number exceeds u32 in xref section".into(),
                            ));
                        }
                    };
                    let count = match lexer.next_token()? {
                        Token::Integer(c) if c >= 0 => c as usize,
                        Token::Integer(_) => {
                            return Err(PdfError::InvalidXRef(
                                "Negative xref section count".into(),
                            ));
                        }
                        other => {
                            return Err(PdfError::InvalidXRef(format!(
                                "Expected xref section count, found {}",
                                other
                            )))
                        }
                    };

                    for i in 0..count {
                        let obj_num = match u32::try_from(i).ok().and_then(|idx| start_u32.checked_add(idx)) {
                            Some(num) => num,
                            None => {
                                return Err(PdfError::InvalidXRef(
                                    "Object number overflow in xref section".into(),
                                ));
                            }
                        };
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

                        let id = ObjectId::new(obj_num, gen);
                        if state == "n" && offset > 0 {
                            xref_map.insert(id, XRefEntry::Uncompressed { offset, generation: gen });
                        } else if state == "f" {
                            tombstoned.insert(obj_num);
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
        let mut trailer_dict = match trailer_obj {
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
                if let Ok((prev_map, prev_trailer)) =
                    self.parse_xref_and_trailer_inner(prev_offset as usize, visited)
                {
                    // ISO 32000-1 Section 7.5.6: For each object number, the entry from the most
                    // recent cross-reference section takes precedence over older entries.
                    let mut seen_numbers: BTreeSet<u32> = xref_map.keys().map(|k| k.number).collect();
                    seen_numbers.extend(&tombstoned);
                    for (id, entry) in prev_map {
                        if seen_numbers.insert(id.number) {
                            xref_map.insert(id, entry);
                        }
                    }
                    // ISO 32000-1 Section 7.5.5: Inherit earlier trailer entries if not overridden
                    for (key, val) in prev_trailer.map.iter() {
                        if !trailer_dict.map.contains_key(key) && key.as_str() != "Prev" {
                            trailer_dict.map.insert(key.clone(), val.clone());
                        }
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
        visited: &mut HashSet<usize>,
    ) -> Result<(XRefMap, PdfDictionary)> {
        let stream = match obj {
            Object::Stream(s) => s,
            _ => return Err(PdfError::InvalidXRef("Expected XRef stream object".into())),
        };

        let mut dict = stream.dict.clone();
        let decoded = stream.decode()?;

        let w_arr = dict
            .get_array("W")
            .ok_or_else(|| PdfError::InvalidXRef("Missing /W in XRef stream".into()))?;
        if w_arr.len() < 3 {
            return Err(PdfError::InvalidXRef("Invalid /W length in XRef stream".into()));
        }

        let w0_val = w_arr[0].as_i64().unwrap_or(1);
        let w1_val = w_arr[1].as_i64().unwrap_or(0);
        let w2_val = w_arr[2].as_i64().unwrap_or(0);

        if w0_val < 0 || w1_val < 0 || w2_val < 0 {
            return Err(PdfError::InvalidXRef(
                "Negative field width in XRef stream /W".into(),
            ));
        }

        let w0 = w0_val as usize;
        let w1 = w1_val as usize;
        let w2 = w2_val as usize;

        let entry_size = match w0.checked_add(w1).and_then(|sum| sum.checked_add(w2)) {
            Some(sum) if sum > 0 => sum,
            Some(_) => {
                return Err(PdfError::InvalidXRef(
                    "Invalid /W in XRef stream: entry size must be greater than zero".into(),
                ));
            }
            None => {
                return Err(PdfError::InvalidXRef(
                    "Arithmetic overflow calculating /W entry size in XRef stream".into(),
                ));
            }
        };

        let size = dict
            .get_i64("Size")
            .and_then(|s| if s >= 0 { usize::try_from(s).ok() } else { None })
            .unwrap_or(0);

        let index_pairs = if let Some(idx_arr) = dict.get_array("Index") {
            let mut pairs = Vec::new();
            let mut i = 0;
            while i + 1 < idx_arr.len() {
                let start_val = idx_arr[i].as_i64().unwrap_or(0);
                let count_val = idx_arr[i + 1].as_i64().unwrap_or(0);
                if start_val < 0 || count_val < 0 {
                    return Err(PdfError::InvalidXRef(
                        "Negative /Index entry in XRef stream".into(),
                    ));
                }
                let start = match u32::try_from(start_val) {
                    Ok(n) => n,
                    Err(_) => {
                        return Err(PdfError::InvalidXRef(
                            "/Index start object number exceeds u32".into(),
                        ));
                    }
                };
                let count = match usize::try_from(count_val) {
                    Ok(n) => n,
                    Err(_) => {
                        return Err(PdfError::InvalidXRef(
                            "/Index count exceeds usize".into(),
                        ));
                    }
                };
                pairs.push((start, count));
                i += 2;
            }
            pairs
        } else {
            vec![(0, size)]
        };

        let mut xref_map = BTreeMap::new();
        let mut tombstoned: BTreeSet<u32> = BTreeSet::new();
        let mut byte_idx: usize = 0;

        for (start_id, count) in index_pairs {
            for i in 0..count {
                let next_byte_idx = match byte_idx.checked_add(entry_size) {
                    Some(next) if next <= decoded.len() => next,
                    _ => break,
                };
                let entry = &decoded[byte_idx..next_byte_idx];
                byte_idx = next_byte_idx;

                let entry_type = if w0 == 0 {
                    1 // Default entry type is 1
                } else {
                    read_big_endian_uint(&entry[..w0])
                };

                let f1 = read_big_endian_uint(&entry[w0..w0 + w1]);
                let f2 = read_big_endian_uint(&entry[w0 + w1..entry_size]);

                let obj_num = match u32::try_from(i).ok().and_then(|idx| start_id.checked_add(idx)) {
                    Some(num) => num,
                    None => {
                        return Err(PdfError::InvalidXRef(
                            "Object number overflow in XRef stream /Index".into(),
                        ));
                    }
                };

                match entry_type {
                    0 => {
                        // Free entry: f1 is next free obj num, f2 is generation number
                        tombstoned.insert(obj_num);
                    }
                    1 => {
                        // Uncompressed object at byte offset f1, generation f2
                        let offset = f1 as usize;
                        let gen = f2 as u16;
                        xref_map.insert(
                            ObjectId::new(obj_num, gen),
                            XRefEntry::Uncompressed {
                                offset,
                                generation: gen,
                            },
                        );
                    }
                    2 => {
                        // Compressed object inside object stream f1 at index f2
                        // ISO 32000-1 7.5.8: Generation number of compressed objects is implicitly 0
                        let container_obj_num = f1 as u32;
                        let index_in_stream = f2 as usize;
                        xref_map.insert(
                            ObjectId::new(obj_num, 0),
                            XRefEntry::Compressed {
                                container_obj_num,
                                index_in_stream,
                            },
                        );
                    }
                    _ => {}
                }
            }
        }

        // Follow /Prev if present
        if let Some(prev_offset) = dict.get_i64("Prev") {
            if prev_offset > 0 && (prev_offset as usize) < self.data.len() {
                if let Ok((prev_map, prev_trailer)) =
                    self.parse_xref_and_trailer_inner(prev_offset as usize, visited)
                {
                    // ISO 32000-1 Section 7.5.6 precedence
                    let mut seen_numbers: BTreeSet<u32> = xref_map.keys().map(|k| k.number).collect();
                    seen_numbers.extend(&tombstoned);
                    for (id, entry) in prev_map {
                        if seen_numbers.insert(id.number) {
                            xref_map.insert(id, entry);
                        }
                    }
                    // Inherit earlier trailer entries if not overridden
                    for (key, val) in prev_trailer.map.iter() {
                        if !dict.map.contains_key(key) && key.as_str() != "Prev" {
                            dict.map.insert(key.clone(), val.clone());
                        }
                    }
                }
            }
        }

        Ok((xref_map, dict))
    }

    /// Parses an ISO 32000-1 Section 7.5.7 Object Stream (`/Type /ObjStm`).
    ///
    /// Object streams store multiple indirect objects in compressed form without `obj` and `endobj` wrappers.
    /// The decoded stream begins with `N` pairs of integers `(obj_num, byte_offset)` relative to `/First`.
    pub fn parse_object_stream(stream_obj: &PdfStream) -> Result<Vec<(ObjectId, Object)>> {
        if let Some(type_name) = stream_obj.dict.get_name("Type") {
            if type_name != "ObjStm" {
                return Err(PdfError::InvalidStructure(format!(
                    "Expected /Type /ObjStm, found /Type /{}",
                    type_name
                )));
            }
        }

        let n = stream_obj
            .dict
            .get_i64("N")
            .ok_or_else(|| PdfError::InvalidStructure("Missing /N in /ObjStm stream".into()))?;
        if n < 0 {
            return Err(PdfError::InvalidStructure("Negative /N in /ObjStm stream".into()));
        }
        if n == 0 {
            return Ok(Vec::new());
        }

        let first = stream_obj
            .dict
            .get_i64("First")
            .ok_or_else(|| PdfError::InvalidStructure("Missing /First in /ObjStm stream".into()))?
            as usize;

        let decoded = stream_obj.decode()?;
        if first > decoded.len() {
            return Err(PdfError::InvalidStructure(format!(
                "/First byte offset {} exceeds decoded object stream length {}",
                first,
                decoded.len()
            )));
        }

        let mut header_lexer = Lexer::new(&decoded[..first]);
        let mut pairs = Vec::with_capacity((n as usize).min(65536));

        for _ in 0..n {
            let obj_num = match header_lexer.next_token()? {
                Token::Integer(num) if num >= 0 => num as u32,
                Token::Integer(num) => {
                    return Err(PdfError::ParseError(format!(
                        "Negative object number in /ObjStm header: {}",
                        num
                    )))
                }
                other => {
                    return Err(PdfError::ParseError(format!(
                        "Expected object number integer in /ObjStm header, found {}",
                        other
                    )))
                }
            };

            let rel_offset = match header_lexer.next_token()? {
                Token::Integer(off) if off >= 0 => off as usize,
                Token::Integer(off) => {
                    return Err(PdfError::ParseError(format!(
                        "Negative relative offset in /ObjStm header: {}",
                        off
                    )))
                }
                other => {
                    return Err(PdfError::ParseError(format!(
                        "Expected offset integer in /ObjStm header, found {}",
                        other
                    )))
                }
            };

            pairs.push((obj_num, rel_offset));
        }

        let mut results = Vec::with_capacity(pairs.len());
        let obj_parser = Parser::new(&decoded);

        for (obj_num, rel_offset) in pairs {
            let abs_offset = first.checked_add(rel_offset).ok_or_else(|| {
                PdfError::InvalidStructure("Offset overflow in /ObjStm".into())
            })?;

            if abs_offset >= decoded.len() {
                return Err(PdfError::InvalidStructure(format!(
                    "Object offset {} exceeds object stream length {}",
                    abs_offset,
                    decoded.len()
                )));
            }

            let mut obj_lexer = Lexer::new_at(&decoded, abs_offset);
            let obj = obj_parser.parse_object(&mut obj_lexer)?;

            // ISO 32000-1 Clause 7.5.7: "An object stream shall not contain any stream objects."
            if matches!(obj, Object::Stream(_)) {
                return Err(PdfError::InvalidStructure(
                    "Stream object inside /ObjStm is forbidden by ISO 32000-1 Clause 7.5.7".into(),
                ));
            }

            // ISO 32000-1 Clause 7.5.8: Generation of objects in /ObjStm is always 0
            results.push((ObjectId::new(obj_num, 0), obj));
        }

        Ok(results)
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

    #[test]
    fn test_parse_object_stream_plain() {
        let mut dict = PdfDictionary::new();
        dict.insert("Type", PdfName::new("ObjStm"));
        dict.insert("N", 3);
        dict.insert("First", 15);

        // Header: obj 10 at 0, obj 11 at 5, obj 12 at 8
        // Data starting at 15: "true\n42\n<< /Key (val) >>\n"
        let raw = b"10 0 11 5 12 8\ntrue\n42\n<< /Key (val) >>\n";
        let stream = PdfStream::with_plain_data(dict, raw.to_vec());

        let parsed = Parser::parse_object_stream(&stream).expect("parse object stream");
        assert_eq!(parsed.len(), 3);

        assert_eq!(parsed[0].0, ObjectId::new(10, 0));
        assert_eq!(parsed[0].1, Object::Boolean(true));

        assert_eq!(parsed[1].0, ObjectId::new(11, 0));
        assert_eq!(parsed[1].1, Object::Integer(42));

        assert_eq!(parsed[2].0, ObjectId::new(12, 0));
        let d = parsed[2].1.as_dict().unwrap();
        assert_eq!(d.get_str("Key"), Some("val".into()));
    }

    #[test]
    fn test_parse_object_stream_flate() {
        let mut dict = PdfDictionary::new();
        dict.insert("Type", PdfName::new("ObjStm"));
        dict.insert("N", 2);
        dict.insert("First", 10);

        let raw = b"20 0 21 8\n[1 2 3]\n(hello)\n";
        let mut stream = PdfStream::with_plain_data(dict, raw.to_vec());
        stream.compress_flate().expect("compress flate");

        let parsed = Parser::parse_object_stream(&stream).expect("parse flate objstm");
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].0, ObjectId::new(20, 0));
        assert_eq!(parsed[0].1.as_array().unwrap().len(), 3);
        assert_eq!(parsed[1].0, ObjectId::new(21, 0));
        assert_eq!(parsed[1].1.as_str().unwrap(), "hello");
    }

    #[test]
    fn test_parse_object_stream_forbidden_stream() {
        let mut dict = PdfDictionary::new();
        dict.insert("Type", PdfName::new("ObjStm"));
        dict.insert("N", 1);
        dict.insert("First", 5);

        let raw = b"30 0\n<< /Length 4 >>\nstream\n1234\nendstream\n";
        let stream = PdfStream::with_plain_data(dict, raw.to_vec());

        let res = Parser::parse_object_stream(&stream);
        assert!(res.is_err());
        assert!(matches!(res.unwrap_err(), PdfError::InvalidStructure(_)));
    }

    #[test]
    fn test_parse_object_stream_corrupt_header() {
        let mut dict = PdfDictionary::new();
        dict.insert("Type", PdfName::new("ObjStm"));
        dict.insert("N", 2);
        dict.insert("First", 10);

        // Header only has 1 pair instead of 2
        let raw = b"10 0\n[true]\n";
        let stream = PdfStream::with_plain_data(dict, raw.to_vec());

        let res = Parser::parse_object_stream(&stream);
        assert!(res.is_err());
    }

    #[test]
    fn test_xref_stream_type_2_parsing() {
        // Construct a synthetic XRef stream with W = [1, 2, 1]
        // Entry 1: Type 1 (uncompressed): offset 120, gen 0 -> obj 1
        // Entry 2: Type 2 (compressed): container 10, index 0 -> obj 2
        let mut dict = PdfDictionary::new();
        dict.insert("Type", PdfName::new("XRef"));
        dict.insert("Size", 3);
        dict.insert(
            "W",
            vec![Object::Integer(1), Object::Integer(2), Object::Integer(1)],
        );
        dict.insert("Index", vec![Object::Integer(1), Object::Integer(2)]);

        // 4 bytes per entry:
        // Entry 1: [1, 0, 120, 0] -> Type 1, offset 120, gen 0
        // Entry 2: [2, 0, 10, 0]  -> Type 2, container 10, index 0
        let data = vec![
            1, 0, 120, 0,
            2, 0, 10, 0,
        ];

        let stream = PdfStream::with_plain_data(dict, data);
        let parser = Parser::new(b"");
        let mut visited = HashSet::new();
        let (xref_map, _) = parser
            .parse_xref_stream(ObjectId::new(99, 0), Object::Stream(stream), &mut visited)
            .expect("parse xref stream");

        assert_eq!(
            xref_map.get(&ObjectId::new(1, 0)),
            Some(&XRefEntry::Uncompressed {
                offset: 120,
                generation: 0,
            })
        );
        assert_eq!(
            xref_map.get(&ObjectId::new(2, 0)),
            Some(&XRefEntry::Compressed {
                container_obj_num: 10,
                index_in_stream: 0,
            })
        );
    }

    #[test]
    fn test_parse_stream_negative_length_resilience() {
        // Must fall back to scan_for_endstream without panicking
        let raw = b"1 0 obj\n<< /Length -500 >>\nstream\nhello payload\nendstream\nendobj\n";
        let parser = Parser::new(raw);
        let (id, obj) = parser.parse_indirect_object_at(0).expect("parse object");
        assert_eq!(id, ObjectId::new(1, 0));
        if let Object::Stream(s) = obj {
            assert_eq!(s.raw_data, b"hello payload");
        } else {
            panic!("Expected Stream object");
        }
    }

    #[test]
    fn test_parse_stream_overflowing_length_resilience() {
        // High length integer that would overflow stream_start + len
        let raw = b"2 0 obj\n<< /Length 9223372036854775807 >>\nstream\ncontent\nendstream\nendobj\n";
        let parser = Parser::new(raw);
        let (id, obj) = parser.parse_indirect_object_at(0).expect("parse object");
        assert_eq!(id, ObjectId::new(2, 0));
        if let Object::Stream(s) = obj {
            assert_eq!(s.raw_data, b"content");
        } else {
            panic!("Expected Stream object");
        }
    }

    #[test]
    fn test_parse_xref_stream_invalid_w_rejected() {
        // Negative /W
        let raw_neg_w = b"1 0 obj\n<< /Type /XRef /Size 2 /W [-1 1 0] >>\nstream\n\x01\x10endstream\nendobj\nstartxref\n0\n%%EOF";
        let p_neg = Parser::new(raw_neg_w);
        assert!(matches!(p_neg.parse_xref_and_trailer(0), Err(PdfError::InvalidXRef(_))));

        // Zero total entry size /W [0 0 0]
        let raw_zero_w = b"1 0 obj\n<< /Type /XRef /Size 2 /W [0 0 0] >>\nstream\nendstream\nendobj\nstartxref\n0\n%%EOF";
        let p_zero = Parser::new(raw_zero_w);
        assert!(matches!(p_zero.parse_xref_and_trailer(0), Err(PdfError::InvalidXRef(_))));
    }

    #[test]
    fn test_parse_xref_stream_index_overflow_rejected() {
        let raw_overflow = b"1 0 obj\n<< /Type /XRef /Size 10 /W [1 1 0] /Index [4294967295 5] >>\nstream\n\x01\x00\x01\x00endstream\nendobj\nstartxref\n0\n%%EOF";
        let p_overflow = Parser::new(raw_overflow);
        assert!(matches!(p_overflow.parse_xref_and_trailer(0), Err(PdfError::InvalidXRef(_))));
    }
}
