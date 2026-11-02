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
    add_annotation_to_page, delete_annotation, detect_urls_in_text, export_annotations_to_xfdf,
    get_page_annotations, import_annotations_from_xfdf, update_annotation_color,
    update_annotation_contents, update_annotation_rect, Annotation, AnnotationType, LinkAction,
    LinkHighlight,
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

        let mut sticky_note = Annotation::new(AnnotationType::Text, [50.0, 700.0, 80.0, 730.0]);
        sticky_note.contents = Some("Review comments on section 1".into());
        sticky_note.author = Some("Auditor".into());

        let annot_id = add_annotation_to_page(&mut doc, 0, &sticky_note).expect("add annot");
        assert!(doc.objects.contains_key(&annot_id));

        let annots = get_page_annotations(&doc, 0).expect("get annots");
        assert_eq!(annots.len(), 1);
        assert_eq!(
            annots[0].contents.as_deref(),
            Some("Review comments on section 1")
        );
    }

    #[test]
    fn test_link_annotation_and_url_detection() {
        let mut doc = PdfDocument::new();
        doc.add_blank_page(612.0, 792.0);
        doc.add_blank_page(612.0, 792.0);

        // 1. Web URL Link
        let link_uri =
            Annotation::new_link_uri([100.0, 500.0, 200.0, 520.0], "https://example.com/spec");
        let uri_id = add_annotation_to_page(&mut doc, 0, &link_uri).expect("add uri link");
        assert!(doc.objects.contains_key(&uri_id));

        // 2. GoTo Page Link
        let link_goto = Annotation::new_link_goto([100.0, 450.0, 200.0, 470.0], 1);
        let goto_id = add_annotation_to_page(&mut doc, 0, &link_goto).expect("add goto link");
        assert!(doc.objects.contains_key(&goto_id));

        // 3. Named Destination Link (FR-20.2, FR-20.4)
        let link_named = Annotation::new_link_named([100.0, 400.0, 200.0, 420.0], "Chapter1");
        add_annotation_to_page(&mut doc, 0, &link_named).expect("add named link");

        // 4. Launch File Link (FR-20.2)
        let link_launch = Annotation::new_link_launch([100.0, 350.0, 200.0, 370.0], "data/report.xlsx");
        add_annotation_to_page(&mut doc, 0, &link_launch).expect("add launch link");

        // 5. JavaScript Link (FR-20.2, FR-12)
        let link_js = Annotation::new_link_javascript([100.0, 300.0, 200.0, 320.0], "app.alert('Hello');");
        add_annotation_to_page(&mut doc, 0, &link_js).expect("add js link");

        // 6. Submit Form Link (FR-20.2)
        let link_submit = Annotation::new_link_submit_form([100.0, 250.0, 200.0, 270.0], "https://server.com/submit", 4);
        add_annotation_to_page(&mut doc, 0, &link_submit).expect("add submit link");

        // 7. Reset Form Link (FR-20.2)
        let link_reset = Annotation::new_link_reset_form([100.0, 200.0, 200.0, 220.0], vec!["Field1".into()], false);
        add_annotation_to_page(&mut doc, 0, &link_reset).expect("add reset link");

        let retrieved = get_page_annotations(&doc, 0).expect("get annotations");
        assert_eq!(retrieved.len(), 7);

        let link1 = &retrieved[0];
        match &link1.annot_type {
            AnnotationType::Link {
                action, highlight, ..
            } => {
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

        let link3 = &retrieved[2];
        match &link3.annot_type {
            AnnotationType::Link { action, .. } => {
                assert_eq!(action, &LinkAction::Named("Chapter1".into()));
            }
            _ => panic!("Expected named link annotation"),
        }

        let link4 = &retrieved[3];
        match &link4.annot_type {
            AnnotationType::Link { action, .. } => {
                assert_eq!(action, &LinkAction::OpenFile("data/report.xlsx".into()));
            }
            _ => panic!("Expected launch link annotation"),
        }

        let link5 = &retrieved[4];
        match &link5.annot_type {
            AnnotationType::Link { action, .. } => {
                assert_eq!(action, &LinkAction::JavaScript("app.alert('Hello');".into()));
            }
            _ => panic!("Expected js link annotation"),
        }

        let link6 = &retrieved[5];
        match &link6.annot_type {
            AnnotationType::Link { action, .. } => {
                assert_eq!(action, &LinkAction::SubmitForm { url: "https://server.com/submit".into(), flags: 4 });
            }
            _ => panic!("Expected submit link annotation"),
        }

        let link7 = &retrieved[6];
        match &link7.annot_type {
            AnnotationType::Link { action, .. } => {
                assert_eq!(action, &LinkAction::ResetForm { fields: vec!["Field1".into()], exclude: false });
            }
            _ => panic!("Expected reset link annotation"),
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
        update_annotation_rect(&mut doc, annot_id, [200.0, 200.0, 260.0, 260.0])
            .expect("update rect");
        let updated_annots = get_page_annotations(&doc, 0).expect("get annots");
        assert_eq!(updated_annots[0].rect, [200.0, 200.0, 260.0, 260.0]);

        // 3. Update contents & author
        update_annotation_contents(&mut doc, annot_id, "Updated note text", Some("Bob"))
            .expect("update content");
        let content_annots = get_page_annotations(&doc, 0).expect("get annots");
        assert_eq!(
            content_annots[0].contents.as_deref(),
            Some("Updated note text")
        );
        assert_eq!(content_annots[0].author.as_deref(), Some("Bob"));

        // 4. Update color & opacity
        update_annotation_color(&mut doc, annot_id, (0.0, 0.5, 1.0), Some(0.8))
            .expect("update color");
        let color_annots = get_page_annotations(&doc, 0).expect("get annots");
        assert_eq!(color_annots[0].opacity, 0.8);

        // 5. Delete annotation
        delete_annotation(&mut doc, 0, annot_id).expect("delete annotation");
        let remaining_annots = get_page_annotations(&doc, 0).expect("get annots");
        assert_eq!(remaining_annots.len(), 0);
        assert!(!doc.objects.contains_key(&annot_id));
    }

    #[test]
    fn test_xfdf_export_and_import() {
        let mut doc = PdfDocument::new();
        doc.add_blank_page(612.0, 792.0);

        let mut annot = Annotation::new(AnnotationType::Highlight, [100.0, 200.0, 300.0, 220.0]);
        annot.contents = Some("Important section".into());
        annot.author = Some("Alice".into());
        annot.color = Some((1.0, 1.0, 0.0));
        annot.opacity = 0.85;

        add_annotation_to_page(&mut doc, 0, &annot).expect("add annot");

        // Export to XFDF
        let xfdf = export_annotations_to_xfdf(&doc).expect("export xfdf");
        assert!(xfdf.contains("<highlight"));
        assert!(xfdf.contains("Alice"));
        assert!(xfdf.contains("Important section"));

        // Import into fresh document
        let mut doc2 = PdfDocument::new();
        doc2.add_blank_page(612.0, 792.0);
        let imported_count = import_annotations_from_xfdf(&mut doc2, &xfdf).expect("import xfdf");
        assert_eq!(imported_count, 1);

        let imported_annots = get_page_annotations(&doc2, 0).expect("get annots");
        assert_eq!(imported_annots.len(), 1);
        assert_eq!(
            imported_annots[0].contents.as_deref(),
            Some("Important section")
        );
        assert_eq!(imported_annots[0].author.as_deref(), Some("Alice"));
    }
}
