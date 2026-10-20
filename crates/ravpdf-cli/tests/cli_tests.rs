use ravpdf_core::document::{paper_sizes, PdfDocument};
use ravpdf_crypto::{decrypt_document, encrypt_document, Permissions};
use ravpdf_edit::{merge_documents, rotate_page, split_document};
use ravpdf_forms::{add_form_field, flatten_form, get_form_fields, FormField};
use ravpdf_render::{pixmap_to_png, render_page, RenderOptions};
use ravpdf_tools::{
    apply_bates_numbering, apply_text_watermark, extract_all_text, get_document_info,
    redact_pattern, search_document, set_document_info, DocumentInfo,
};

#[test]
fn test_full_document_lifecycle() {
    // 1. Create document conforming to ISO 32000
    let mut doc = PdfDocument::new();
    let content = b"BT\n/F1 14 Tf\n100 700 Td\n(Account Number: 123-45-6789) Tj\nET\n";
    doc.add_page_with_content(paper_sizes::LETTER.0, paper_sizes::LETTER.1, content.to_vec());
    doc.add_blank_page(paper_sizes::A4.0, paper_sizes::A4.1);

    assert_eq!(doc.page_count(), 2);

    // 2. Set Metadata
    let mut info = DocumentInfo::default();
    info.title = Some("Billing Statement".into());
    info.author = Some("Finance Dept".into());
    set_document_info(&mut doc, &info).expect("set info");

    let read_info = get_document_info(&doc).expect("read info");
    assert_eq!(read_info.title.as_deref(), Some("Billing Statement"));

    // 3. Search text
    let matches = search_document(&doc, r"\d{3}-\d{2}-\d{4}", true, false).expect("search");
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].match_text, "123-45-6789");

    // 4. Redact pattern (FR-15)
    let redactions = redact_pattern(&mut doc, r"\d{3}-\d{2}-\d{4}", "XXX-XX-XXXX").expect("redact");
    assert_eq!(redactions, 1);

    let text_after = extract_all_text(&doc).expect("extract text");
    assert!(text_after.contains("XXX-XX-XXXX"));
    assert!(!text_after.contains("123-45-6789"));

    // 5. Watermark & Bates Numbering (FR-18)
    apply_text_watermark(&mut doc, "AUDITED", 0.35, 45.0, 50.0).expect("watermark");
    apply_bates_numbering(&mut doc, "STMT", 500, 6, "").expect("bates");

    // 6. Form fields & Flattening (FR-11)
    let mut field = FormField::new_text("CustomerSignature", [100.0, 100.0, 300.0, 120.0]);
    field.value = Some("Signed By User".into());
    add_form_field(&mut doc, 0, &field).expect("add field");
    assert_eq!(get_form_fields(&doc).unwrap().len(), 1);

    flatten_form(&mut doc).expect("flatten");
    assert_eq!(get_form_fields(&doc).unwrap().len(), 0);

    // 7. Page Operations (FR-06, FR-07)
    rotate_page(&mut doc, 0, 90).expect("rotate");
    let split_docs = split_document(&doc, &[(0, 0), (1, 1)]).expect("split");
    assert_eq!(split_docs.len(), 2);

    let re_merged = merge_documents(&split_docs).expect("merge");
    assert_eq!(re_merged.page_count(), 2);

    // 8. Render to image (FR-01)
    let options = RenderOptions::default();
    let pixmap = render_page(&re_merged, 0, &options).expect("render");
    let png = pixmap_to_png(&pixmap).expect("png");
    assert!(png.starts_with(b"\x89PNG\r\n\x1a\n"));

    // 9. Encryption & Decryption (FR-14)
    let mut enc_doc = re_merged.clone();
    encrypt_document(&mut enc_doc, "userpass", "ownerpass", Permissions::all_permitted()).expect("encrypt");
    assert!(enc_doc.trailer.contains_key("Encrypt"));

    decrypt_document(&mut enc_doc, "userpass").expect("decrypt");
    assert!(!enc_doc.trailer.contains_key("Encrypt"));

    // 10. Round-trip binary serialization and reload (ISO 32000 Conformance)
    let bytes = enc_doc.to_bytes().expect("serialize");
    assert!(bytes.starts_with(b"%PDF-1.7\n"));
    let reloaded = PdfDocument::load_from_bytes(&bytes).expect("reload");
    assert_eq!(reloaded.page_count(), 2);
}

#[test]
fn test_accessibility_export_and_comments_workflows() {
    use ravpdf_annot::{add_annotation_to_page, Annotation, AnnotationType};
    use ravpdf_tools::{
        audit_accessibility, export_comments_to_csv, export_page_to_image, export_to_html,
        export_to_plain_text, get_document_comments, remediate_all_automatic,
        ExportImageFormat,
    };

    let mut doc = PdfDocument::new();
    let content = b"BT\n/F1 14 Tf\n50 700 Td\n(Accessibility & Export Test Report) Tj\nET\n";
    doc.add_page_with_content(paper_sizes::LETTER.0, paper_sizes::LETTER.1, content.to_vec());

    // 1. Accessibility Audit (FR-21.1)
    let audit_before = audit_accessibility(&doc).expect("audit before");
    assert!(!audit_before.is_compliant);

    // 2. Auto-Remediation (FR-21.3 - FR-21.10)
    let audit_after = remediate_all_automatic(&mut doc, "Compliant Report", "en-US").expect("remediate");
    assert_eq!(audit_after.failed_count, 0);
    assert!(audit_after.is_compliant);

    // 3. Export to Plain Text (FR-03.2.5)
    let plain_text = export_to_plain_text(&doc, None).expect("export text");
    assert!(plain_text.contains("Accessibility & Export Test Report"));
    assert!(plain_text.contains("--- Page 1 of 1 ---"));

    // 4. Export to HTML (FR-03.2.6)
    let html = export_to_html(&doc, None).expect("export html");
    assert!(html.contains("<!DOCTYPE html>"));
    assert!(html.contains("Compliant Report"));
    assert!(html.contains("Accessibility &amp; Export Test Report"));

    // 5. Export to PNG image (FR-03.2.4)
    let png = export_page_to_image(&doc, 0, 150.0, ExportImageFormat::Png).expect("export png");
    assert!(png.starts_with(b"\x89PNG\r\n\x1a\n"));

    // 6. Review Comments & CSV (FR-31.5)
    let mut note = Annotation::new(AnnotationType::Text, [100.0, 500.0, 130.0, 530.0]);
    note.contents = Some("Approved with minor revisions.".into());
    note.author = Some("Accessibility Reviewer".into());
    add_annotation_to_page(&mut doc, 0, &note).expect("add note");

    let comments = get_document_comments(&doc).expect("get comments");
    assert_eq!(comments.len(), 1);
    assert_eq!(comments[0].author, "Accessibility Reviewer");

    let csv = export_comments_to_csv(&comments);
    assert!(csv.contains("Accessibility Reviewer"));
    assert!(csv.contains("Approved with minor revisions."));
}

