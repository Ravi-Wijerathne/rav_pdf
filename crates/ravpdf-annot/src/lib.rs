//! RavPDF Annotation and Markup Subsystem conforming to ISO 32000-1 Section 12.5.
//!
//! Traceability:
//! - FR-08 (Annotation and Markup Tools: sticky notes, text boxes, highlights, strikeout)
//! - FR-09 (Drawing and Shape Tools: lines, rectangles, circles, ink)
//! - FR-10 (Stamps: rubber stamps, approval marks)

pub mod annotation;

pub use annotation::{
    add_annotation_to_page, get_page_annotations, Annotation, AnnotationType,
};

#[cfg(test)]
mod tests {
    use super::*;
    use ravpdf_core::document::PdfDocument;

    #[test]
    fn test_add_and_retrieve_annotations() {
        let mut doc = PdfDocument::new();
        doc.add_blank_page(612.0, 792.0);

        let mut sticky_note =
            Annotation::new(AnnotationType::Text, [50.0, 700.0, 80.0, 730.0]);
        sticky_note.contents = Some("Review comments on section 1".into());
        sticky_note.author = Some("Auditor".into());

        let annot_id =
            add_annotation_to_page(&mut doc, 0, &sticky_note).expect("add annot");
        assert!(doc.objects.contains_key(&annot_id));

        let annots = get_page_annotations(&doc, 0).expect("get annots");
        assert_eq!(annots.len(), 1);
        assert_eq!(annots[0].contents.as_deref(), Some("Review comments on section 1"));
    }
}
