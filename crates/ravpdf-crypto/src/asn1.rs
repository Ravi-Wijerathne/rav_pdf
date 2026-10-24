//! ASN.1 DER (Distinguished Encoding Rules) encoder and decoder for cryptographic structures.
//!
//! Traceability:
//! - FR-13.1 (PKCS#7 / CMS and X.509 certificate DER structure generation)
//! - FR-13.6 (PAdES baseline signature structure)
//! - FR-13.7 (CAdES signature format)
//! - FR-13.13 (Digital ID creation)

use anyhow::{anyhow, Result};

pub const TAG_INTEGER: u8 = 0x02;
pub const TAG_BIT_STRING: u8 = 0x03;
pub const TAG_OCTET_STRING: u8 = 0x04;
pub const TAG_NULL: u8 = 0x05;
pub const TAG_OID: u8 = 0x06;
pub const TAG_UTF8_STRING: u8 = 0x0C;
pub const TAG_PRINTABLE_STRING: u8 = 0x13;
pub const TAG_UTC_TIME: u8 = 0x17;
pub const TAG_GENERALIZED_TIME: u8 = 0x18;
pub const TAG_SEQUENCE: u8 = 0x30;
pub const TAG_SET: u8 = 0x31;

/// Encodes a length value in DER format.
pub fn encode_length(len: usize) -> Vec<u8> {
    if len < 128 {
        vec![len as u8]
    } else if len <= 0xFF {
        vec![0x81, len as u8]
    } else if len <= 0xFFFF {
        vec![0x82, (len >> 8) as u8, (len & 0xFF) as u8]
    } else if len <= 0xFF_FFFF {
        vec![
            0x83,
            (len >> 16) as u8,
            ((len >> 8) & 0xFF) as u8,
            (len & 0xFF) as u8,
        ]
    } else {
        vec![
            0x84,
            (len >> 24) as u8,
            ((len >> 16) & 0xFF) as u8,
            ((len >> 8) & 0xFF) as u8,
            (len & 0xFF) as u8,
        ]
    }
}

/// Encodes a Tag-Length-Value (TLV) triple.
pub fn encode_tlv(tag: u8, value: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(1 + 4 + value.len());
    out.push(tag);
    out.extend_from_slice(&encode_length(value.len()));
    out.extend_from_slice(value);
    out
}

/// Encodes a SEQUENCE of elements.
pub fn encode_sequence(elements: &[Vec<u8>]) -> Vec<u8> {
    let total_len: usize = elements.iter().map(|e| e.len()).sum();
    let mut val = Vec::with_capacity(total_len);
    for elem in elements {
        val.extend_from_slice(elem);
    }
    encode_tlv(TAG_SEQUENCE, &val)
}

/// Encodes a SET of elements.
pub fn encode_set(elements: &[Vec<u8>]) -> Vec<u8> {
    let total_len: usize = elements.iter().map(|e| e.len()).sum();
    let mut val = Vec::with_capacity(total_len);
    for elem in elements {
        val.extend_from_slice(elem);
    }
    encode_tlv(TAG_SET, &val)
}

/// Encodes a context-specific tagged element [tag_num].
pub fn encode_context_tagged(tag_num: u8, value: &[u8], constructed: bool) -> Vec<u8> {
    let tag = if constructed {
        0xA0 | (tag_num & 0x1F)
    } else {
        0x80 | (tag_num & 0x1F)
    };
    encode_tlv(tag, value)
}

/// Encodes an unsigned integer from bytes (prepends 0x00 if highest bit is set).
pub fn encode_integer_bytes(bytes: &[u8]) -> Vec<u8> {
    let mut val = Vec::new();
    // Trim leading zeroes unless only 0 remains
    let mut start = 0;
    while start < bytes.len() - 1 && bytes[start] == 0 {
        start += 1;
    }
    let trimmed = &bytes[start..];
    if trimmed.is_empty() {
        val.push(0);
    } else {
        if trimmed[0] & 0x80 != 0 {
            val.push(0); // Prepend 0 to indicate positive integer
        }
        val.extend_from_slice(trimmed);
    }
    encode_tlv(TAG_INTEGER, &val)
}

/// Encodes a u64 as an ASN.1 INTEGER.
pub fn encode_integer_u64(val: u64) -> Vec<u8> {
    let bytes = val.to_be_bytes();
    encode_integer_bytes(&bytes)
}

/// Encodes an ASN.1 NULL element (0x05 0x00).
pub fn encode_null() -> Vec<u8> {
    vec![TAG_NULL, 0x00]
}

/// Encodes an OCTET STRING.
pub fn encode_octet_string(data: &[u8]) -> Vec<u8> {
    encode_tlv(TAG_OCTET_STRING, data)
}

/// Encodes a BIT STRING.
pub fn encode_bit_string(data: &[u8], unused_bits: u8) -> Vec<u8> {
    let mut val = Vec::with_capacity(1 + data.len());
    val.push(unused_bits);
    val.extend_from_slice(data);
    encode_tlv(TAG_BIT_STRING, &val)
}

/// Encodes a UTF8String.
pub fn encode_utf8_string(s: &str) -> Vec<u8> {
    encode_tlv(TAG_UTF8_STRING, s.as_bytes())
}

/// Encodes a PrintableString.
pub fn encode_printable_string(s: &str) -> Vec<u8> {
    encode_tlv(TAG_PRINTABLE_STRING, s.as_bytes())
}

/// Encodes an Object Identifier (OID) in dot notation (e.g. "1.2.840.113549.1.7.2").
pub fn encode_oid(oid_str: &str) -> Result<Vec<u8>> {
    let parts: Vec<u64> = oid_str
        .split('.')
        .map(|p| p.parse::<u64>().map_err(|e| anyhow!("Invalid OID segment '{}': {}", p, e)))
        .collect::<Result<Vec<u64>>>()?;

    if parts.len() < 2 {
        return Err(anyhow!("OID must have at least 2 components: {}", oid_str));
    }

    let mut body = Vec::new();
    // First byte is (40 * first + second)
    body.push(((40 * parts[0]) + parts[1]) as u8);

    for &part in &parts[2..] {
        if part < 128 {
            body.push(part as u8);
        } else {
            let mut stack = Vec::new();
            let mut val = part;
            stack.push((val & 0x7F) as u8);
            val >>= 7;
            while val > 0 {
                stack.push(((val & 0x7F) | 0x80) as u8);
                val >>= 7;
            }
            while let Some(b) = stack.pop() {
                body.push(b);
            }
        }
    }

    Ok(encode_tlv(TAG_OID, &body))
}

/// Encodes a UTC Time string in "YYMMDDHHMMSSZ" format.
pub fn encode_utc_time(dt: &chrono::DateTime<chrono::Utc>) -> Vec<u8> {
    let formatted = dt.format("%y%m%d%H%M%SZ").to_string();
    encode_tlv(TAG_UTC_TIME, formatted.as_bytes())
}

/// Parses the leading TLV header from bytes and returns `(tag, value, remainder)`.
pub fn parse_tlv(data: &[u8]) -> Result<(u8, &[u8], &[u8])> {
    if data.is_empty() {
        return Err(anyhow!("Unexpected end of data reading ASN.1 tag"));
    }
    let tag = data[0];
    let mut idx = 1;
    if idx >= data.len() {
        return Err(anyhow!("Unexpected end of data reading ASN.1 length"));
    }
    let first_len = data[idx];
    idx += 1;

    let len: usize = if first_len < 128 {
        first_len as usize
    } else {
        let num_bytes = (first_len & 0x7F) as usize;
        if idx + num_bytes > data.len() {
            return Err(anyhow!("Length prefix extends past data boundary"));
        }
        let mut l = 0usize;
        for &b in &data[idx..idx + num_bytes] {
            l = (l << 8) | (b as usize);
        }
        idx += num_bytes;
        l
    };

    if idx + len > data.len() {
        return Err(anyhow!(
            "TLV value length ({}) exceeds remaining buffer ({})",
            len,
            data.len() - idx
        ));
    }

    let val = &data[idx..idx + len];
    let rem = &data[idx + len..];
    Ok((tag, val, rem))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_asn1_oid_encoding() {
        // OID 1.2.840.113549.1.7.2 (CMS SignedData)
        let der = encode_oid("1.2.840.113549.1.7.2").expect("encode oid");
        assert_eq!(der[0], TAG_OID);
        // 1*40 + 2 = 42 (0x2A), 840 = 0x86 0x48, 113549 = 0x86 0xF7 0x0D, 1 = 0x01, 7 = 0x07, 2 = 0x02
        assert_eq!(&der[2..], &[0x2A, 0x86, 0x48, 0x86, 0xF7, 0x0D, 0x01, 0x07, 0x02]);
    }

    #[test]
    fn test_asn1_tlv_roundtrip() {
        let seq = encode_sequence(&[
            encode_integer_u64(42),
            encode_utf8_string("RavPDF Engine"),
        ]);
        let (tag, val, rem) = parse_tlv(&seq).expect("parse tlv");
        assert_eq!(tag, TAG_SEQUENCE);
        assert!(rem.is_empty());

        let (tag1, val1, rem1) = parse_tlv(val).expect("parse child 1");
        assert_eq!(tag1, TAG_INTEGER);
        assert_eq!(val1, &[42]);

        let (tag2, val2, _) = parse_tlv(rem1).expect("parse child 2");
        assert_eq!(tag2, TAG_UTF8_STRING);
        assert_eq!(val2, b"RavPDF Engine");
    }
}
