//! Page Labeling Schemes conforming to ISO 32000-1 Section 8.3.1.
//!
//! Traceability: FR-06.9.

use ravpdf_core::document::PdfDocument;
use ravpdf_core::error::{PdfError, Result};
use ravpdf_core::object::{Object, PdfDictionary, PdfName, PdfString};

/// Numbering style for page labels according to ISO 32000-1 Table 159.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageLabelStyle {
    /// /D: Decimal arabic numerals (1, 2, 3...)
    Decimal,
    /// /R: Uppercase roman numerals (I, II, III...)
    UppercaseRoman,
    /// /r: Lowercase roman numerals (i, ii, iii...)
    LowercaseRoman,
    /// /A: Uppercase letters (A, B, C... Z, AA, BB...)
    UppercaseLetters,
    /// /a: Lowercase letters (a, b, c... z, aa, bb...)
    LowercaseLetters,
}

impl PageLabelStyle {
    pub fn to_pdf_name(&self) -> &'static str {
        match self {
            PageLabelStyle::Decimal => "D",
            PageLabelStyle::UppercaseRoman => "R",
            PageLabelStyle::LowercaseRoman => "r",
            PageLabelStyle::UppercaseLetters => "A",
            PageLabelStyle::LowercaseLetters => "a",
        }
    }

    pub fn from_pdf_name(name: &str) -> Option<Self> {
        match name {
            "D" => Some(PageLabelStyle::Decimal),
            "R" => Some(PageLabelStyle::UppercaseRoman),
            "r" => Some(PageLabelStyle::LowercaseRoman),
            "A" => Some(PageLabelStyle::UppercaseLetters),
            "a" => Some(PageLabelStyle::LowercaseLetters),
            _ => None,
        }
    }
}

/// A page labeling range defining how pages starting at `start_page_index` are numbered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageLabelRange {
    /// 0-based page index where this numbering range begins.
    pub start_page_index: usize,
    /// Optional numbering style. If None, only `prefix` is displayed.
    pub style: Option<PageLabelStyle>,
    /// Optional prefix prepended to the page label (e.g. "A-", "Cover ", "Appendix-").
    pub prefix: Option<String>,
    /// 1-based start offset number for this range (defaults to 1).
    pub start_number: u32,
}

impl PageLabelRange {
    pub fn new(start_page_index: usize, style: Option<PageLabelStyle>) -> Self {
        Self {
            start_page_index,
            style,
            prefix: None,
            start_number: 1,
        }
    }

    pub fn with_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.prefix = Some(prefix.into());
        self
    }

    pub fn with_start_number(mut self, start_number: u32) -> Self {
        self.start_number = start_number.max(1);
        self
    }
}

/// Converts a positive integer to Roman numerals.
pub fn to_roman(mut num: u32, uppercase: bool) -> String {
    if num == 0 {
        return String::new();
    }
    const TABLE: &[(u32, &str)] = &[
        (1000, "M"),
        (900, "CM"),
        (500, "D"),
        (400, "CD"),
        (100, "C"),
        (90, "XC"),
        (50, "L"),
        (40, "XL"),
        (10, "X"),
        (9, "IX"),
        (5, "V"),
        (4, "IV"),
        (1, "I"),
    ];

    let mut result = String::new();
    for &(val, sym) in TABLE {
        while num >= val {
            result.push_str(sym);
            num -= val;
        }
    }

    if uppercase {
        result
    } else {
        result.to_lowercase()
    }
}

/// Converts a positive integer to repeated letters conforming to ISO 32000-1:
/// "a through z for the first 26 pages, aa through zz for the next 26, and so on."
pub fn to_alpha(num: u32, uppercase: bool) -> String {
    if num == 0 {
        return String::new();
    }
    let count = ((num - 1) / 26 + 1) as usize;
    let letter_idx = ((num - 1) % 26) as u8;
    let base_char = if uppercase { b'A' } else { b'a' };
    let ch = (base_char + letter_idx) as char;
    ch.to_string().repeat(count)
}

/// Formats a page label for a given 0-based page index given the document's label ranges.
pub fn format_page_label(ranges: &[PageLabelRange], page_index: usize) -> String {
    if ranges.is_empty() {
        return (page_index + 1).to_string();
    }

    // Find the range with the highest start_page_index that is <= page_index
    let active_range = ranges
        .iter()
        .filter(|r| r.start_page_index <= page_index)
        .max_by_key(|r| r.start_page_index);

    let range = match active_range {
        Some(r) => r,
        None => return (page_index + 1).to_string(),
    };

    let offset = (page_index - range.start_page_index) as u32;
    let num = range.start_number + offset;

    let num_str = match range.style {
        Some(PageLabelStyle::Decimal) => num.to_string(),
        Some(PageLabelStyle::UppercaseRoman) => to_roman(num, true),
        Some(PageLabelStyle::LowercaseRoman) => to_roman(num, false),
        Some(PageLabelStyle::UppercaseLetters) => to_alpha(num, true),
        Some(PageLabelStyle::LowercaseLetters) => to_alpha(num, false),
        None => String::new(),
    };

    match &range.prefix {
        Some(prefix) => format!("{}{}", prefix, num_str),
        None => {
            if num_str.is_empty() {
                (page_index + 1).to_string()
            } else {
                num_str
            }
        }
    }
}

/// Extracts page label ranges from the document's Catalog `/PageLabels` number tree.
pub fn get_page_labels(doc: &PdfDocument) -> Result<Vec<PageLabelRange>> {
    let mut ranges = Vec::new();

    let catalog = match doc.catalog() {
        Some(c) => c,
        None => return Ok(ranges),
    };

    let pagelabels_dict = match catalog.get("PageLabels") {
        Some(Object::Dictionary(d)) => Some(d),
        Some(Object::Reference(id)) => doc.objects.get(id).and_then(Object::as_dict),
        _ => None,
    };

    let pl_dict = match pagelabels_dict {
        Some(d) => d,
        None => return Ok(ranges),
    };

    let nums_array = match pl_dict.get("Nums") {
        Some(Object::Array(arr)) => arr,
        _ => return Ok(ranges),
    };

    let mut i = 0;
    while i + 1 < nums_array.len() {
        let page_idx = match &nums_array[i] {
            Object::Integer(n) => *n as usize,
            _ => {
                i += 2;
                continue;
            }
        };

        let label_dict = match &nums_array[i + 1] {
            Object::Dictionary(d) => Some(d),
            Object::Reference(id) => doc.objects.get(id).and_then(Object::as_dict),
            _ => None,
        };

        if let Some(ld) = label_dict {
            let style = ld.get_name("S").and_then(PageLabelStyle::from_pdf_name);
            let prefix = ld.get_str("P");
            let start_number = ld.get_i64("St").unwrap_or(1).max(1) as u32;

            ranges.push(PageLabelRange {
                start_page_index: page_idx,
                style,
                prefix,
                start_number,
            });
        }

        i += 2;
    }

    ranges.sort_by_key(|r| r.start_page_index);
    Ok(ranges)
}

/// Sets page label ranges into the document's Catalog `/PageLabels` conforming to ISO 32000-1 Section 8.3.1.
pub fn set_page_labels(doc: &mut PdfDocument, ranges: &[PageLabelRange]) -> Result<()> {
    let mut sorted_ranges = ranges.to_vec();
    sorted_ranges.sort_by_key(|r| r.start_page_index);

    let mut nums_vec = Vec::new();

    for r in &sorted_ranges {
        nums_vec.push(Object::Integer(r.start_page_index as i64));

        let mut dict = PdfDictionary::new();
        dict.insert("Type", PdfName::new("PageLabel"));
        if let Some(style) = r.style {
            dict.insert("S", PdfName::new(style.to_pdf_name()));
        }
        if let Some(ref p) = r.prefix {
            dict.insert("P", PdfString::from_str(p));
        }
        if r.start_number != 1 {
            dict.insert("St", Object::Integer(r.start_number as i64));
        }

        nums_vec.push(Object::Dictionary(dict));
    }

    let pl_dict_id = doc.next_object_id();
    let mut pl_dict = PdfDictionary::new();
    pl_dict.insert("Nums", Object::Array(nums_vec));
    doc.objects.insert(pl_dict_id, Object::Dictionary(pl_dict));

    let root_id = doc.root_id;
    let root_obj = doc
        .objects
        .get_mut(&root_id)
        .ok_or(PdfError::ObjectNotFound(root_id.number, root_id.generation))?;

    let root_dict = root_obj
        .as_dict_mut()
        .ok_or_else(|| PdfError::InvalidStructure("Root object is not a dictionary".into()))?;

    root_dict.insert("PageLabels", Object::Reference(pl_dict_id));

    Ok(())
}

/// Convenience function to get the formatted label for a specific page index in a document.
pub fn get_page_label(doc: &PdfDocument, page_index: usize) -> String {
    match get_page_labels(doc) {
        Ok(ranges) if !ranges.is_empty() => format_page_label(&ranges, page_index),
        _ => (page_index + 1).to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_roman_conversion() {
        assert_eq!(to_roman(1, true), "I");
        assert_eq!(to_roman(4, false), "iv");
        assert_eq!(to_roman(9, true), "IX");
        assert_eq!(to_roman(14, false), "xiv");
        assert_eq!(to_roman(42, true), "XLII");
        assert_eq!(to_roman(2026, true), "MMXXVI");
    }

    #[test]
    fn test_alpha_conversion() {
        assert_eq!(to_alpha(1, true), "A");
        assert_eq!(to_alpha(26, true), "Z");
        assert_eq!(to_alpha(27, true), "AA");
        assert_eq!(to_alpha(28, true), "BB");
        assert_eq!(to_alpha(52, false), "zz");
        assert_eq!(to_alpha(53, false), "aaa");
    }

    #[test]
    fn test_format_page_label_ranges() {
        let ranges = vec![
            PageLabelRange::new(0, Some(PageLabelStyle::LowercaseRoman)), // pages 0-3: i, ii, iii, iv
            PageLabelRange::new(4, Some(PageLabelStyle::Decimal)).with_start_number(1), // pages 4-9: 1, 2, 3...
            PageLabelRange::new(10, Some(PageLabelStyle::UppercaseLetters)).with_prefix("App-"), // pages 10+: App-A, App-B...
        ];

        assert_eq!(format_page_label(&ranges, 0), "i");
        assert_eq!(format_page_label(&ranges, 2), "iii");
        assert_eq!(format_page_label(&ranges, 4), "1");
        assert_eq!(format_page_label(&ranges, 5), "2");
        assert_eq!(format_page_label(&ranges, 10), "App-A");
        assert_eq!(format_page_label(&ranges, 11), "App-B");
    }

    #[test]
    fn test_page_labels_document_roundtrip() {
        let mut doc = PdfDocument::new();
        for _ in 0..15 {
            doc.add_blank_page(612.0, 792.0);
        }

        let ranges = vec![
            PageLabelRange::new(0, Some(PageLabelStyle::LowercaseRoman)),
            PageLabelRange::new(3, Some(PageLabelStyle::Decimal)).with_start_number(1),
            PageLabelRange::new(10, Some(PageLabelStyle::UppercaseLetters)).with_prefix("Sec-"),
        ];

        set_page_labels(&mut doc, &ranges).expect("set page labels");

        let retrieved = get_page_labels(&doc).expect("get page labels");
        assert_eq!(retrieved.len(), 3);
        assert_eq!(retrieved[0].style, Some(PageLabelStyle::LowercaseRoman));
        assert_eq!(retrieved[1].style, Some(PageLabelStyle::Decimal));
        assert_eq!(retrieved[2].prefix.as_deref(), Some("Sec-"));

        assert_eq!(get_page_label(&doc, 0), "i");
        assert_eq!(get_page_label(&doc, 1), "ii");
        assert_eq!(get_page_label(&doc, 3), "1");
        assert_eq!(get_page_label(&doc, 4), "2");
        assert_eq!(get_page_label(&doc, 10), "Sec-A");
    }
}
