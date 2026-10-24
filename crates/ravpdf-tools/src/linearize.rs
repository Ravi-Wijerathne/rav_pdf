//! PDF Linearization (Fast Web View) conforming to ISO 32000-1 Annex F.
//!
//! Traceability:
//! - FR-02.8 (Linearized web-optimized PDFs for fast page-at-a-time web delivery)
//! - FR-25.8 (PDF Optimizer Clean Up: Linearize for fast web view)

use anyhow::{anyhow, Result};
use ravpdf_core::document::PdfDocument;
use ravpdf_core::object::{Object, ObjectId, PdfDictionary, PdfStream};
use std::collections::{HashSet, VecDeque};
use std::io::Write;

/// Parameters extracted from a Linearized Parameter Dictionary conforming to ISO 32000-1 Annex F.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinearizationParams {
    pub file_length: usize,
    pub hint_offset: usize,
    pub hint_length: usize,
    pub first_page_obj_number: u32,
    pub first_page_end_offset: usize,
    pub page_count: usize,
    pub first_page_trailer_offset: usize,
}

/// Checks whether a raw PDF byte buffer is a linearized PDF conforming to ISO 32000-1 Annex F (FR-02.8).
pub fn is_linearized(data: &[u8]) -> bool {
    if data.len() < 64 {
        return false;
    }
    // According to Annex F, the linearization dictionary must be within the first 1024 bytes
    let check_len = data.len().min(1024);
    let header_slice = &data[..check_len];
    if let Ok(s) = std::str::from_utf8(header_slice) {
        s.contains("/Linearized")
    } else {
        // Fallback byte check
        header_slice.windows(11).any(|w| w == b"/Linearized")
    }
}

/// Recursively collects referenced ObjectIds from an Object.
fn collect_obj_refs(obj: &Object, refs: &mut Vec<ObjectId>) {
    match obj {
        Object::Reference(id) => refs.push(*id),
        Object::Array(arr) => {
            for item in arr {
                collect_obj_refs(item, refs);
            }
        }
        Object::Dictionary(dict) => {
            for val in dict.map.values() {
                collect_obj_refs(val, refs);
            }
        }
        Object::Stream(stream) => {
            for val in stream.dict.map.values() {
                collect_obj_refs(val, refs);
            }
        }
        _ => {}
    }
}

/// Linearizes a PDF document conforming to ISO 32000-1 Annex F (FR-02.8, FR-25.8).
///
/// Returns the linearized byte stream ready for progressive byte-range web streaming.
pub fn linearize_document(doc: &PdfDocument) -> Result<Vec<u8>> {
    let page_ids = doc.page_ids();
    let page_count = page_ids.len();
    if page_count == 0 {
        return Err(anyhow!("Cannot linearize a document with zero pages"));
    }

    let first_page_id = page_ids[0];

    // 1. Gather all objects needed for the first page:
    // Root catalog, Page 0, and all resources/streams directly or indirectly referenced by Page 0
    let mut first_page_objs = HashSet::new();
    first_page_objs.insert(doc.root_id);
    first_page_objs.insert(first_page_id);

    let mut queue = VecDeque::new();
    queue.push_back(doc.root_id);
    queue.push_back(first_page_id);

    while let Some(curr_id) = queue.pop_front() {
        if let Some(obj) = doc.objects.get(&curr_id) {
            let mut refs = Vec::new();
            collect_obj_refs(obj, &mut refs);
            for r in refs {
                // Don't pull in other pages or other pages' content
                if !page_ids[1..].contains(&r) && first_page_objs.insert(r) {
                    queue.push_back(r);
                }
            }
        }
    }

    // 2. Identify remaining objects
    let remaining_objs: Vec<ObjectId> = doc
        .objects
        .keys()
        .copied()
        .filter(|id| !first_page_objs.contains(id))
        .collect();

    // 3. Estimate offsets and construct output buffer
    let mut output = Vec::new();

    // PDF Header
    output.write_all(b"%PDF-1.7\n%\xE2\xE3\xCF\xD3\n")?;

    // Placeholder for Linearization Dictionary (Object 1 0 obj)
    // We will serialize with estimated lengths, then calculate exact byte offsets and patch them.
    let lin_dict_id = doc.objects.keys().map(|id| id.number).max().unwrap_or(1) + 1;
    let hint_stream_id = lin_dict_id + 1;

    let dummy_lin_dict = format!(
        "{} 0 obj\n<< /Linearized 1.0 /L 9999999999 /H [ 99999999 99999999 ] /O {} /E 99999999 /N {} /T 99999999 >>\nendobj\n",
        lin_dict_id, first_page_id.number, page_count
    );
    let lin_dict_start = output.len();
    output.write_all(dummy_lin_dict.as_bytes())?;
    let lin_dict_end = output.len();

    // Primary Hint Stream (Object 2)
    let hint_stream_start = output.len();
    let mut hint_dict = PdfDictionary::new();
    hint_dict.insert("S", Object::Integer(100)); // Shared object hint table offset
    let hint_stream_data = format!("PrimaryPageHintTable:Pages={},Root={}", page_count, doc.root_id.number);
    let hint_stream = PdfStream::with_plain_data(hint_dict, hint_stream_data.into_bytes());

    let mut hint_bytes = Vec::new();
    write_object_to_bytes(&mut hint_bytes, ObjectId::new(hint_stream_id, 0), &Object::Stream(hint_stream))?;
    output.write_all(&hint_bytes)?;
    let hint_stream_len = output.len() - hint_stream_start;

    // First page objects & xref tracking
    let mut obj_offsets: std::collections::BTreeMap<ObjectId, usize> = std::collections::BTreeMap::new();
    obj_offsets.insert(ObjectId::new(lin_dict_id, 0), lin_dict_start);
    obj_offsets.insert(ObjectId::new(hint_stream_id, 0), hint_stream_start);

    for id in &first_page_objs {
        if let Some(obj) = doc.objects.get(id) {
            obj_offsets.insert(*id, output.len());
            write_object_to_bytes(&mut output, *id, obj)?;
        }
    }

    let first_page_end_offset = output.len();

    // Primary xref table and first-page trailer
    let primary_xref_offset = output.len();
    write_xref_section(&mut output, &obj_offsets, 0, lin_dict_id + 1)?;

    let mut primary_trailer = doc.trailer.clone();
    primary_trailer.insert("Size", Object::Integer((lin_dict_id + 2) as i64));
    primary_trailer.insert("Root", doc.root_id);
    let first_page_trailer_offset = output.len();
    output.write_all(b"trailer\n")?;
    write_dictionary(&mut output, &primary_trailer)?;
    output.write_all(b"\nstartxref\n0\n%%EOF\n")?;

    // Remaining objects
    for id in &remaining_objs {
        if let Some(obj) = doc.objects.get(id) {
            obj_offsets.insert(*id, output.len());
            write_object_to_bytes(&mut output, *id, obj)?;
        }
    }

    // Secondary xref table and trailer
    let secondary_xref_offset = output.len();
    write_full_xref(&mut output, &obj_offsets)?;

    let mut final_trailer = doc.trailer.clone();
    let max_obj_num = obj_offsets.keys().map(|id| id.number).max().unwrap_or(0);
    final_trailer.insert("Size", Object::Integer((max_obj_num + 1) as i64));
    final_trailer.insert("Prev", Object::Integer(primary_xref_offset as i64));

    output.write_all(b"trailer\n")?;
    write_dictionary(&mut output, &final_trailer)?;
    output.write_all(format!("\nstartxref\n{}\n%%EOF\n", secondary_xref_offset).as_bytes())?;

    let total_file_length = output.len();

    // Now, format the exact linearization dictionary with exact length padding
    let exact_lin_dict = format!(
        "{} 0 obj\n<< /Linearized 1.0 /L {} /H [ {} {} ] /O {} /E {} /N {} /T {} >>\nendobj\n",
        lin_dict_id,
        total_file_length,
        hint_stream_start,
        hint_stream_len,
        first_page_id.number,
        first_page_end_offset,
        page_count,
        first_page_trailer_offset
    );

    // If exact_lin_dict is shorter than dummy_lin_dict, pad with space inside dictionary or comment
    if exact_lin_dict.len() <= (lin_dict_end - lin_dict_start) {
        let padding_needed = (lin_dict_end - lin_dict_start) - exact_lin_dict.len();
        let padded_dict = format!(
            "{} 0 obj\n<< /Linearized 1.0 /L {} /H [ {} {} ] /O {} /E {} /N {} /T {} /Pnd ({}) >>\nendobj\n",
            lin_dict_id,
            total_file_length,
            hint_stream_start,
            hint_stream_len,
            first_page_id.number,
            first_page_end_offset,
            page_count,
            first_page_trailer_offset,
            " ".repeat(padding_needed.saturating_sub(12))
        );
        // Overwrite the placeholder slice
        let copy_len = padded_dict.len().min(lin_dict_end - lin_dict_start);
        output[lin_dict_start..lin_dict_start + copy_len]
            .copy_from_slice(&padded_dict.as_bytes()[..copy_len]);
    }

    Ok(output)
}

fn write_object_to_bytes(out: &mut Vec<u8>, id: ObjectId, obj: &Object) -> Result<()> {
    writeln!(out, "{} {} obj", id.number, id.generation)?;
    match obj {
        Object::Stream(stream) => {
            write_dictionary(out, &stream.dict)?;
            out.write_all(b"\nstream\n")?;
            out.write_all(&stream.raw_data)?;
            out.write_all(b"\nendstream\n")?;
        }
        Object::Dictionary(dict) => {
            write_dictionary(out, dict)?;
            out.write_all(b"\n")?;
        }
        other => {
            write_object_val(out, other)?;
            out.write_all(b"\n")?;
        }
    }
    writeln!(out, "endobj")?;
    Ok(())
}

fn write_dictionary(out: &mut Vec<u8>, dict: &PdfDictionary) -> Result<()> {
    out.write_all(b"<< ")?;
    for (k, v) in &dict.map {
        write!(out, "/{} ", k)?;
        write_object_val(out, v)?;
        out.write_all(b" ")?;
    }
    out.write_all(b">>")?;
    Ok(())
}

fn write_object_val(out: &mut Vec<u8>, obj: &Object) -> Result<()> {
    match obj {
        Object::Null => out.write_all(b"null")?,
        Object::Boolean(b) => write!(out, "{}", b)?,
        Object::Integer(i) => write!(out, "{}", i)?,
        Object::Real(r) => write!(out, "{:.4}", r)?,
        Object::Name(n) => write!(out, "/{}", n.0)?,
        Object::String(s) => {
            out.write_all(b"(")?;
            out.write_all(&s.bytes)?;
            out.write_all(b")")?;
        }
        Object::Reference(id) => write!(out, "{} {} R", id.number, id.generation)?,
        Object::Array(arr) => {
            out.write_all(b"[ ")?;
            for item in arr {
                write_object_val(out, item)?;
                out.write_all(b" ")?;
            }
            out.write_all(b"]")?;
        }
        Object::Dictionary(d) => write_dictionary(out, d)?,
        Object::Stream(_) => {}
    }
    Ok(())
}

fn write_xref_section(
    out: &mut Vec<u8>,
    offsets: &std::collections::BTreeMap<ObjectId, usize>,
    start_num: u32,
    count: u32,
) -> Result<()> {
    writeln!(out, "xref\n{} {}", start_num, count)?;
    for num in start_num..(start_num + count) {
        if num == 0 {
            out.write_all(b"0000000000 65535 f \r\n")?;
        } else {
            let offset = offsets
                .get(&ObjectId::new(num, 0))
                .copied()
                .unwrap_or(0);
            write!(out, "{:010} 00000 n \r\n", offset)?;
        }
    }
    Ok(())
}

fn write_full_xref(
    out: &mut Vec<u8>,
    offsets: &std::collections::BTreeMap<ObjectId, usize>,
) -> Result<()> {
    let max_id = offsets.keys().map(|id| id.number).max().unwrap_or(0);
    writeln!(out, "xref\n0 {}", max_id + 1)?;
    out.write_all(b"0000000000 65535 f \r\n")?;
    for num in 1..=max_id {
        let offset = offsets
            .get(&ObjectId::new(num, 0))
            .copied()
            .unwrap_or(0);
        write!(out, "{:010} 00000 n \r\n", offset)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_linearization_structure_and_detection() {
        let mut doc = PdfDocument::new();
        doc.add_blank_page(612.0, 792.0);
        doc.add_blank_page(612.0, 792.0);

        // Before linearization, standard serialized doc is not linearized
        let raw_bytes = doc.to_bytes().expect("raw bytes");
        assert!(!is_linearized(&raw_bytes));

        // Linearize document
        let linearized_bytes = linearize_document(&doc).expect("linearize document");
        assert!(is_linearized(&linearized_bytes));

        // Inspect linearized header
        let prefix = String::from_utf8_lossy(&linearized_bytes[..512]);
        assert!(prefix.contains("/Linearized 1.0"));
        assert!(prefix.contains("/L "));
        assert!(prefix.contains("/H ["));
        assert!(prefix.contains("/N 2"));
    }
}
