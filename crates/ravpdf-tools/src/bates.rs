//! Bates Numbering for legal production conforming to ISO 32000-1.
//!
//! Traceability: FR-18.4, FR-37.2 (ravpdf bates).

use ravpdf_core::document::PdfDocument;
use ravpdf_core::error::{PdfError, Result};
use ravpdf_core::object::{Object, PdfDictionary, PdfName, PdfStream};

/// Applies sequential Bates numbering across all pages in the document.
pub fn apply_bates_numbering(
    doc: &mut PdfDocument,
    prefix: &str,
    start_num: u32,
    digits: usize,
    suffix: &str,
) -> Result<()> {
    let page_ids = doc.page_ids();
    let total_pages = page_ids.len();

    for i in 0..total_pages {
        let bates_number = start_num + (i as u32);
        let bates_str = format!("{}{:0width$}{}", prefix, bates_number, suffix, width = digits);

        let page = match doc.get_page(i) {
            Some(p) => p,
            None => continue,
        };

        let width = (page.crop_box[2] - page.crop_box[0]).abs();
        let x = width - 130.0;
        let y = 30.0; // 30 points above bottom edge

        let bates_cmd = format!(
            "\nq\nBT\n/F_BATES 10 Tf\n0 0 0 rg\n{:.2} {:.2} Td\n({}) Tj\nET\nQ\n",
            x, y, bates_str
        );

        let stream_id = doc.next_object_id();
        let mut stream =
            PdfStream::with_plain_data(PdfDictionary::new(), bates_cmd.into_bytes());
        let _ = stream.compress_flate();
        doc.objects.insert(stream_id, Object::Stream(stream));

        let page_id = page_ids[i];
        let page_obj = doc
            .objects
            .get_mut(&page_id)
            .ok_or_else(|| PdfError::ObjectNotFound(page_id.number, page_id.generation))?;
        let page_dict = page_obj
            .as_dict_mut()
            .ok_or_else(|| PdfError::InvalidStructure("Expected page dictionary".into()))?;

        match page_dict.get_mut("Contents") {
            Some(Object::Array(arr)) => {
                arr.push(Object::Reference(stream_id));
            }
            Some(Object::Reference(orig)) => {
                let orig_id = *orig;
                page_dict.insert(
                    "Contents",
                    vec![Object::Reference(orig_id), Object::Reference(stream_id)],
                );
            }
            _ => {
                page_dict.insert("Contents", stream_id);
            }
        }

        // Add standard font to Resources
        let resources = page_dict
            .map
            .entry(PdfName::new("Resources"))
            .or_insert_with(|| Object::Dictionary(PdfDictionary::new()));
        if let Object::Dictionary(res) = resources {
            let font_dict = res
                .map
                .entry(PdfName::new("Font"))
                .or_insert_with(|| Object::Dictionary(PdfDictionary::new()));
            if let Object::Dictionary(fonts) = font_dict {
                if !fonts.contains_key("F_BATES") {
                    let mut font_entry = PdfDictionary::new();
                    font_entry.insert("Type", PdfName::new("Font"));
                    font_entry.insert("Subtype", PdfName::new("Type1"));
                    font_entry.insert("BaseFont", PdfName::new("Courier-Bold"));
                    fonts.insert("F_BATES", font_entry);
                }
            }
        }
    }

    Ok(())
}
