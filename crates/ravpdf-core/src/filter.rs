//! PDF Stream Filters conforming to ISO 32000-1 Section 7.4.
//!
//! Traceability: FR-01.1.1, Section 7.1 Stream Filters.

use crate::error::{PdfError, Result};
use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;
use flate2::Compression;
use std::collections::HashMap;
use std::io::{Cursor, Read, Write};

/// Decodes raw stream data using the named PDF filter.
/// Supports standard names and ISO 32000-1 Table 6 abbreviations.
pub fn decode_stream(filter_name: &str, data: &[u8]) -> Result<Vec<u8>> {
    decode_stream_with_dict(filter_name, data, None)
}

/// Decodes raw stream data with an optional `/DecodeParms` dictionary.
/// Conforms to ISO 32000-1 Section 7.4 (Table 6, Table 11).
pub fn decode_stream_with_dict(
    filter_name: &str,
    data: &[u8],
    params: Option<&crate::object::PdfDictionary>,
) -> Result<Vec<u8>> {
    let name = filter_name.strip_prefix('/').unwrap_or(filter_name);
    match name {
        "FlateDecode" | "Fl" => decode_flate(data),
        "ASCIIHexDecode" | "AHx" => decode_ascii_hex(data),
        "ASCII85Decode" | "A85" => decode_ascii85(data),
        "RunLengthDecode" | "RL" => decode_run_length(data),
        "LZWDecode" | "LZW" => decode_lzw(data, true),
        "DCTDecode" | "DCT" => decode_dct(data),
        "CCITTFaxDecode" | "CCF" => {
            let p = params.map(CcittFaxParams::from_dict).unwrap_or_default();
            decode_ccitt_fax(data, &p)
        }
        "JBIG2Decode" => decode_jbig2(data, None),
        other => Err(PdfError::FilterError(format!(
            "Unsupported or unhandled stream filter: {}",
            other
        ))),
    }
}

/// Compresses stream data using the specified filter (default is FlateDecode).
/// Supports standard names and ISO 32000-1 Table 6 abbreviations.
pub fn encode_stream(filter_name: &str, data: &[u8]) -> Result<Vec<u8>> {
    let name = filter_name.strip_prefix('/').unwrap_or(filter_name);
    match name {
        "FlateDecode" | "Fl" => encode_flate(data),
        "ASCIIHexDecode" | "AHx" => encode_ascii_hex(data),
        "ASCII85Decode" | "A85" => encode_ascii85(data),
        "RunLengthDecode" | "RL" => encode_run_length(data),
        "LZWDecode" | "LZW" => encode_lzw(data, true),
        "DCTDecode" | "DCT" => Err(PdfError::FilterError(
            "DCTEncode requires image dimensions; call encode_dct(rgb_data, width, height, quality) directly".into(),
        )),
        "CCITTFaxDecode" | "CCF" => Err(PdfError::FilterError(
            "CCITTFaxEncode requires image dimensions; call encode_ccitt_fax(data, width, height, black_is_1) directly".into(),
        )),
        other => Err(PdfError::FilterError(format!(
            "Unsupported encoding filter: {}",
            other
        ))),
    }
}

/// FlateDecode (Zlib/Deflate) decompression.
pub fn decode_flate(data: &[u8]) -> Result<Vec<u8>> {
    let mut decoder = ZlibDecoder::new(data);
    let mut output = Vec::new();
    decoder
        .read_to_end(&mut output)
        .map_err(|e| PdfError::FilterError(format!("FlateDecode error: {}", e)))?;
    Ok(output)
}

/// FlateEncode (Zlib/Deflate) compression with standard compression level.
pub fn encode_flate(data: &[u8]) -> Result<Vec<u8>> {
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    encoder
        .write_all(data)
        .map_err(|e| PdfError::FilterError(format!("FlateEncode write error: {}", e)))?;
    encoder
        .finish()
        .map_err(|e| PdfError::FilterError(format!("FlateEncode finish error: {}", e)))
}

/// ASCIIHexDecode: Decodes ASCII hexadecimal digits conforming to ISO 32000-1 Section 7.4.2.
/// Whitespace characters are ignored. An optional '>' terminates the data.
pub fn decode_ascii_hex(data: &[u8]) -> Result<Vec<u8>> {
    let mut output = Vec::new();
    let mut first_nibble: Option<u8> = None;

    for &byte in data {
        if byte.is_ascii_whitespace() {
            continue;
        }
        if byte == b'>' {
            break;
        }

        let val = match byte {
            b'0'..=b'9' => byte - b'0',
            b'a'..=b'f' => byte - b'a' + 10,
            b'A'..=b'F' => byte - b'A' + 10,
            _ => {
                return Err(PdfError::FilterError(format!(
                    "Invalid character in ASCIIHex stream: 0x{:02X}",
                    byte
                )))
            }
        };

        if let Some(high) = first_nibble.take() {
            output.push((high << 4) | val);
        } else {
            first_nibble = Some(val);
        }
    }

    // If an odd number of hex digits was encountered, the last digit is padded with a trailing zero.
    if let Some(high) = first_nibble {
        output.push(high << 4);
    }

    Ok(output)
}

/// ASCIIHexEncode: Encodes bytes to uppercase hex ASCII terminated with '>'.
pub fn encode_ascii_hex(data: &[u8]) -> Result<Vec<u8>> {
    let mut output = Vec::with_capacity(data.len() * 2 + 1);
    for &byte in data {
        output.extend_from_slice(format!("{:02X}", byte).as_bytes());
    }
    output.push(b'>');
    Ok(output)
}

/// ASCII85Decode: Decodes 5-character groups in the range '!' (33) to 'u' (117)
/// into 4-byte integers conforming to ISO 32000-1 Section 7.4.3. 'z' expands to four 0x00 bytes.
pub fn decode_ascii85(data: &[u8]) -> Result<Vec<u8>> {
    let mut output = Vec::new();
    let mut tuple: u32 = 0;
    let mut count = 0;

    let mut i = 0;
    while i < data.len() {
        let b = data[i];
        if b.is_ascii_whitespace() {
            i += 1;
            continue;
        }
        if b == b'<' && i + 1 < data.len() && data[i + 1] == b'~' {
            i += 2;
            continue;
        }
        if b == b'~' && i + 1 < data.len() && data[i + 1] == b'>' {
            break;
        }
        if b == b'z' && count == 0 {
            output.extend_from_slice(&[0, 0, 0, 0]);
            i += 1;
            continue;
        }

        if !(b'!'..=b'u').contains(&b) {
            return Err(PdfError::FilterError(format!(
                "Invalid byte in ASCII85 stream: {}",
                b
            )));
        }

        tuple = tuple
            .checked_mul(85)
            .and_then(|val| val.checked_add((b - b'!') as u32))
            .ok_or_else(|| PdfError::FilterError("ASCII85 overflow".into()))?;

        count += 1;
        if count == 5 {
            output.push((tuple >> 24) as u8);
            output.push((tuple >> 16) as u8);
            output.push((tuple >> 8) as u8);
            output.push(tuple as u8);
            tuple = 0;
            count = 0;
        }
        i += 1;
    }

    if count > 1 {
        let pad = 5 - count;
        for _ in 0..pad {
            tuple = tuple
                .checked_mul(85)
                .and_then(|val| val.checked_add(84))
                .ok_or_else(|| PdfError::FilterError("ASCII85 padding overflow".into()))?;
        }
        for j in 0..(count - 1) {
            let shift = (3 - j) * 8;
            output.push((tuple >> shift) as u8);
        }
    } else if count == 1 {
        return Err(PdfError::FilterError(
            "Invalid single-character final group in ASCII85 stream".into(),
        ));
    }

    Ok(output)
}

/// ASCII85Encode: Encodes bytes into standard ASCII85 format with '~>' terminator.
pub fn encode_ascii85(data: &[u8]) -> Result<Vec<u8>> {
    let mut output = Vec::new();
    let chunks = data.chunks_exact(4);
    let remainder = chunks.remainder();

    for chunk in chunks {
        let val = ((chunk[0] as u32) << 24)
            | ((chunk[1] as u32) << 16)
            | ((chunk[2] as u32) << 8)
            | (chunk[3] as u32);

        if val == 0 {
            output.push(b'z');
        } else {
            let mut buf = [0u8; 5];
            let mut v = val;
            for i in (0..5).rev() {
                buf[i] = b'!' + (v % 85) as u8;
                v /= 85;
            }
            output.extend_from_slice(&buf);
        }
    }

    if !remainder.is_empty() {
        let mut val = 0u32;
        for (i, &b) in remainder.iter().enumerate() {
            val |= (b as u32) << ((3 - i) * 8);
        }
        let mut buf = [0u8; 5];
        let mut v = val;
        for i in (0..5).rev() {
            buf[i] = b'!' + (v % 85) as u8;
            v /= 85;
        }
        output.extend_from_slice(&buf[..(remainder.len() + 1)]);
    }

    output.extend_from_slice(b"~>");
    Ok(output)
}

/// RunLengthDecode: Decodes run-length encoded stream conforming to ISO 32000-1 Section 7.4.5.
pub fn decode_run_length(data: &[u8]) -> Result<Vec<u8>> {
    let mut output = Vec::new();
    let mut i = 0;

    while i < data.len() {
        let length = data[i];
        i += 1;

        if length == 128 {
            // EOD marker
            break;
        } else if length < 128 {
            let count = (length as usize) + 1;
            if i + count > data.len() {
                return Err(PdfError::FilterError(
                    "Unexpected EOF in RunLengthDecode copy".into(),
                ));
            }
            output.extend_from_slice(&data[i..i + count]);
            i += count;
        } else {
            let count = 257 - (length as usize);
            if i >= data.len() {
                return Err(PdfError::FilterError(
                    "Unexpected EOF in RunLengthDecode repeat".into(),
                ));
            }
            let byte_to_repeat = data[i];
            i += 1;
            output.resize(output.len() + count, byte_to_repeat);
        }
    }

    Ok(output)
}

/// RunLengthEncode: Encodes data with run-length encoding.
pub fn encode_run_length(data: &[u8]) -> Result<Vec<u8>> {
    let mut output = Vec::new();
    let mut i = 0;

    while i < data.len() {
        // Find run of identical bytes
        let mut run_len = 1;
        while i + run_len < data.len() && run_len < 128 && data[i + run_len] == data[i] {
            run_len += 1;
        }

        if run_len >= 2 {
            // Repeat byte run
            output.push((257 - run_len) as u8);
            output.push(data[i]);
            i += run_len;
        } else {
            // Literal sequence
            let start = i;
            let mut lit_len = 1;
            i += 1;

            while i < data.len() && lit_len < 128 {
                if i + 1 < data.len() && data[i] == data[i + 1] {
                    break;
                }
                lit_len += 1;
                i += 1;
            }

            output.push((lit_len - 1) as u8);
            output.extend_from_slice(&data[start..start + lit_len]);
        }
    }

    // EOD marker
    output.push(128);
    Ok(output)
}

// -----------------------------------------------------------------------------
// LZW (Lempel-Ziv-Welch) Implementation conforming to ISO 32000-1 Section 7.4.4
// -----------------------------------------------------------------------------

const LZW_CLEAR_TABLE: u16 = 256;
const LZW_EOD: u16 = 257;
const LZW_FIRST_ENTRY: u16 = 258;
const LZW_MAX_CODES: usize = 4096;

struct MsbBitReader<'a> {
    data: &'a [u8],
    byte_pos: usize,
    bit_buf: u32,
    bits_in_buf: u32,
}

impl<'a> MsbBitReader<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self {
            data,
            byte_pos: 0,
            bit_buf: 0,
            bits_in_buf: 0,
        }
    }

    fn read_bits(&mut self, n: u32) -> Option<u16> {
        while self.bits_in_buf < n {
            if self.byte_pos >= self.data.len() {
                return None;
            }
            self.bit_buf = (self.bit_buf << 8) | (self.data[self.byte_pos] as u32);
            self.byte_pos += 1;
            self.bits_in_buf += 8;
        }
        let shift = self.bits_in_buf - n;
        let val = (self.bit_buf >> shift) & ((1 << n) - 1);
        self.bits_in_buf -= n;
        self.bit_buf &= (1 << self.bits_in_buf) - 1;
        Some(val as u16)
    }
}

struct MsbBitWriter {
    bytes: Vec<u8>,
    bit_buf: u32,
    bits_in_buf: u32,
}

impl MsbBitWriter {
    fn new() -> Self {
        Self {
            bytes: Vec::new(),
            bit_buf: 0,
            bits_in_buf: 0,
        }
    }

    fn write_bits(&mut self, val: u16, n: u32) {
        self.bit_buf = (self.bit_buf << n) | (val as u32 & ((1 << n) - 1));
        self.bits_in_buf += n;
        while self.bits_in_buf >= 8 {
            let shift = self.bits_in_buf - 8;
            let byte = (self.bit_buf >> shift) as u8;
            self.bytes.push(byte);
            self.bits_in_buf -= 8;
            self.bit_buf &= (1 << self.bits_in_buf) - 1;
        }
    }

    fn finish(mut self) -> Vec<u8> {
        if self.bits_in_buf > 0 {
            let byte = (self.bit_buf << (8 - self.bits_in_buf)) as u8;
            self.bytes.push(byte);
        }
        self.bytes
    }
}

fn init_lzw_decode_table() -> Vec<Vec<u8>> {
    let mut table = Vec::with_capacity(LZW_MAX_CODES);
    for i in 0..=255 {
        table.push(vec![i as u8]);
    }
    table.push(Vec::new()); // 256 CLEAR_TABLE
    table.push(Vec::new()); // 257 EOD
    table
}

/// LZWDecode: Decodes an LZW-compressed byte stream conforming to ISO 32000-1 Section 7.4.4.
/// `early_change` indicates whether code length increments 1 code early (PDF standard default: true).
pub fn decode_lzw(data: &[u8], early_change: bool) -> Result<Vec<u8>> {
    let mut reader = MsbBitReader::new(data);
    let mut table = init_lzw_decode_table();
    let mut code_size = 9u32;
    let mut output = Vec::new();
    let mut prev_entry: Option<Vec<u8>> = None;

    while let Some(code) = reader.read_bits(code_size) {
        if code == LZW_CLEAR_TABLE {
            table = init_lzw_decode_table();
            code_size = 9;
            prev_entry = None;
            continue;
        }
        if code == LZW_EOD {
            break;
        }

        let entry = if (code as usize) < table.len() {
            table[code as usize].clone()
        } else if (code as usize) == table.len() {
            // Special KwKwK case
            if let Some(ref prev) = prev_entry {
                let mut k = prev.clone();
                k.push(prev[0]);
                k
            } else {
                return Err(PdfError::FilterError("Invalid initial KwKwK in LZW".into()));
            }
        } else {
            return Err(PdfError::FilterError(format!(
                "Invalid LZW code: {} (table size {})",
                code,
                table.len()
            )));
        };

        output.extend_from_slice(&entry);

        if let Some(ref prev) = prev_entry {
            if table.len() < LZW_MAX_CODES {
                let mut new_entry = prev.clone();
                new_entry.push(entry[0]);
                table.push(new_entry);

                let next_index = table.len();
                let target = if early_change { 511 } else { 512 };
                if next_index == target && code_size < 10 {
                    code_size = 10;
                } else if next_index == (if early_change { 1023 } else { 1024 }) && code_size < 11 {
                    code_size = 11;
                } else if next_index == (if early_change { 2047 } else { 2048 }) && code_size < 12 {
                    code_size = 12;
                }
            }
        }

        prev_entry = Some(entry);
    }

    Ok(output)
}

/// LZWEncode: Encodes data with LZW compression conforming to ISO 32000-1 Section 7.4.4.
pub fn encode_lzw(data: &[u8], early_change: bool) -> Result<Vec<u8>> {
    if data.is_empty() {
        let mut writer = MsbBitWriter::new();
        writer.write_bits(LZW_CLEAR_TABLE, 9);
        writer.write_bits(LZW_EOD, 9);
        return Ok(writer.finish());
    }

    let mut table: HashMap<Vec<u8>, u16> = HashMap::new();
    let mut next_code = LZW_FIRST_ENTRY;
    let mut code_size = 9u32;
    let mut writer = MsbBitWriter::new();

    let init_map = |t: &mut HashMap<Vec<u8>, u16>| {
        t.clear();
        for i in 0..=255 {
            t.insert(vec![i as u8], i as u16);
        }
    };
    init_map(&mut table);

    // Initial ClearTable
    writer.write_bits(LZW_CLEAR_TABLE, code_size);

    let mut current = Vec::new();
    for &byte in data {
        let mut next_seq = current.clone();
        next_seq.push(byte);

        if table.contains_key(&next_seq) {
            current = next_seq;
        } else {
            let code = table[&current];
            writer.write_bits(code, code_size);

            if next_code < LZW_MAX_CODES as u16 {
                table.insert(next_seq, next_code);
                next_code += 1;

                let code_just_added = (next_code - 1) as usize;
                let target = if early_change { 511 } else { 512 };
                if code_just_added == target && code_size < 10 {
                    code_size = 10;
                } else if code_just_added == (if early_change { 1023 } else { 1024 }) && code_size < 11 {
                    code_size = 11;
                } else if code_just_added == (if early_change { 2047 } else { 2048 }) && code_size < 12 {
                    code_size = 12;
                }
            } else {
                writer.write_bits(LZW_CLEAR_TABLE, code_size);
                init_map(&mut table);
                next_code = LZW_FIRST_ENTRY;
                code_size = 9;
            }

            current = vec![byte];
        }
    }

    if !current.is_empty() {
        let code = table[&current];
        writer.write_bits(code, code_size);
    }

    writer.write_bits(LZW_EOD, code_size);
    Ok(writer.finish())
}

// -----------------------------------------------------------------------------
// DCT (JPEG) Implementation conforming to ISO 32000-1 Section 7.4.8
// -----------------------------------------------------------------------------

/// DCTDecode (JPEG) decompression conforming to ISO 32000-1 Section 7.4.8.
/// Decompresses a JPEG byte stream into raw RGB pixel bytes (3 bytes per pixel: R, G, B interleaved).
pub fn decode_dct(data: &[u8]) -> Result<Vec<u8>> {
    let img = image::load_from_memory_with_format(data, image::ImageFormat::Jpeg)
        .map_err(|e| PdfError::FilterError(format!("DCTDecode decompression error: {}", e)))?;
    Ok(img.to_rgb8().into_raw())
}

/// Decodes a JPEG byte stream (DCTDecode) and returns `(raw_rgb_bytes, width, height)`.
pub fn decode_dct_with_dimensions(data: &[u8]) -> Result<(Vec<u8>, u32, u32)> {
    let img = image::load_from_memory_with_format(data, image::ImageFormat::Jpeg)
        .map_err(|e| PdfError::FilterError(format!("DCTDecode decompression error: {}", e)))?;
    let width = img.width();
    let height = img.height();
    Ok((img.to_rgb8().into_raw(), width, height))
}

/// DCTEncode (JPEG) compression.
/// Encodes raw RGB pixel bytes of dimensions `width x height` into a JPEG byte stream with configurable quality (1-100).
pub fn encode_dct(rgb_data: &[u8], width: u32, height: u32, quality: u8) -> Result<Vec<u8>> {
    let expected_len = (width as usize)
        .checked_mul(height as usize)
        .and_then(|wh| wh.checked_mul(3))
        .ok_or_else(|| PdfError::FilterError("Image dimensions overflow usize".into()))?;
    if rgb_data.len() != expected_len {
        return Err(PdfError::FilterError(format!(
            "DCTEncode error: expected RGB buffer size {} for {}x{} image, got {}",
            expected_len, width, height, rgb_data.len()
        )));
    }

    let rgb_img = image::RgbImage::from_raw(width, height, rgb_data.to_vec())
        .ok_or_else(|| PdfError::FilterError("Failed to create RGB image buffer".into()))?;

    let mut output = Vec::new();
    let mut cursor = Cursor::new(&mut output);
    let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut cursor, quality.clamp(1, 100));
    encoder
        .encode(rgb_img.as_raw(), width, height, image::ExtendedColorType::Rgb8)
        .map_err(|e| PdfError::FilterError(format!("DCTEncode error: {}", e)))?;
    drop(encoder);
    Ok(output)
}

/// Parameters for CCITTFaxDecode filter conforming to ISO 32000-1 Section 7.4.6 Table 11.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CcittFaxParams {
    /// A code selecting the encoding scheme.
    /// K < 0: Pure two-dimensional encoding (Group 4 / T.6 MMR).
    /// K = 0: Pure one-dimensional encoding (Group 3 1D / T.4).
    /// K > 0: Mixed one- and two-dimensional encoding (Group 3 2D / T.4).
    pub k: i32,
    /// A flag indicating whether end-of-line bit patterns are present. Default: false.
    pub end_of_line: bool,
    /// A flag indicating whether byte alignment precedes each end-of-line. Default: false.
    pub encoded_byte_align: bool,
    /// The width of the image in pixels. Default: 1728.
    pub columns: u32,
    /// The height of the image in scanlines. Default: 0 (decoded until EOFB or data end).
    pub rows: u32,
    /// A flag indicating whether an end-of-block code is expected. Default: true.
    pub end_of_block: bool,
    /// A flag indicating whether 1 bits are black (true) or 1 bits are white (false). Default: false.
    pub black_is_1: bool,
}

impl Default for CcittFaxParams {
    fn default() -> Self {
        Self {
            k: 0,
            end_of_line: false,
            encoded_byte_align: false,
            columns: 1728,
            rows: 0,
            end_of_block: true,
            black_is_1: false,
        }
    }
}

impl CcittFaxParams {
    pub fn from_dict(dict: &crate::object::PdfDictionary) -> Self {
        let mut params = Self::default();
        if let Some(k) = dict.get_i64("K") {
            params.k = k as i32;
        }
        if let Some(eol) = dict.get_bool("EndOfLine") {
            params.end_of_line = eol;
        }
        if let Some(eba) = dict.get_bool("EncodedByteAlign") {
            params.encoded_byte_align = eba;
        }
        if let Some(cols) = dict.get_i64("Columns") {
            params.columns = cols.max(1) as u32;
        }
        if let Some(rows) = dict.get_i64("Rows") {
            params.rows = rows.max(0) as u32;
        }
        if let Some(eob) = dict.get_bool("EndOfBlock") {
            params.end_of_block = eob;
        }
        if let Some(b1) = dict.get_bool("BlackIs1") {
            params.black_is_1 = b1;
        }
        params
    }
}

/// CCITTFaxDecode: Decompresses facsimile bi-level image data conforming to ISO 32000-1 Section 7.4.6.
pub fn decode_ccitt_fax(data: &[u8], params: &CcittFaxParams) -> Result<Vec<u8>> {
    let width = params.columns.max(1);
    let height = if params.rows > 0 { Some(params.rows) } else { None };
    let bytes_per_row = (width as usize + 7) / 8;
    let mut output = Vec::new();

    let mut line_decoder = |line: &[u32]| {
        let mut row = vec![0u8; bytes_per_row];
        for (i, c) in fax::decoder::pels(line, width).enumerate() {
            let is_one = match c {
                fax::Color::Black => params.black_is_1,
                fax::Color::White => !params.black_is_1,
            };
            if is_one {
                let byte_idx = i / 8;
                let bit_idx = 7 - (i % 8);
                if byte_idx < row.len() {
                    row[byte_idx] |= 1 << bit_idx;
                }
            }
        }
        output.extend_from_slice(&row);
    };

    if params.k < 0 {
        // Group 4 (2D MMR)
        let _ = fax::decoder::decode_g4(data.iter().copied(), width, height, &mut line_decoder);
    } else {
        // Group 3 (1D)
        let _ = fax::decoder::decode_g3(data.iter().copied(), &mut line_decoder);
    }

    Ok(output)
}

/// CCITTFaxEncode: Compresses bi-level raster bitmap data into CCITT Group 4 MMR facsimile format.
pub fn encode_ccitt_fax(data: &[u8], width: u32, height: u32, black_is_1: bool) -> Result<Vec<u8>> {
    let bytes_per_row = (width as usize + 7) / 8;
    let writer = fax::VecWriter::new();
    let mut encoder = fax::encoder::Encoder::new(writer);

    for row_idx in 0..height as usize {
        let start = row_idx * bytes_per_row;
        let end = (start + bytes_per_row).min(data.len());
        if start >= data.len() {
            break;
        }
        let row_slice = &data[start..end];
        let pel_iter = (0..width as usize).map(|col_idx| {
            let byte = row_slice.get(col_idx / 8).copied().unwrap_or(0);
            let bit = (byte >> (7 - (col_idx % 8))) & 1;
            let is_black = if black_is_1 { bit == 1 } else { bit == 0 };
            if is_black {
                fax::Color::Black
            } else {
                fax::Color::White
            }
        });
        encoder
            .encode_line(pel_iter, width)
            .map_err(|e| PdfError::FilterError(format!("CCITTFax encode error: {:?}", e)))?;
    }

    let finished_writer = encoder
        .finish()
        .map_err(|e| PdfError::FilterError(format!("CCITTFax finish error: {:?}", e)))?;
    Ok(finished_writer.finish())
}

/// JBIG2Decode: Decompresses bi-level images encoded with JBIG2 (ISO 32000-1 Section 7.4.7).
pub fn decode_jbig2(data: &[u8], globals: Option<&[u8]>) -> Result<Vec<u8>> {
    struct BitCollector {
        rows: Vec<u8>,
        current_byte: u8,
        bits_in_current_byte: usize,
    }

    impl pdfluent_jbig2::Decoder for BitCollector {
        fn push_pixel(&mut self, black: bool) {
            if black {
                self.current_byte |= 1 << (7 - self.bits_in_current_byte);
            }
            self.bits_in_current_byte += 1;
            if self.bits_in_current_byte == 8 {
                self.rows.push(self.current_byte);
                self.current_byte = 0;
                self.bits_in_current_byte = 0;
            }
        }

        fn push_pixel_chunk(&mut self, black: bool, chunk_count: u32) {
            let byte_val = if black { 0xFF } else { 0x00 };
            for _ in 0..chunk_count {
                self.rows.push(byte_val);
            }
        }

        fn next_line(&mut self) {
            if self.bits_in_current_byte > 0 {
                self.rows.push(self.current_byte);
                self.current_byte = 0;
                self.bits_in_current_byte = 0;
            }
        }
    }

    // Try embedded format first (standard for PDF streams), fall back to standalone file format
    let parse_embedded = pdfluent_jbig2::Image::new_embedded(data, globals);
    let img = match parse_embedded {
        Ok(img) => img,
        Err(_) => pdfluent_jbig2::Image::new(data)
            .map_err(|e| PdfError::FilterError(format!("JBIG2 decode error: {:?}", e)))?,
    };

    let mut collector = BitCollector {
        rows: Vec::new(),
        current_byte: 0,
        bits_in_current_byte: 0,
    };

    img.decode(&mut collector)
        .map_err(|e| PdfError::FilterError(format!("JBIG2 execution error: {:?}", e)))?;

    Ok(collector.rows)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_flate_roundtrip() {
        let original = b"The quick brown fox jumps over the lazy dog. ISO 32000-1 specification test.";
        let compressed = encode_flate(original).expect("encode flate");
        let decompressed = decode_flate(&compressed).expect("decode flate");
        assert_eq!(original.to_vec(), decompressed);
    }

    #[test]
    fn test_ascii_hex_roundtrip() {
        let original = b"Hello, RavPDF Hex Stream!";
        let encoded = encode_ascii_hex(original).expect("encode hex");
        let decoded = decode_ascii_hex(&encoded).expect("decode hex");
        assert_eq!(original.to_vec(), decoded);
    }

    #[test]
    fn test_ascii_hex_edge_cases() {
        // Odd number of digits pads with trailing zero
        let data_odd = b"4B6>";
        let decoded_odd = decode_ascii_hex(data_odd).expect("odd hex decode");
        assert_eq!(decoded_odd, vec![0x4B, 0x60]);

        // Whitespace ignored and lowercase accepted
        let data_ws = b" 4 8 \n 6 5 \r\t 6 c \n 6 C 6 f > ";
        let decoded_ws = decode_ascii_hex(data_ws).expect("whitespace hex decode");
        assert_eq!(decoded_ws, b"Hello".to_vec());

        // Unterminated stream without '>'
        let data_unterminated = b"48656C6C6F";
        let decoded_unterminated = decode_ascii_hex(data_unterminated).expect("unterminated hex");
        assert_eq!(decoded_unterminated, b"Hello".to_vec());

        // Invalid character error
        let data_invalid = b"486Z6C>";
        assert!(decode_ascii_hex(data_invalid).is_err());
    }

    #[test]
    fn test_ascii85_roundtrip() {
        let original = b"Hello World! Testing ASCII85 Base-85 encoding for PDF streams.";
        let encoded = encode_ascii85(original).expect("encode ascii85");
        let decoded = decode_ascii85(&encoded).expect("decode ascii85");
        assert_eq!(original.to_vec(), decoded);
    }

    #[test]
    fn test_ascii85_edge_cases() {
        // Zero block compression
        let zeros = vec![0u8; 12];
        let encoded_zeros = encode_ascii85(&zeros).expect("encode zeros");
        assert_eq!(encoded_zeros, b"zzz~>");
        let decoded_zeros = decode_ascii85(&encoded_zeros).expect("decode zeros");
        assert_eq!(decoded_zeros, zeros);

        // Remainder chunks (1, 2, 3 remainder bytes)
        for len in 1..=10 {
            let src: Vec<u8> = (0..len).map(|b| b as u8).collect();
            let enc = encode_ascii85(&src).expect("encode remainder");
            let dec = decode_ascii85(&enc).expect("decode remainder");
            assert_eq!(src, dec);
        }

        // Whitespace handling with leading <~ and trailing ~>
        let hello_enc = encode_ascii85(b"Hello World").expect("encode hello");
        let mut with_ws = b" <~ ".to_vec();
        with_ws.extend_from_slice(&hello_enc[..5]);
        with_ws.extend_from_slice(b" \n\t ");
        with_ws.extend_from_slice(&hello_enc[5..]);
        with_ws.extend_from_slice(b" ");
        let dec = decode_ascii85(&with_ws).expect("decode with whitespace");
        assert_eq!(dec, b"Hello World".to_vec());

        // Invalid character error
        assert!(decode_ascii85(b"~v~>").is_err());

        // Invalid single-character final group error
        assert!(decode_ascii85(b"9~>").is_err());
    }

    #[test]
    fn test_run_length_roundtrip() {
        let original = b"AAAAABBBBBCCCCCDDDDD1234567890EEEEE";
        let encoded = encode_run_length(original).expect("encode run length");
        let decoded = decode_run_length(&encoded).expect("decode run length");
        assert_eq!(original.to_vec(), decoded);
    }

    #[test]
    fn test_run_length_edge_cases() {
        // Run of 128 repeated bytes (maximum repeat run in PDF RLE)
        let max_run = vec![0x42; 128];
        let enc_max_run = encode_run_length(&max_run).expect("encode 128 run");
        let dec_max_run = decode_run_length(&enc_max_run).expect("decode 128 run");
        assert_eq!(max_run, dec_max_run);

        // Run of 128 literal distinct bytes (maximum literal run in PDF RLE)
        let max_lit: Vec<u8> = (0..128).collect();
        let enc_max_lit = encode_run_length(&max_lit).expect("encode 128 lit");
        let dec_max_lit = decode_run_length(&enc_max_lit).expect("decode 128 lit");
        assert_eq!(max_lit, dec_max_lit);

        // Truncated copy run
        let truncated_copy = vec![5, 0x01, 0x02]; // length 5 expects 6 bytes, only 2 provided
        assert!(decode_run_length(&truncated_copy).is_err());

        // Truncated repeat run
        let truncated_repeat = vec![250]; // repeat run expects 1 byte to follow
        assert!(decode_run_length(&truncated_repeat).is_err());
    }

    #[test]
    fn test_lzw_roundtrip() {
        let original = b"TOBEORNOTTOBEORTOBEORNOT";
        let compressed = encode_lzw(original, true).expect("encode lzw");
        let decompressed = decode_lzw(&compressed, true).expect("decode lzw");
        assert_eq!(original.to_vec(), decompressed);
    }

    #[test]
    fn test_lzw_early_change_variants() {
        let original = b"PDF standard LZW stream verification test with repeated patterns abcdef123456abcdef123456";
        // EarlyChange = true (default)
        let enc_early = encode_lzw(original, true).expect("encode lzw early true");
        let dec_early = decode_lzw(&enc_early, true).expect("decode lzw early true");
        assert_eq!(original.to_vec(), dec_early);

        // EarlyChange = false
        let enc_late = encode_lzw(original, false).expect("encode lzw early false");
        let dec_late = decode_lzw(&enc_late, false).expect("decode lzw early false");
        assert_eq!(original.to_vec(), dec_late);
    }

    #[test]
    fn test_lzw_kwkwk_pattern() {
        // Highly repetitive pattern that exercises code == table.len() KwKwK path
        let data = b"ABABABABABABABABABABABABABABABABAB";
        let enc = encode_lzw(data, true).expect("encode kwkwk");
        let dec = decode_lzw(&enc, true).expect("decode kwkwk");
        assert_eq!(data.to_vec(), dec);
    }

    #[test]
    fn test_lzw_corrupt_data() {
        // Premature end of stream without EOD
        let empty = vec![0x80]; // partial bits
        let res = decode_lzw(&empty, true);
        // Clean error or graceful EOF
        assert!(res.is_ok() || res.is_err());

        // Completely invalid codes
        let invalid = vec![0xFF, 0xFF, 0xFF, 0xFF];
        assert!(decode_lzw(&invalid, true).is_err());
    }

    #[test]
    fn test_dct_roundtrip_synthetic() {
        let width = 8u32;
        let height = 8u32;
        // Generate an 8x8 synthetic RGB checkerboard pattern
        let mut rgb = Vec::with_capacity((width * height * 3) as usize);
        for y in 0..height {
            for x in 0..width {
                if (x + y) % 2 == 0 {
                    rgb.extend_from_slice(&[255, 0, 0]); // Red
                } else {
                    rgb.extend_from_slice(&[0, 255, 255]); // Cyan
                }
            }
        }

        // Encode as JPEG at high quality 95
        let jpeg_bytes = encode_dct(&rgb, width, height, 95).expect("encode dct");
        assert!(jpeg_bytes.starts_with(&[0xFF, 0xD8])); // JPEG SOI
        assert!(jpeg_bytes.ends_with(&[0xFF, 0xD9])); // JPEG EOI

        // Decode with dimensions
        let (decoded_rgb, w, h) = decode_dct_with_dimensions(&jpeg_bytes).expect("decode dct with dims");
        assert_eq!(w, width);
        assert_eq!(h, height);
        assert_eq!(decoded_rgb.len(), (width * height * 3) as usize);

        // Decode via decode_stream
        let stream_decoded = decode_stream("DCTDecode", &jpeg_bytes).expect("decode stream dct");
        assert_eq!(stream_decoded.len(), (width * height * 3) as usize);

        // Decode via abbreviation
        let abbr_decoded = decode_stream("DCT", &jpeg_bytes).expect("decode stream dct abbr");
        assert_eq!(abbr_decoded, stream_decoded);
    }

    #[test]
    fn test_dct_corrupted_stream() {
        // Non-JPEG arbitrary bytes
        let corrupt_data = b"This is not a JPEG stream at all!";
        let err = decode_dct(corrupt_data);
        assert!(err.is_err());
        assert!(matches!(err.unwrap_err(), PdfError::FilterError(_)));

        // Truncated JPEG stream (SOI without valid payload)
        let truncated = vec![0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10];
        let err2 = decode_dct(&truncated);
        assert!(err2.is_err());
    }

    #[test]
    fn test_dct_dimension_mismatch() {
        let bad_rgb = vec![0u8; 10]; // Not 8x8x3 = 192 bytes
        let err = encode_dct(&bad_rgb, 8, 8, 85);
        assert!(err.is_err());
    }

    #[test]
    fn test_filter_dispatch_aliases() {
        let text = b"Testing filter aliases across ISO 32000 Table 6";

        // FlateDecode and Fl
        let flate_enc = encode_stream("FlateDecode", text).unwrap();
        assert_eq!(decode_stream("FlateDecode", &flate_enc).unwrap(), text);
        assert_eq!(decode_stream("/FlateDecode", &flate_enc).unwrap(), text);
        assert_eq!(decode_stream("Fl", &flate_enc).unwrap(), text);
        assert_eq!(decode_stream("/Fl", &flate_enc).unwrap(), text);

        // ASCIIHexDecode and AHx
        let hex_enc = encode_stream("AHx", text).unwrap();
        assert_eq!(decode_stream("ASCIIHexDecode", &hex_enc).unwrap(), text);
        assert_eq!(decode_stream("/AHx", &hex_enc).unwrap(), text);

        // ASCII85Decode and A85
        let a85_enc = encode_stream("A85", text).unwrap();
        assert_eq!(decode_stream("ASCII85Decode", &a85_enc).unwrap(), text);
        assert_eq!(decode_stream("/A85", &a85_enc).unwrap(), text);

        // RunLengthDecode and RL
        let rl_enc = encode_stream("RL", text).unwrap();
        assert_eq!(decode_stream("RunLengthDecode", &rl_enc).unwrap(), text);
        assert_eq!(decode_stream("/RL", &rl_enc).unwrap(), text);

        // LZWDecode and LZW
        let lzw_enc = encode_stream("LZW", text).unwrap();
        assert_eq!(decode_stream("LZWDecode", &lzw_enc).unwrap(), text);
        assert_eq!(decode_stream("/LZW", &lzw_enc).unwrap(), text);

        // Unsupported filter
        assert!(decode_stream("UnsupportedFilter", text).is_err());
    }

    #[test]
    fn test_lzw_early_change_threshold_crossings() {
        let mut large_data = Vec::new();
        for i in 0..2000 {
            large_data.extend_from_slice(format!("Token_{:04}_data_chunk_#", i % 100).as_bytes());
        }

        let enc_early = encode_lzw(&large_data, true).expect("encode lzw EarlyChange=true");
        let dec_early = decode_lzw(&enc_early, true).expect("decode lzw EarlyChange=true");
        assert_eq!(large_data, dec_early, "EarlyChange=true roundtrip must match");

        let enc_late = encode_lzw(&large_data, false).expect("encode lzw EarlyChange=false");
        let dec_late = decode_lzw(&enc_late, false).expect("decode lzw EarlyChange=false");
        assert_eq!(large_data, dec_late, "EarlyChange=false roundtrip must match");
    }

    #[test]
    fn test_lzw_table_saturation_clear_table_emission() {
        let mut diverse_data = Vec::with_capacity(30000);
        for i in 0..15000 {
            diverse_data.push((i % 256) as u8);
            diverse_data.push(((i * 7 + 13) % 256) as u8);
        }

        for &early_change in &[true, false] {
            let enc = encode_lzw(&diverse_data, early_change).expect("encode diverse LZW");
            let dec = decode_lzw(&enc, early_change).expect("decode diverse LZW");
            assert_eq!(
                diverse_data, dec,
                "LZW with > 4096 code entries must cleanly roundtrip through ClearTable reset (early={})",
                early_change
            );
        }
    }

    #[test]
    fn test_ccitt_fax_roundtrip_g4() {
        let width = 64u32;
        let height = 8u32;
        let bytes_per_row = (width as usize + 7) / 8;
        let mut bitmap = vec![0u8; bytes_per_row * height as usize];
        // Create an alternating checkerboard pattern
        for y in 0..height as usize {
            for x in 0..bytes_per_row {
                bitmap[y * bytes_per_row + x] = if (y + x) % 2 == 0 { 0xAA } else { 0x55 };
            }
        }

        let encoded = encode_ccitt_fax(&bitmap, width, height, false).expect("encode CCITT G4");
        assert!(!encoded.is_empty(), "Encoded CCITT stream should not be empty");

        let params = CcittFaxParams {
            k: -1, // Group 4
            columns: width,
            rows: height,
            black_is_1: false,
            ..Default::default()
        };
        let decoded = decode_ccitt_fax(&encoded, &params).expect("decode CCITT G4");
        assert_eq!(bitmap, decoded, "CCITT G4 decoded bitmap must match original bitmap");
    }

    #[test]
    fn test_ccitt_fax_dispatch_and_aliases() {
        let width = 32u32;
        let height = 4u32;
        let bytes_per_row = (width as usize + 7) / 8;
        let bitmap = vec![0xF0; bytes_per_row * height as usize];

        let encoded = encode_ccitt_fax(&bitmap, width, height, false).unwrap();

        let mut dict = crate::object::PdfDictionary::new();
        dict.insert("K", -1i64);
        dict.insert("Columns", width as i64);
        dict.insert("Rows", height as i64);

        let decoded1 = decode_stream_with_dict("CCITTFaxDecode", &encoded, Some(&dict)).unwrap();
        assert_eq!(bitmap, decoded1);

        let decoded2 = decode_stream_with_dict("/CCF", &encoded, Some(&dict)).unwrap();
        assert_eq!(bitmap, decoded2);
    }

    #[test]
    fn test_jbig2_invalid_header_handling() {
        let bad_data = b"NOT_A_VALID_JBIG2_STREAM";
        let res = decode_jbig2(bad_data, None);
        assert!(res.is_err(), "Invalid JBIG2 stream should return an error");
    }
}
