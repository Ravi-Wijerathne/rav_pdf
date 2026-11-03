//! CMap parser and font encoding conforming to ISO 32000-1 Section 9.7 and 9.10.
//!
//! Traceability: FR-01.1.1, FR-04.1.

use crate::document::PdfDocument;
use crate::object::{Object, PdfDictionary};
use std::collections::HashMap;

/// Parsed CMap mapping character codes (CIDs or 8-bit/16-bit codes) to Unicode strings.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CMap {
    pub name: String,
    pub is_16bit: bool,
    pub code_to_unicode: HashMap<u32, String>,
}

#[derive(Debug, PartialEq)]
enum CMapToken {
    Word(String),
    Hex(String),
    OpenBracket,
    CloseBracket,
}

fn tokenize_cmap(text: &str) -> Vec<CMapToken> {
    let mut tokens = Vec::new();
    let mut chars = text.chars().peekable();

    while let Some(&c) = chars.peek() {
        if c.is_whitespace() {
            chars.next();
        } else if c == '%' {
            for ch in chars.by_ref() {
                if ch == '\n' || ch == '\r' {
                    break;
                }
            }
        } else if c == '<' {
            chars.next();
            if chars.peek() == Some(&'<') {
                chars.next(); // Consume second '<' of '<<'
                continue;
            }
            let mut hex = String::new();
            for ch in chars.by_ref() {
                if ch == '>' {
                    break;
                }
                if !ch.is_whitespace() {
                    hex.push(ch);
                }
            }
            tokens.push(CMapToken::Hex(hex));
        } else if c == '>' {
            chars.next();
            if chars.peek() == Some(&'>') {
                chars.next(); // Consume second '>' of '>>'
            }
        } else if c == '[' {
            chars.next();
            tokens.push(CMapToken::OpenBracket);
        } else if c == ']' {
            chars.next();
            tokens.push(CMapToken::CloseBracket);
        } else {
            let mut word = String::new();
            while let Some(&ch) = chars.peek() {
                if ch.is_whitespace()
                    || ch == '<'
                    || ch == '>'
                    || ch == '['
                    || ch == ']'
                    || ch == '%'
                {
                    break;
                }
                word.push(ch);
                chars.next();
            }
            if word.is_empty() {
                chars.next();
            } else {
                tokens.push(CMapToken::Word(word));
            }
        }
    }

    tokens
}

fn hex_to_unicode_string(hex: &str) -> String {
    let mut u16_units = Vec::new();
    let bytes = hex.as_bytes();
    let mut i = 0;
    while i + 3 < bytes.len() {
        if let Ok(chunk) = std::str::from_utf8(&bytes[i..i + 4]) {
            if let Ok(val) = u16::from_str_radix(chunk, 16) {
                u16_units.push(val);
            }
        }
        i += 4;
    }
    if !u16_units.is_empty() {
        String::from_utf16_lossy(&u16_units)
    } else if let Ok(val) = u32::from_str_radix(hex, 16) {
        char::from_u32(val).map(|c| c.to_string()).unwrap_or_default()
    } else {
        String::new()
    }
}

impl CMap {
    /// Parses a CMap from raw stream bytes (ISO 32000-1 Clause 9.10).
    pub fn parse(data: &[u8]) -> Self {
        let mut cmap = CMap {
            is_16bit: true,
            ..Default::default()
        };

        let text = String::from_utf8_lossy(data);
        let tokens = tokenize_cmap(&text);

        let mut idx = 0;
        while idx < tokens.len() {
            match &tokens[idx] {
                CMapToken::Word(w) if w == "begincodespacerange" => {
                    idx += 1;
                    while idx < tokens.len() {
                        if let CMapToken::Word(end) = &tokens[idx] {
                            if end == "endcodespacerange" {
                                break;
                            }
                        }
                        if let CMapToken::Hex(h1) = &tokens[idx] {
                            cmap.is_16bit = h1.len() > 2;
                        }
                        idx += 1;
                    }
                }
                CMapToken::Word(w) if w == "beginbfchar" => {
                    idx += 1;
                    while idx < tokens.len() {
                        if let CMapToken::Word(end) = &tokens[idx] {
                            if end == "endbfchar" {
                                break;
                            }
                        }
                        if idx + 1 < tokens.len() {
                            if let (CMapToken::Hex(src_hex), CMapToken::Hex(dst_hex)) =
                                (&tokens[idx], &tokens[idx + 1])
                            {
                                if let Ok(src_code) = u32::from_str_radix(src_hex, 16) {
                                    let dst_str = hex_to_unicode_string(dst_hex);
                                    cmap.code_to_unicode.insert(src_code, dst_str);
                                }
                                idx += 2;
                                continue;
                            }
                        }
                        idx += 1;
                    }
                }
                CMapToken::Word(w) if w == "beginbfrange" => {
                    idx += 1;
                    while idx < tokens.len() {
                        if let CMapToken::Word(end) = &tokens[idx] {
                            if end == "endbfrange" {
                                break;
                            }
                        }
                        if idx + 2 < tokens.len() {
                            if let (CMapToken::Hex(start_hex), CMapToken::Hex(end_hex)) =
                                (&tokens[idx], &tokens[idx + 1])
                            {
                                if let (Ok(start_code), Ok(end_code)) = (
                                    u32::from_str_radix(start_hex, 16),
                                    u32::from_str_radix(end_hex, 16),
                                ) {
                                    if let CMapToken::OpenBracket = &tokens[idx + 2] {
                                        idx += 3;
                                        let mut cur_code = start_code;
                                        while idx < tokens.len() && cur_code <= end_code {
                                            if let CMapToken::CloseBracket = &tokens[idx] {
                                                idx += 1;
                                                break;
                                            }
                                            if let CMapToken::Hex(dst_hex) = &tokens[idx] {
                                                let dst_str = hex_to_unicode_string(dst_hex);
                                                cmap.code_to_unicode.insert(cur_code, dst_str);
                                                cur_code += 1;
                                            }
                                            idx += 1;
                                        }
                                        continue;
                                    } else if let CMapToken::Hex(dst_start_hex) = &tokens[idx + 2] {
                                        if let Ok(dst_start_code) =
                                            u32::from_str_radix(dst_start_hex, 16)
                                        {
                                            for offset in 0..=(end_code.saturating_sub(start_code)) {
                                                let cur_code = start_code + offset;
                                                let cur_dst = dst_start_code + offset;
                                                if let Some(ch) = char::from_u32(cur_dst) {
                                                    cmap.code_to_unicode
                                                        .insert(cur_code, ch.to_string());
                                                }
                                            }
                                        }
                                        idx += 3;
                                        continue;
                                    }
                                }
                            }
                        }
                        idx += 1;
                    }
                }
                _ => {
                    idx += 1;
                }
            }
        }

        cmap
    }

    /// Decodes a byte sequence into a Unicode string according to this CMap.
    pub fn decode_bytes(&self, bytes: &[u8]) -> String {
        let mut out = String::with_capacity(bytes.len());
        if self.is_16bit {
            let mut i = 0;
            while i + 1 < bytes.len() {
                let code = ((bytes[i] as u32) << 8) | (bytes[i + 1] as u32);
                if let Some(s) = self.code_to_unicode.get(&code) {
                    out.push_str(s);
                } else if code <= 255 && is_printable_char(code as u8) {
                    out.push((code as u8) as char);
                }
                i += 2;
            }
            if i < bytes.len() {
                let code = bytes[i] as u32;
                if let Some(s) = self.code_to_unicode.get(&code) {
                    out.push_str(s);
                } else if is_printable_char(bytes[i]) {
                    out.push(bytes[i] as char);
                }
            }
        } else {
            for &b in bytes {
                let code = b as u32;
                if let Some(s) = self.code_to_unicode.get(&code) {
                    out.push_str(s);
                } else if is_printable_char(b) {
                    out.push(b as char);
                }
            }
        }
        out
    }
}

fn is_printable_char(b: u8) -> bool {
    b == b'\t' || b == b'\n' || b == b'\r' || (0x20..=0x7E).contains(&b) || b >= 0xA0
}

/// Font encoding details extracted from a PDF /Font dictionary conforming to ISO 32000-1 Clause 9.5-9.7.
#[derive(Debug, Clone, Default)]
pub struct FontEncodingInfo {
    pub base_font: String,
    pub subtype: String,
    pub to_unicode: Option<CMap>,
    pub encoding_name: Option<String>,
    pub differences: HashMap<u8, String>,
}

impl FontEncodingInfo {
    /// Extracts font encoding and CMap from a PDF font dictionary.
    pub fn from_dict(doc: &PdfDocument, font_dict: &PdfDictionary) -> Self {
        let base_font = font_dict.get_name("BaseFont").unwrap_or("").to_string();
        let subtype = font_dict.get_name("Subtype").unwrap_or("").to_string();
        let encoding_name = font_dict.get_name("Encoding").map(|s| s.to_string());

        // Check for /ToUnicode CMap stream
        let mut to_unicode = None;
        if let Some(to_uni_obj) = font_dict.get("ToUnicode") {
            let stream_data = match to_uni_obj {
                Object::Reference(id) => doc
                    .get_object(*id)
                    .and_then(|o| o.as_stream())
                    .and_then(|s| s.decode().ok()),
                Object::Stream(s) => s.decode().ok(),
                _ => None,
            };
            if let Some(data) = stream_data {
                to_unicode = Some(CMap::parse(&data));
            }
        }

        // If Type0, also inspect /DescendantFonts
        if to_unicode.is_none() && subtype == "Type0" {
            if let Some(desc_obj) = font_dict.get("DescendantFonts") {
                let desc_arr = match desc_obj {
                    Object::Array(arr) => Some(arr.as_slice()),
                    Object::Reference(id) => doc.get_object(*id).and_then(Object::as_array),
                    _ => None,
                };
                if let Some(arr) = desc_arr {
                    if let Some(first_desc) = arr.first() {
                        let desc_dict = match first_desc {
                            Object::Dictionary(d) => Some(d),
                            Object::Reference(id) => doc.get_object(*id).and_then(Object::as_dict),
                            _ => None,
                        };
                        if let Some(d) = desc_dict {
                            if let Some(to_uni_obj) = d.get("ToUnicode") {
                                let stream_data = match to_uni_obj {
                                    Object::Reference(id) => doc
                                        .get_object(*id)
                                        .and_then(|o| o.as_stream())
                                        .and_then(|s| s.decode().ok()),
                                    Object::Stream(s) => s.decode().ok(),
                                    _ => None,
                                };
                                if let Some(data) = stream_data {
                                    to_unicode = Some(CMap::parse(&data));
                                }
                            }
                        }
                    }
                }
            }
        }

        // Parse /Differences if present in /Encoding dictionary
        let mut differences = HashMap::new();
        if let Some(enc_obj) = font_dict.get("Encoding") {
            let enc_dict = match enc_obj {
                Object::Dictionary(d) => Some(d),
                Object::Reference(id) => doc.get_object(*id).and_then(Object::as_dict),
                _ => None,
            };
            if let Some(d) = enc_dict {
                if let Some(diff_arr) = d.get_array("Differences") {
                    let mut cur_code: Option<u8> = None;
                    for item in diff_arr {
                        match item {
                            Object::Integer(i) => {
                                cur_code = Some((*i & 0xFF) as u8);
                            }
                            Object::Name(n) => {
                                if let Some(code) = cur_code {
                                    differences.insert(code, n.as_str().to_string());
                                    cur_code = code.checked_add(1);
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
        }

        FontEncodingInfo {
            base_font,
            subtype,
            to_unicode,
            encoding_name,
            differences,
        }
    }

    /// Decodes character codes in `bytes` into a Unicode string.
    pub fn decode_string(&self, bytes: &[u8]) -> String {
        // 1. If /ToUnicode CMap is available, use it (ISO 32000-1 Clause 9.10)
        if let Some(ref cmap) = self.to_unicode {
            let res = cmap.decode_bytes(bytes);
            if !res.is_empty() {
                return res;
            }
        }

        // 2. Check for UTF-16BE BOM (\xFE\xFF)
        if bytes.len() >= 2 && bytes[0] == 0xFE && bytes[1] == 0xFF {
            let u16_chars: Vec<u16> = bytes[2..]
                .chunks_exact(2)
                .map(|chunk| ((chunk[0] as u16) << 8) | (chunk[1] as u16))
                .collect();
            return String::from_utf16_lossy(&u16_chars);
        }

        // 3. Check for raw UTF-16BE without BOM
        if bytes.len() >= 4 && bytes.len() % 2 == 0 && bytes[0] == 0x00 && bytes[2] == 0x00 {
            let u16_chars: Vec<u16> = bytes
                .chunks_exact(2)
                .map(|chunk| ((chunk[0] as u16) << 8) | (chunk[1] as u16))
                .collect();
            let s = String::from_utf16_lossy(&u16_chars);
            if s.chars().all(|c| !c.is_control() || c == '\t' || c == '\n' || c == '\r') {
                return s;
            }
        }

        // 4. Custom /Differences array
        if !self.differences.is_empty() {
            let mut out = String::with_capacity(bytes.len());
            for &b in bytes {
                if let Some(glyph_name) = self.differences.get(&b) {
                    if let Some(ch) = glyph_name_to_unicode(glyph_name) {
                        out.push(ch);
                    } else if (0x20..=0x7E).contains(&b) {
                        out.push(b as char);
                    }
                } else if let Some(ref enc) = self.encoding_name {
                    if enc == "WinAnsiEncoding" {
                        out.push(winansi_byte_to_char(b));
                    } else if (0x20..=0x7E).contains(&b) {
                        out.push(b as char);
                    }
                } else if (0x20..=0x7E).contains(&b) {
                    out.push(b as char);
                }
            }
            return out;
        }

        // 5. WinAnsiEncoding (Windows-1252)
        if let Some(ref enc) = self.encoding_name {
            if enc == "WinAnsiEncoding" {
                return decode_winansi(bytes);
            }
        }

        // 6. Fallback
        match std::str::from_utf8(bytes) {
            Ok(s) => s.to_string(),
            Err(_) => decode_winansi(bytes),
        }
    }

    /// Extracts all fonts declared in a resource dictionary, keyed by both "/F1" and "F1".
    pub fn load_page_fonts(
        doc: &PdfDocument,
        resources: Option<&PdfDictionary>,
    ) -> HashMap<String, FontEncodingInfo> {
        let mut fonts = HashMap::new();
        if let Some(res) = resources {
            let font_dict = match res.get("Font") {
                Some(Object::Dictionary(d)) => Some(d),
                Some(Object::Reference(id)) => doc.get_object(*id).and_then(Object::as_dict),
                _ => None,
            };
            if let Some(fd) = font_dict {
                for (name, val) in &fd.map {
                    let single_font_dict = match val {
                        Object::Dictionary(d) => Some(d),
                        Object::Reference(id) => doc.get_object(*id).and_then(Object::as_dict),
                        _ => None,
                    };
                    if let Some(sfd) = single_font_dict {
                        let info = FontEncodingInfo::from_dict(doc, sfd);
                        let key = name.as_str().to_string();
                        let unslashed = key.strip_prefix('/').unwrap_or(&key).to_string();
                        fonts.insert(key, info.clone());
                        fonts.insert(unslashed, info);
                    }
                }
            }
        }
        fonts
    }
}

/// Decodes bytes via Windows-1252 (WinAnsiEncoding).
pub fn decode_winansi(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len());
    for &b in bytes {
        s.push(winansi_byte_to_char(b));
    }
    s
}

fn winansi_byte_to_char(b: u8) -> char {
    match b {
        0x00..=0x1F => {
            if b == b'\t' {
                '\t'
            } else if b == b'\n' {
                '\n'
            } else if b == b'\r' {
                '\r'
            } else {
                ' '
            }
        }
        0x20..=0x7E => b as char,
        0x7F => ' ',
        0x80 => '€',
        0x82 => '‚',
        0x83 => 'ƒ',
        0x84 => '„',
        0x85 => '…',
        0x86 => '†',
        0x87 => '‡',
        0x88 => 'ˆ',
        0x89 => '‰',
        0x8A => 'Š',
        0x8B => '‹',
        0x8C => 'Œ',
        0x8E => 'Ž',
        0x91 => '‘',
        0x92 => '’',
        0x93 => '“',
        0x94 => '”',
        0x95 => '•',
        0x96 => '–',
        0x97 => '—',
        0x98 => '˜',
        0x99 => '™',
        0x9A => 'š',
        0x9B => '›',
        0x9C => 'œ',
        0x9E => 'ž',
        0x9F => 'Ÿ',
        0xA0..=0xFF => b as char,
        _ => ' ',
    }
}

fn glyph_name_to_unicode(name: &str) -> Option<char> {
    if name.len() == 1 {
        return name.chars().next();
    }
    if let Some(stripped) = name.strip_prefix("uni") {
        if let Ok(val) = u32::from_str_radix(stripped, 16) {
            return char::from_u32(val);
        }
    }
    if let Some(stripped) = name.strip_prefix('u') {
        if let Ok(val) = u32::from_str_radix(stripped, 16) {
            return char::from_u32(val);
        }
    }

    match name {
        "space" => Some(' '),
        "exclam" => Some('!'),
        "quotedbl" => Some('"'),
        "numbersign" => Some('#'),
        "dollar" => Some('$'),
        "percent" => Some('%'),
        "ampersand" => Some('&'),
        "quotesingle" => Some('\''),
        "parenleft" => Some('('),
        "parenright" => Some(')'),
        "asterisk" => Some('*'),
        "plus" => Some('+'),
        "comma" => Some(','),
        "hyphen" | "minus" => Some('-'),
        "period" => Some('.'),
        "slash" => Some('/'),
        "zero" => Some('0'),
        "one" => Some('1'),
        "two" => Some('2'),
        "three" => Some('3'),
        "four" => Some('4'),
        "five" => Some('5'),
        "six" => Some('6'),
        "seven" => Some('7'),
        "eight" => Some('8'),
        "nine" => Some('9'),
        "colon" => Some(':'),
        "semicolon" => Some(';'),
        "less" => Some('<'),
        "equal" => Some('='),
        "greater" => Some('>'),
        "question" => Some('?'),
        "at" => Some('@'),
        "bracketleft" => Some('['),
        "backslash" => Some('\\'),
        "bracketright" => Some(']'),
        "asciicircum" => Some('^'),
        "underscore" => Some('_'),
        "grave" => Some('`'),
        "braceleft" => Some('{'),
        "bar" => Some('|'),
        "braceright" => Some('}'),
        "asciitilde" => Some('~'),
        "quoteleft" => Some('‘'),
        "quoteright" => Some('’'),
        "quotedblleft" => Some('“'),
        "quotedblright" => Some('”'),
        "bullet" => Some('•'),
        "endash" => Some('–'),
        "emdash" => Some('—'),
        "ellipsis" => Some('…'),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cmap_bfchar_and_bfrange() {
        let cmap_sample = br#"
/CIDInit /ProcSet findresource begin
12 dict begin
begincmap
1 begincodespacerange
<0000> <FFFF>
endcodespacerange
2 beginbfchar
<0001> <0041>
<01B1> <00660069>
endbfchar
1 beginbfrange
<0061> <0063> <004D>
endbfrange
1 beginbfrange
<03DF> <03E0> [<002E> <002C>]
endbfrange
endcmap
"#;

        let cmap = CMap::parse(cmap_sample);
        assert_eq!(cmap.code_to_unicode.get(&0x0001).unwrap(), "A");
        assert_eq!(cmap.code_to_unicode.get(&0x01B1).unwrap(), "fi");
        assert_eq!(cmap.code_to_unicode.get(&0x0061).unwrap(), "M");
        assert_eq!(cmap.code_to_unicode.get(&0x0062).unwrap(), "N");
        assert_eq!(cmap.code_to_unicode.get(&0x0063).unwrap(), "O");
        assert_eq!(cmap.code_to_unicode.get(&0x03DF).unwrap(), ".");
        assert_eq!(cmap.code_to_unicode.get(&0x03E0).unwrap(), ",");

        // Test decoding 16-bit bytes
        let bytes = vec![0x00, 0x01, 0x00, 0x61, 0x03, 0xDF];
        assert_eq!(cmap.decode_bytes(&bytes), "AM.");
    }

    #[test]
    fn test_winansi_decoding() {
        let bytes = vec![b'H', b'e', b'l', b'l', b'o', 0x93, b'W', 0x94];
        assert_eq!(decode_winansi(&bytes), "Hello“W”");
    }
}
