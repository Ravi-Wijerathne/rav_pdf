//! PDF Object Model conforming to ISO 32000-1 Section 7.3.
//!
//! Traceability: Section 7.1 PDF Internal Structure, DR-01.

use crate::error::{PdfError, Result};
use crate::filter::{decode_stream_with_dict, encode_flate};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;

/// An Indirect Object Identifier comprising an object number and generation number.
/// Format in PDF: `N G R` (reference) or `N G obj` (definition).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ObjectId {
    pub number: u32,
    pub generation: u16,
}

impl ObjectId {
    pub const fn new(number: u32, generation: u16) -> Self {
        Self { number, generation }
    }
}

impl fmt::Display for ObjectId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {} R", self.number, self.generation)
    }
}

/// A PDF Name object (ISO 32000-1 Section 7.3.5).
/// Names begin with a solidus `/` and can contain octet codes encoded as `#XX`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct PdfName(pub String);

impl PdfName {
    pub fn new(name: impl Into<String>) -> Self {
        let mut s = name.into();
        if s.starts_with('/') {
            s.remove(0);
        }
        Self(s)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for PdfName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "/{}", self.0)
    }
}

impl From<&str> for PdfName {
    fn from(s: &str) -> Self {
        Self::new(s)
    }
}

impl From<String> for PdfName {
    fn from(s: String) -> Self {
        Self::new(s)
    }
}

/// A PDF String object (ISO 32000-1 Section 7.3.4).
/// Can be represented as literal text `(...)` or hexadecimal `<...>`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PdfString {
    pub bytes: Vec<u8>,
    pub is_hex: bool,
}

impl PdfString {
    pub fn from_bytes(bytes: Vec<u8>) -> Self {
        Self {
            bytes,
            is_hex: false,
        }
    }

    pub fn from_hex(bytes: Vec<u8>) -> Self {
        Self {
            bytes,
            is_hex: true,
        }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Self {
        Self {
            bytes: s.as_bytes().to_vec(),
            is_hex: false,
        }
    }

    pub fn to_string_lossy(&self) -> String {
        // Check for UTF-16BE BOM: \xFE\xFF
        if self.bytes.len() >= 2 && self.bytes[0] == 0xFE && self.bytes[1] == 0xFF {
            let u16_chars: Vec<u16> = self.bytes[2..]
                .chunks_exact(2)
                .map(|chunk| ((chunk[0] as u16) << 8) | (chunk[1] as u16))
                .collect();
            String::from_utf16_lossy(&u16_chars)
        } else if self.bytes.len() >= 3
            && self.bytes[0] == 0xEF
            && self.bytes[1] == 0xBB
            && self.bytes[2] == 0xBF
        {
            // UTF-8 BOM
            String::from_utf8_lossy(&self.bytes[3..]).into_owned()
        } else {
            // PDFDocEncoding / ASCII fallback
            String::from_utf8_lossy(&self.bytes).into_owned()
        }
    }
}

impl From<&str> for PdfString {
    fn from(s: &str) -> Self {
        Self::from_str(s)
    }
}

impl From<String> for PdfString {
    fn from(s: String) -> Self {
        Self::from_bytes(s.into_bytes())
    }
}

/// A PDF Dictionary object (ISO 32000-1 Section 7.3.7).
/// Maps `PdfName` keys to `Object` values. BTreeMap ensures deterministic output order.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct PdfDictionary {
    pub map: BTreeMap<PdfName, Object>,
}

impl PdfDictionary {
    pub fn new() -> Self {
        Self {
            map: BTreeMap::new(),
        }
    }

    pub fn insert<K: Into<PdfName>, V: Into<Object>>(&mut self, key: K, val: V) {
        self.map.insert(key.into(), val.into());
    }

    pub fn get(&self, key: &str) -> Option<&Object> {
        let key = key.strip_prefix('/').unwrap_or(key);
        self.map.get(&PdfName(key.to_string()))
    }

    pub fn get_mut(&mut self, key: &str) -> Option<&mut Object> {
        let key = key.strip_prefix('/').unwrap_or(key);
        self.map.get_mut(&PdfName(key.to_string()))
    }

    pub fn remove(&mut self, key: &str) -> Option<Object> {
        let key = key.strip_prefix('/').unwrap_or(key);
        self.map.remove(&PdfName(key.to_string()))
    }

    pub fn contains_key(&self, key: &str) -> bool {
        let key = key.strip_prefix('/').unwrap_or(key);
        self.map.contains_key(&PdfName(key.to_string()))
    }

    pub fn get_name(&self, key: &str) -> Option<&str> {
        self.get(key).and_then(Object::as_name)
    }

    pub fn get_str(&self, key: &str) -> Option<String> {
        self.get(key).and_then(|obj| match obj {
            Object::String(s) => Some(s.to_string_lossy()),
            Object::Name(n) => Some(n.as_str().to_string()),
            _ => None,
        })
    }

    pub fn get_i64(&self, key: &str) -> Option<i64> {
        self.get(key).and_then(Object::as_i64)
    }

    pub fn get_f64(&self, key: &str) -> Option<f64> {
        self.get(key).and_then(Object::as_f64)
    }

    pub fn get_bool(&self, key: &str) -> Option<bool> {
        self.get(key).and_then(Object::as_bool)
    }

    pub fn get_dict(&self, key: &str) -> Option<&PdfDictionary> {
        self.get(key).and_then(Object::as_dict)
    }

    pub fn get_dict_mut(&mut self, key: &str) -> Option<&mut PdfDictionary> {
        self.get_mut(key).and_then(Object::as_dict_mut)
    }

    pub fn get_array(&self, key: &str) -> Option<&[Object]> {
        self.get(key).and_then(Object::as_array)
    }

    pub fn get_array_mut(&mut self, key: &str) -> Option<&mut Vec<Object>> {
        self.get_mut(key).and_then(Object::as_array_mut)
    }

    pub fn get_ref(&self, key: &str) -> Option<ObjectId> {
        self.get(key).and_then(Object::as_reference)
    }
}

/// A PDF Stream object (ISO 32000-1 Section 7.3.8).
/// Consists of a dictionary followed by binary content between `stream` and `endstream`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PdfStream {
    pub dict: PdfDictionary,
    pub raw_data: Vec<u8>,
}

impl PdfStream {
    pub fn new(dict: PdfDictionary, raw_data: Vec<u8>) -> Self {
        let mut s = Self { dict, raw_data };
        s.update_length();
        s
    }

    pub fn with_plain_data(mut dict: PdfDictionary, data: Vec<u8>) -> Self {
        dict.remove("Filter");
        dict.insert("Length", data.len() as i64);
        Self {
            dict,
            raw_data: data,
        }
    }

    /// Decompresses and returns the decoded payload using the stream's `/Filter` and `/DecodeParms`.
    pub fn decode(&self) -> Result<Vec<u8>> {
        let filter_obj = self.dict.get("Filter");
        let parms_obj = self.dict.get("DecodeParms");
        match filter_obj {
            None => Ok(self.raw_data.clone()),
            Some(Object::Name(name)) => {
                let dict_parm = parms_obj.and_then(Object::as_dict);
                decode_stream_with_dict(name.as_str(), &self.raw_data, dict_parm)
            }
            Some(Object::Array(arr)) => {
                let mut data = self.raw_data.clone();
                for (i, item) in arr.iter().enumerate() {
                    if let Some(name) = item.as_name() {
                        let dict_parm = match parms_obj {
                            Some(Object::Array(parms_arr)) => parms_arr.get(i).and_then(Object::as_dict),
                            Some(Object::Dictionary(d)) => Some(d),
                            _ => None,
                        };
                        data = decode_stream_with_dict(name, &data, dict_parm)?;
                    }
                }
                Ok(data)
            }
            _ => Err(PdfError::FilterError("Invalid /Filter type".into())),
        }
    }

    /// Compresses uncompressed data using FlateDecode and updates `/Filter` and `/Length`.
    pub fn compress_flate(&mut self) -> Result<()> {
        let compressed = encode_flate(&self.raw_data)?;
        self.dict.insert("Filter", PdfName::new("FlateDecode"));
        self.raw_data = compressed;
        self.update_length();
        Ok(())
    }

    pub fn update_length(&mut self) {
        self.dict.insert("Length", self.raw_data.len() as i64);
    }
}

/// The fundamental PDF Object enumeration conforming to ISO 32000-1 Section 7.3.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Object {
    Null,
    Boolean(bool),
    Integer(i64),
    Real(f64),
    String(PdfString),
    Name(PdfName),
    Array(Vec<Object>),
    Dictionary(PdfDictionary),
    Stream(PdfStream),
    Reference(ObjectId),
}

impl Object {
    pub fn type_name(&self) -> &'static str {
        match self {
            Object::Null => "null",
            Object::Boolean(_) => "boolean",
            Object::Integer(_) => "integer",
            Object::Real(_) => "real",
            Object::String(_) => "string",
            Object::Name(_) => "name",
            Object::Array(_) => "array",
            Object::Dictionary(_) => "dictionary",
            Object::Stream(_) => "stream",
            Object::Reference(_) => "reference",
        }
    }

    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Object::Integer(i) => Some(*i),
            Object::Real(r) => Some(*r as i64),
            _ => None,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Object::Real(r) => Some(*r),
            Object::Integer(i) => Some(*i as f64),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Object::Boolean(b) => Some(*b),
            _ => None,
        }
    }

    pub fn as_name(&self) -> Option<&str> {
        match self {
            Object::Name(n) => Some(n.as_str()),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Object::String(s) => std::str::from_utf8(&s.bytes).ok(),
            _ => None,
        }
    }

    pub fn as_dict(&self) -> Option<&PdfDictionary> {
        match self {
            Object::Dictionary(d) => Some(d),
            Object::Stream(s) => Some(&s.dict),
            _ => None,
        }
    }

    pub fn as_dict_mut(&mut self) -> Option<&mut PdfDictionary> {
        match self {
            Object::Dictionary(d) => Some(d),
            Object::Stream(s) => Some(&mut s.dict),
            _ => None,
        }
    }

    pub fn as_array(&self) -> Option<&[Object]> {
        match self {
            Object::Array(arr) => Some(arr.as_slice()),
            _ => None,
        }
    }

    pub fn as_array_mut(&mut self) -> Option<&mut Vec<Object>> {
        match self {
            Object::Array(arr) => Some(arr),
            _ => None,
        }
    }

    pub fn as_stream(&self) -> Option<&PdfStream> {
        match self {
            Object::Stream(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_stream_mut(&mut self) -> Option<&mut PdfStream> {
        match self {
            Object::Stream(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_reference(&self) -> Option<ObjectId> {
        match self {
            Object::Reference(id) => Some(*id),
            _ => None,
        }
    }
}

// Convenient From implementations
impl From<bool> for Object {
    fn from(b: bool) -> Self {
        Object::Boolean(b)
    }
}

impl From<i32> for Object {
    fn from(i: i32) -> Self {
        Object::Integer(i as i64)
    }
}

impl From<i64> for Object {
    fn from(i: i64) -> Self {
        Object::Integer(i)
    }
}

impl From<f32> for Object {
    fn from(f: f32) -> Self {
        Object::Real(f as f64)
    }
}

impl From<f64> for Object {
    fn from(f: f64) -> Self {
        Object::Real(f)
    }
}

impl From<PdfName> for Object {
    fn from(name: PdfName) -> Self {
        Object::Name(name)
    }
}

impl From<PdfString> for Object {
    fn from(s: PdfString) -> Self {
        Object::String(s)
    }
}

impl From<PdfDictionary> for Object {
    fn from(dict: PdfDictionary) -> Self {
        Object::Dictionary(dict)
    }
}

impl From<PdfStream> for Object {
    fn from(stream: PdfStream) -> Self {
        Object::Stream(stream)
    }
}

impl From<ObjectId> for Object {
    fn from(id: ObjectId) -> Self {
        Object::Reference(id)
    }
}

impl From<Vec<Object>> for Object {
    fn from(arr: Vec<Object>) -> Self {
        Object::Array(arr)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_object_conversions() {
        let mut dict = PdfDictionary::new();
        dict.insert("Type", PdfName::new("Page"));
        dict.insert("Count", 10);
        dict.insert("Width", 612.0);
        dict.insert("Visible", true);
        dict.insert("Title", PdfString::from_str("Sample Page"));
        dict.insert("Parent", ObjectId::new(1, 0));

        assert_eq!(dict.get_name("Type"), Some("Page"));
        assert_eq!(dict.get_i64("Count"), Some(10));
        assert_eq!(dict.get_f64("Width"), Some(612.0));
        assert_eq!(dict.get_bool("Visible"), Some(true));
        assert_eq!(dict.get_ref("Parent"), Some(ObjectId::new(1, 0)));
        assert_eq!(dict.get_str("Title"), Some("Sample Page".to_string()));
    }
}
