//! RavPDF Annotation and Markup Subsystem conforming to ISO 32000-1 Section 12.5.
//!
//! Traceability:
//! - FR-08 (Annotation and Markup Tools: sticky notes, text boxes, highlights, strikeout)
//! - FR-09 (Drawing and Shape Tools: lines, rectangles, circles, ink)
//! - FR-10 (Stamps: rubber stamps, approval marks)

pub mod annotation;

pub use annotation::{
    add_annotation_to_page, detect_urls_in_text, get_page_annotations, Annotation, AnnotationType,
    LinkAction, LinkHighlight,
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

    #[test]
    fn test_link_annotation_and_url_detection() {
        let mut doc = PdfDocument::new();
        doc.add_blank_page(612.0, 792.0);
        doc.add_blank_page(612.0, 792.0);

        // 1. Web URL Link
        let link_uri = Annotation::new_link_uri([100.0, 500.0, 200.0, 520.0], "https://example.com/spec");
        let uri_id = add_annotation_to_page(&mut doc, 0, &link_uri).expect("add uri link");
        assert!(doc.objects.contains_key(&uri_id));

        // 2. GoTo Page Link
        let link_goto = Annotation::new_link_goto([100.0, 450.0, 200.0, 470.0], 1);
        let goto_id = add_annotation_to_page(&mut doc, 0, &link_goto).expect("add goto link");
        assert!(doc.objects.contains_key(&goto_id));

        let retrieved = get_page_annotations(&doc, 0).expect("get annotations");
        assert_eq!(retrieved.len(), 2);

        let link1 = &retrieved[0];
        match &link1.annot_type {
            AnnotationType::Link { action, highlight, .. } => {
                assert_eq!(action, &LinkAction::Uri("https://example.com/spec".into()));
                assert_eq!(*highlight, LinkHighlight::Invert);
            }
            _ => panic!("Expected link annotation"),
        }

        let link2 = &retrieved[1];
        match &link2.annot_type {
            AnnotationType::Link { action, .. } => {
                assert_eq!(action, &LinkAction::GoToPage(1));
            }
            _ => panic!("Expected goto link annotation"),
        }

        // Test URL regex detector (FR-20.5)
        let sample_text = "Please check our docs at https://ravpdf.org/docs and github at https://github.com/rust-lang/rust.";
        let urls = detect_urls_in_text(sample_text);
        assert_eq!(urls.len(), 2);
        assert_eq!(urls[0].0, "https://ravpdf.org/docs");
        assert_eq!(urls[1].0, "https://github.com/rust-lang/rust");
    }
}
