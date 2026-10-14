//! PDF Stream Filters conforming to ISO 32000-1 Section 7.4.
//!
//! Traceability: FR-01.1.1, Section 7.1 Stream Filters.

use crate::error::{PdfError, Result};
use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;
use flate2::Compression;
use std::io::{Read, Write};

/// Decodes raw stream data using the named PDF filter.
pub fn decode_stream(filter_name: &str, data: &[u8]) -> Result<Vec<u8>> {
    match filter_name {
        "FlateDecode" | "/FlateDecode" => decode_flate(data),
        "ASCIIHexDecode" | "/ASCIIHexDecode" => decode_ascii_hex(data),
        "ASCII85Decode" | "/ASCII85Decode" => decode_ascii85(data),
        "RunLengthDecode" | "/RunLengthDecode" => decode_run_length(data),
        other => Err(PdfError::FilterError(format!(
            "Unsupported or unhandled stream filter: {}",
            other
        ))),
    }
}

/// Compresses stream data using the specified filter (default is FlateDecode).
pub fn encode_stream(filter_name: &str, data: &[u8]) -> Result<Vec<u8>> {
    match filter_name {
        "FlateDecode" | "/FlateDecode" => encode_flate(data),
        "ASCIIHexDecode" | "/ASCIIHexDecode" => encode_ascii_hex(data),
        "ASCII85Decode" | "/ASCII85Decode" => encode_ascii85(data),
        "RunLengthDecode" | "/RunLengthDecode" => encode_run_length(data),
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

/// ASCIIHexDecode: Decodes ASCII hexadecimal digits.
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

/// ASCIIHexEncode: Encodes bytes to uppercase hex ASCII.
pub fn encode_ascii_hex(data: &[u8]) -> Result<Vec<u8>> {
    let mut output = Vec::with_capacity(data.len() * 2 + 1);
    for &byte in data {
        output.extend_from_slice(format!("{:02X}", byte).as_bytes());
    }
    output.push(b'>');
    Ok(output)
}

/// ASCII85Decode: Decodes 5-character groups in the range '!' (33) to 'u' (117)
/// into 4-byte integers. 'z' expands to four 0x00 bytes.
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

/// RunLengthDecode: Decodes run-length encoded stream.
pub fn decode_run_length(data: &[u8]) -> Result<Vec<u8>> {
    let mut output = Vec::new();
    let mut i = 0;

    while i < data.len() {
        let length = data[i];
        i += 1;

        if length == 128 {
            // EOD
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
    fn test_ascii85_roundtrip() {
        let original = b"Hello World! Testing ASCII85 Base-85 encoding for PDF streams.";
        let encoded = encode_ascii85(original).expect("encode ascii85");
        let decoded = decode_ascii85(&encoded).expect("decode ascii85");
        assert_eq!(original.to_vec(), decoded);
    }

    #[test]
    fn test_run_length_roundtrip() {
        let original = b"AAAAABBBBBCCCCCDDDDD1234567890EEEEE";
        let encoded = encode_run_length(original).expect("encode run length");
        let decoded = decode_run_length(&encoded).expect("decode run length");
        assert_eq!(original.to_vec(), decoded);
    }
}
