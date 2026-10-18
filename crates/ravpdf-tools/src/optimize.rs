//! PDF Optimization, Size Reduction, and Space Usage Audit.
//!
//! Traceability:
//! - FR-25.1 (One-click Reduce File Size)
//! - FR-25.6 (Discard unused objects / unreferenced objects)
//! - FR-25.8 (Consolidate structure and compress streams)
//! - FR-25.9 (Audit Space Usage by component)

use anyhow::Result;
use ravpdf_core::document::PdfDocument;
use ravpdf_core::object::{Object, ObjectId};
use serde::{Deserialize, Serialize};
use std::collections::{HashSet, VecDeque};

/// Space usage breakdown of a PDF document (FR-25.9).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SpaceAudit {
    pub total_bytes: usize,
    pub content_stream_bytes: usize,
    pub image_bytes: usize,
    pub font_bytes: usize,
    pub metadata_bytes: usize,
    pub structure_bytes: usize,
}

impl SpaceAudit {
    pub fn content_percent(&self) -> f64 {
        if self.total_bytes == 0 { 0.0 } else { (self.content_stream_bytes as f64 / self.total_bytes as f64) * 100.0 }
    }

    pub fn image_percent(&self) -> f64 {
        if self.total_bytes == 0 { 0.0 } else { (self.image_bytes as f64 / self.total_bytes as f64) * 100.0 }
    }

    pub fn font_percent(&self) -> f64 {
        if self.total_bytes == 0 { 0.0 } else { (self.font_bytes as f64 / self.total_bytes as f64) * 100.0 }
    }

    pub fn metadata_percent(&self) -> f64 {
        if self.total_bytes == 0 { 0.0 } else { (self.metadata_bytes as f64 / self.total_bytes as f64) * 100.0 }
    }

    pub fn structure_percent(&self) -> f64 {
        if self.total_bytes == 0 { 0.0 } else { (self.structure_bytes as f64 / self.total_bytes as f64) * 100.0 }
    }
}

/// Results and metrics from optimizing a PDF document (FR-25.1).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OptimizationStats {
    pub original_bytes: usize,
    pub optimized_bytes: usize,
    pub streams_compressed: usize,
    pub objects_removed: usize,
}

impl OptimizationStats {
    pub fn saved_bytes(&self) -> usize {
        self.original_bytes.saturating_sub(self.optimized_bytes)
    }

    pub fn reduction_percent(&self) -> f64 {
        if self.original_bytes == 0 {
            0.0
        } else {
            (self.saved_bytes() as f64 / self.original_bytes as f64) * 100.0
        }
    }
}

/// Recursively discovers all `ObjectId` references within an `Object`.
fn collect_references(obj: &Object, refs: &mut Vec<ObjectId>) {
    match obj {
        Object::Reference(id) => refs.push(*id),
        Object::Array(arr) => {
            for item in arr {
                collect_references(item, refs);
            }
        }
        Object::Dictionary(dict) => {
            for val in dict.map.values() {
                collect_references(val, refs);
            }
        }
        Object::Stream(stream) => {
            for val in stream.dict.map.values() {
                collect_references(val, refs);
            }
        }
        _ => {}
    }
}

/// Audits the space usage breakdown of a PDF document (FR-25.9).
pub fn audit_space_usage(doc: &PdfDocument) -> SpaceAudit {
    let mut audit = SpaceAudit::default();
    let doc_bytes = doc.to_bytes().unwrap_or_default();
    audit.total_bytes = doc_bytes.len();

    let mut stream_total = 0;

    for (id, obj) in &doc.objects {
        match obj {
            Object::Stream(s) => {
                let len = s.raw_data.len();
                stream_total += len;
                let subtype = s.dict.get_name("Subtype").unwrap_or_default();
                let obj_type = s.dict.get_name("Type").unwrap_or_default();

                if subtype.eq_ignore_ascii_case("Image") {
                    audit.image_bytes += len;
                } else if obj_type.eq_ignore_ascii_case("Font")
                    || subtype.eq_ignore_ascii_case("Type1C")
                    || subtype.eq_ignore_ascii_case("CIDFontType0C")
                {
                    audit.font_bytes += len;
                } else if Some(*id) == doc.info_id {
                    audit.metadata_bytes += len;
                } else {
                    audit.content_stream_bytes += len;
                }
            }
            Object::Dictionary(d)
                if Some(*id) == doc.info_id || d.get_name("Type").unwrap_or_default().eq_ignore_ascii_case("Metadata") =>
            {
                audit.metadata_bytes += 256; // estimated dictionary footprint
            }
            _ => {}
        }
    }

    audit.structure_bytes = audit.total_bytes.saturating_sub(stream_total + audit.metadata_bytes);
    audit
}

/// Optimizes a PDF document by compressing uncompressed streams and eliminating dead/unreachable objects (FR-25.1, FR-25.6, FR-25.8).
pub fn optimize_document(doc: &mut PdfDocument) -> Result<OptimizationStats> {
    let original_bytes = doc.to_bytes()?.len();
    let mut streams_compressed = 0;

    // 1. Compress any uncompressed streams with FlateDecode (FR-25.8)
    for obj in doc.objects.values_mut() {
        if let Object::Stream(s) = obj {
            if s.dict.get("Filter").is_none() && s.compress_flate().is_ok() {
                streams_compressed += 1;
            }
        }
    }

    // 2. Discover reachable objects starting from root catalog, info dictionary, and trailer (FR-25.6 Garbage Collection)
    let mut reachable = HashSet::new();
    let mut queue = VecDeque::new();

    reachable.insert(doc.root_id);
    queue.push_back(doc.root_id);

    if let Some(info_id) = doc.info_id {
        reachable.insert(info_id);
        queue.push_back(info_id);
    }

    // Traverse references in trailer
    for val in doc.trailer.map.values() {
        let mut refs = Vec::new();
        collect_references(val, &mut refs);
        for r in refs {
            if reachable.insert(r) {
                queue.push_back(r);
            }
        }
    }

    while let Some(curr_id) = queue.pop_front() {
        if let Some(obj) = doc.objects.get(&curr_id) {
            let mut refs = Vec::new();
            collect_references(obj, &mut refs);
            for r in refs {
                if reachable.insert(r) {
                    queue.push_back(r);
                }
            }
        }
    }

    // 3. Remove unreachable / orphaned objects (FR-25.6)
    let before_count = doc.objects.len();
    doc.objects.retain(|id, _| reachable.contains(id));
    let objects_removed = before_count.saturating_sub(doc.objects.len());

    let optimized_bytes = doc.to_bytes()?.len();

    Ok(OptimizationStats {
        original_bytes,
        optimized_bytes,
        streams_compressed,
        objects_removed,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use ravpdf_core::object::{PdfDictionary, PdfName, PdfStream};

    #[test]
    fn test_space_audit_and_optimization() {
        let mut doc = PdfDocument::new();
        doc.add_blank_page(612.0, 792.0);

        // Add an uncompressed plain stream object attached to page 0 Contents
        let uncompressed_data = b"q 1 0 0 1 50 750 cm /F1 24 Tf (Hello Uncompressed World!) Tj Q ".repeat(20);
        let stream_obj = Object::Stream(PdfStream::with_plain_data(
            PdfDictionary::new(),
            uncompressed_data.clone(),
        ));
        let stream_id = ObjectId::new(100, 0);
        doc.objects.insert(stream_id, stream_obj);
        let page_id = doc.page_ids()[0];
        if let Some(Object::Dictionary(page_dict)) = doc.objects.get_mut(&page_id) {
            page_dict.insert("Contents", stream_id);
        }

        // Add an unreachable orphan object
        let orphan_id = ObjectId::new(999, 0);
        let mut orphan_dict = PdfDictionary::new();
        orphan_dict.insert("Type", PdfName::new("OrphanData"));
        doc.objects.insert(orphan_id, Object::Dictionary(orphan_dict));

        // Audit before optimization
        let audit_before = audit_space_usage(&doc);
        assert!(audit_before.total_bytes > 0);

        // Run optimization
        let stats = optimize_document(&mut doc).expect("optimize document");
        assert_eq!(stats.objects_removed, 1);
        assert_eq!(stats.streams_compressed, 1);
        assert!(stats.optimized_bytes < stats.original_bytes);
        assert!(stats.reduction_percent() > 0.0);

        // Confirm orphan is gone
        assert!(!doc.objects.contains_key(&orphan_id));
        // Confirm stream is compressed
        if let Some(Object::Stream(s)) = doc.objects.get(&stream_id) {
            assert_eq!(s.dict.get_name("Filter"), Some("FlateDecode"));
        } else {
            panic!("Stream missing");
        }
    }
}
