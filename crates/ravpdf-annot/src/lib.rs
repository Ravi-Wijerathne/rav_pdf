//! RavPDF Annotation and Markup Subsystem conforming to ISO 32000-1 Section 12.5.
//!
//! Traceability:
//! - FR-08 (Annotation and Markup Tools: sticky notes, text boxes, highlights, strikeout)
//! - FR-09 (Drawing and Shape Tools: lines, rectangles, circles, ink)
//! - FR-10 (Stamps: rubber stamps, approval marks)

pub mod annotation;
pub mod drawing;
pub mod stamps;

pub use annotation::{
    add_annotation_to_page, delete_annotation, detect_urls_in_text, get_page_annotations,
    update_annotation_color, update_annotation_contents, update_annotation_rect, Annotation,
    AnnotationType, LinkAction, LinkHighlight,
};
pub use drawing::{
    add_ink_annotation, add_shape_annotation, erase_ink_strokes, set_annotation_locked,
    snap_angle_orthogonal, snap_node_to_points, BorderEffect, BorderStyle, InkAnnotationConfig,
    LineEndingStyle, ShapeAnnotationConfig, ShapeType,
};
pub use stamps::{
    add_callout_annotation, add_stamp_annotation, add_typewriter_annotation, CalloutConfig,
    StampConfig, StandardStamp, TypewriterConfig,
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

    #[test]
    fn test_annotation_lifecycle_update_and_delete() {
        let mut doc = PdfDocument::new();
        doc.add_blank_page(612.0, 792.0);

        let mut sticky = Annotation::new(AnnotationType::Text, [100.0, 100.0, 150.0, 150.0]);
        sticky.contents = Some("Original note".into());
        sticky.author = Some("Alice".into());
        sticky.color = Some((1.0, 0.0, 0.0));
        let annot_id = add_annotation_to_page(&mut doc, 0, &sticky).expect("add annotation");

        // 1. Verify initially created
        let page_annots = get_page_annotations(&doc, 0).expect("get annots");
        assert_eq!(page_annots.len(), 1);
        assert_eq!(page_annots[0].contents.as_deref(), Some("Original note"));

        // 2. Update rectangle
        update_annotation_rect(&mut doc, annot_id, [200.0, 200.0, 260.0, 260.0]).expect("update rect");
        let updated_annots = get_page_annotations(&doc, 0).expect("get annots");
        assert_eq!(updated_annots[0].rect, [200.0, 200.0, 260.0, 260.0]);

        // 3. Update contents & author
        update_annotation_contents(&mut doc, annot_id, "Updated note text", Some("Bob")).expect("update content");
        let content_annots = get_page_annotations(&doc, 0).expect("get annots");
        assert_eq!(content_annots[0].contents.as_deref(), Some("Updated note text"));
        assert_eq!(content_annots[0].author.as_deref(), Some("Bob"));

        // 4. Update color & opacity
        update_annotation_color(&mut doc, annot_id, (0.0, 0.5, 1.0), Some(0.8)).expect("update color");
        let color_annots = get_page_annotations(&doc, 0).expect("get annots");
        assert_eq!(color_annots[0].opacity, 0.8);

        // 5. Delete annotation
        delete_annotation(&mut doc, 0, annot_id).expect("delete annotation");
        let remaining_annots = get_page_annotations(&doc, 0).expect("get annots");
        assert_eq!(remaining_annots.len(), 0);
        assert!(!doc.objects.contains_key(&annot_id));
    }
}
