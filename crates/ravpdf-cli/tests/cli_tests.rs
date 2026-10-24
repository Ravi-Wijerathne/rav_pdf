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

#[test]
fn test_measure_portfolio_and_linearization_workflows() {
    use ravpdf_tools::{
        add_measurement_line_annotation, calculate_distance, create_portfolio,
        extract_portfolio_item, get_portfolio, is_linearized, is_portfolio,
        linearize_document, optimize_document, search_portfolio, LinearUnit,
        PortfolioItem, PortfolioLayout, ScaleRatio,
    };

    // 1. Measurement Engine (FR-26.1, FR-26.4, FR-26.5)
    let mut doc = PdfDocument::new();
    doc.add_blank_page(612.0, 792.0);

    // Architectural scale: 1 in = 10 ft
    let scale = ScaleRatio::architectural_inch_to_feet(10.0);
    // Measure 2 inches (144 pt) = 20 feet
    let dist = calculate_distance([50.0, 100.0], [194.0, 100.0], &scale, LinearUnit::Feet);
    assert_eq!(dist.points_distance, 144.0);
    assert!((dist.world_distance - 20.0).abs() < 1e-4);
    assert_eq!(dist.formatted, "20.00 ft");

    // Add measurement line annotation to page (FR-26.4)
    let annot_id = add_measurement_line_annotation(&mut doc, 0, &dist, (0.0, 0.45, 0.85))
        .expect("add measurement annot");
    assert!(doc.objects.contains_key(&annot_id));

    // 2. Linearization / Fast Web View (FR-02.8, FR-25.8)
    assert!(!is_linearized(&doc.to_bytes().unwrap()));
    let lin_bytes = linearize_document(&doc).expect("linearize");
    assert!(is_linearized(&lin_bytes));

    // 3. Optimization (FR-25.1)
    let stats = optimize_document(&mut doc).expect("optimize doc");
    assert!(stats.optimized_bytes > 0);

    // 4. Portfolio Studio (FR-29.2.1 - FR-29.2.5)
    let sample_csv = PortfolioItem {
        filename: "dataset.csv".into(),
        description: Some("Sensor Data".into()),
        size: 26,
        data: b"timestamp,temp\n1000,23.5\n".to_vec(),
        mime_type: Some("text/csv".into()),
    };
    let sample_notes = PortfolioItem {
        filename: "notes.txt".into(),
        description: Some("Field Notes".into()),
        size: 19,
        data: b"Calibration passed.".to_vec(),
        mime_type: Some("text/plain".into()),
    };

    let port_doc = create_portfolio(
        &[sample_csv.clone(), sample_notes.clone()],
        PortfolioLayout::Tile,
        "Field Inspection Portfolio",
        Some("Project Phase 1 Assets"),
    )
    .expect("create portfolio");

    assert!(is_portfolio(&port_doc));

    let portfolio = get_portfolio(&port_doc).expect("get portfolio");
    assert_eq!(portfolio.title, "Field Inspection Portfolio");
    assert_eq!(portfolio.items.len(), 2);

    // Search inside portfolio (FR-29.2.3)
    let matches = search_portfolio(&portfolio, "sensor");
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].filename, "dataset.csv");

    // Extract file from portfolio (FR-29.2.4)
    let extracted = extract_portfolio_item(&port_doc, "notes.txt")
        .expect("extract item")
        .expect("found notes");
    assert_eq!(extracted, b"Calibration passed.");
}

#[test]
fn test_js_scan_print_and_plugin_workflows() {
    use image::RgbaImage;
    use ravpdf_forms::{add_form_field, FormField};
    use ravpdf_tools::{
        batch_scan_to_pdf, execute_all_field_calculations, execute_script,
        generate_postscript, generate_print_preview_layout, BookletBinding, JsExecutionContext,
        JsValue, PluginPermission, PluginRegistry, PrintJobConfig, PrintLayoutMode,
        SampleWatermarkPlugin, ScannerJobConfig,
    };

    // 1. JavaScript Engine & AcroForm Calculation Workflow (FR-12)
    let mut doc = PdfDocument::new();
    doc.add_blank_page(paper_sizes::LETTER.0, paper_sizes::LETTER.1);

    let mut item1 = FormField::new_text("item1", [100.0, 600.0, 200.0, 620.0]);
    item1.value = Some("49.99".into());
    add_form_field(&mut doc, 0, &item1).expect("add item1");

    let mut item2 = FormField::new_text("item2", [100.0, 560.0, 200.0, 580.0]);
    item2.value = Some("25.01".into());
    add_form_field(&mut doc, 0, &item2).expect("add item2");

    let mut total_field = FormField::new_text("grand_total", [100.0, 520.0, 200.0, 540.0]);
    total_field.value = Some("0.00".into());
    add_form_field(&mut doc, 0, &total_field).expect("add total");

    let mut js_ctx = JsExecutionContext::new();
    js_ctx.sync_from_document(&doc).expect("sync fields to js");

    // Register calculation script on total
    if let Some(f) = js_ctx.get_field_mut("grand_total") {
        f.calculate_script = Some("AFSimple_Calculate('SUM', new Array('item1', 'item2'));".into());
    }

    let calculated = execute_all_field_calculations(&mut js_ctx).expect("exec calcs");
    assert_eq!(calculated, 1);
    assert_eq!(
        js_ctx.get_field("grand_total").unwrap().value,
        JsValue::Number(75.0)
    );

    // Custom expression evaluation with app.alert
    let expr_res = execute_script(&mut js_ctx, "app.alert('Total verified: ' + grand_total, 'Audit', 3, 0);")
        .expect("alert script");
    assert_eq!(expr_res, JsValue::Number(1.0));
    assert_eq!(js_ctx.alerts.len(), 1);
    assert!(js_ctx.alerts[0].message.contains("Total verified: 75"));

    // Sync back to document
    js_ctx.sync_to_document(&mut doc).expect("sync js back to doc");

    // 2. Scan Cleanup & Batch Scan to PDF (FR-32)
    let mut scan_page1 = RgbaImage::new(120, 160);
    let mut scan_page2 = RgbaImage::new(120, 160);
    for y in 0..160 {
        for x in 0..120 {
            let px = if x < 5 || x >= 115 || y < 5 || y >= 155 {
                image::Rgba([15, 15, 15, 255]) // dark scanner bed border
            } else {
                image::Rgba([230, 230, 230, 255]) // paper
            };
            scan_page1.put_pixel(x, y, px);
            scan_page2.put_pixel(x, y, px);
        }
    }

    let scan_config = ScannerJobConfig::default();
    let scanned_doc = batch_scan_to_pdf(&[scan_page1, scan_page2], &scan_config)
        .expect("batch scan to pdf");
    assert_eq!(scanned_doc.page_count(), 2);

    // 3. Print Imposition & PostScript Generation (FR-35)
    let print_config = PrintJobConfig {
        printer_name: "Office Laser Pro".into(),
        copies: 2,
        collate: true,
        page_range_str: "1-2".into(),
        layout_mode: PrintLayoutMode::Booklet {
            binding: BookletBinding::LeftToRight,
            subset: ravpdf_tools::BookletSubset::BothSides,
        },
        ..Default::default()
    };

    let preview = generate_print_preview_layout(&scanned_doc, &print_config).expect("preview");
    assert_eq!(preview.total_logical_pages, 2);
    assert_eq!(preview.total_physical_sheets, 1);

    let ps_output = generate_postscript(&scanned_doc, &print_config).expect("generate ps");
    assert!(ps_output.starts_with("%!PS-Adobe-3.0"));
    assert!(ps_output.contains("%%Pages: 2"));

    // 4. Plugin Registry & Extension Execution (FR-36)
    let mut registry = PluginRegistry::new();
    let watermark_plugin = Box::new(SampleWatermarkPlugin::new());
    let perms = [
        PluginPermission::UiExtension,
        PluginPermission::DocumentTransform,
    ];
    registry
        .register_plugin(watermark_plugin, &perms)
        .expect("register plugin");

    assert_eq!(registry.list_plugins().len(), 1);
    let mut target_doc = PdfDocument::new();
    target_doc.add_blank_page(612.0, 792.0);

    let plugin_result = registry
        .dispatch_action(
            "org.ravpdf.sample-watermark",
            "apply_draft_watermark",
            Some(&mut target_doc),
        )
        .expect("dispatch plugin action");
    assert!(plugin_result.unwrap().contains("Applied sample draft watermark across 1 pages"));
}

#[test]
fn test_form_data_import_export_autodetect_taborder_workflows() {
    use ravpdf_forms::{
        apply_auto_detected_fields, apply_page_tab_order, auto_detect_form_fields,
        export_form_data, get_form_fields, import_form_data,
        FormDataFormat, FormField, TabOrder,
    };

    let mut doc = PdfDocument::new();
    doc.add_blank_page(612.0, 792.0);

    // 1. Add form fields
    let mut name_f = FormField::new_text("ApplicantName", [100.0, 600.0, 300.0, 620.0]);
    name_f.value = Some("John Doe".into());
    ravpdf_forms::add_form_field(&mut doc, 0, &name_f).expect("add name field");

    let mut dept_f = FormField::new_text("Department", [100.0, 550.0, 300.0, 570.0]);
    dept_f.value = Some("Engineering".into());
    ravpdf_forms::add_form_field(&mut doc, 0, &dept_f).expect("add dept field");

    // 2. Export to all supported formats (FR-11.3.2)
    let json_data = export_form_data(&doc, FormDataFormat::Json).expect("export json");
    assert!(json_data.contains("ApplicantName"));
    assert!(json_data.contains("John Doe"));

    let fdf_data = export_form_data(&doc, FormDataFormat::Fdf).expect("export fdf");
    assert!(fdf_data.contains("%FDF-1.2"));
    assert!(fdf_data.contains("/T (ApplicantName)"));

    let xfdf_data = export_form_data(&doc, FormDataFormat::Xfdf).expect("export xfdf");
    assert!(xfdf_data.contains("<xfdf xmlns="));
    assert!(xfdf_data.contains("name=\"ApplicantName\""));

    let csv_data = export_form_data(&doc, FormDataFormat::Csv).expect("export csv");
    assert!(csv_data.contains("\"ApplicantName\""));
    assert!(csv_data.contains("\"John Doe\""));

    let xml_data = export_form_data(&doc, FormDataFormat::Xml).expect("export xml");
    assert!(xml_data.contains("<form-data>"));
    assert!(xml_data.contains("name=\"ApplicantName\""));

    // 3. Import modified data (FR-11.3.2)
    let updated_json = r#"{"ApplicantName": "Jane Smith", "Department": "Research"}"#;
    let imported_count = import_form_data(&mut doc, updated_json, FormDataFormat::Json).expect("import json");
    assert_eq!(imported_count, 2);

    let fields = get_form_fields(&doc).expect("get fields");
    let name_val = fields.iter().find(|f| f.name == "ApplicantName").and_then(|f| f.value.as_deref());
    assert_eq!(name_val, Some("Jane Smith"));

    // 4. Form Field Auto-Detection (FR-11.3.1)
    let mut scan_doc = PdfDocument::new();
    let text_content = b"BT\n/F1 12 Tf\n72 700 Td\n(Full Name:) Tj\n0 -50 Td\n(Email Address:) Tj\n0 -50 Td\n(Signature:) Tj\nET\n";
    scan_doc.add_page_with_content(612.0, 792.0, text_content.to_vec());

    let detected = auto_detect_form_fields(&scan_doc, 0).expect("autodetect");
    assert_eq!(detected.len(), 3);
    assert_eq!(detected[0].field.name, "Full_Name");
    assert_eq!(detected[1].field.name, "Email_Address");
    assert_eq!(detected[2].field.name, "Signature_3");

    apply_auto_detected_fields(&mut scan_doc, 0, &detected).expect("apply detected");
    let detected_fields = get_form_fields(&scan_doc).expect("get detected fields");
    assert_eq!(detected_fields.len(), 3);

    // 5. Tab Order Specification (FR-11.2.7)
    apply_page_tab_order(&mut scan_doc, 0, TabOrder::Row).expect("apply row tab order");
    let page_id = scan_doc.page_ids()[0];
    let page_obj = scan_doc.objects.get(&page_id).and_then(|o| o.as_dict()).unwrap();
    assert_eq!(page_obj.get_name("Tabs"), Some("R"));

    apply_page_tab_order(&mut scan_doc, 0, TabOrder::Column).expect("apply col tab order");
    let page_obj2 = scan_doc.objects.get(&page_id).and_then(|o| o.as_dict()).unwrap();
    assert_eq!(page_obj2.get_name("Tabs"), Some("C"));
}

#[test]
fn test_multimedia_and_3d_workflows() {
    use ravpdf_tools::{
        add_3d_annotation, add_rich_media_annotation, extract_3d_stream,
        extract_rich_media_content, get_3d_annotations, get_rich_media_annotations,
        MediaContent, RichMediaActivation, RichMediaConfig, RichMediaDeactivation,
        ThreeDConfig, ThreeDFormat, ThreeDLighting, ThreeDRenderMode, ThreeDView,
    };

    let mut doc = PdfDocument::new();
    doc.add_blank_page(612.0, 792.0);

    // 1. RichMedia Embedded Video Annotation (FR-27.1, FR-27.4, FR-27.5)
    let fake_mp4 = b"\x00\x00\x00\x18ftypmp42\x00\x00\x00\x00isommp42".to_vec();
    let media_cfg = RichMediaConfig {
        title: "Product Overview Video".to_string(),
        content: MediaContent::Embedded {
            data: fake_mp4.clone(),
            mime_type: "video/mp4".to_string(),
            filename: "overview.mp4".to_string(),
        },
        rect: [100.0, 400.0, 400.0, 600.0],
        poster_image: None,
        auto_play: false,
        loop_playback: true,
        show_controls: true,
        volume: 0.8,
        activation: RichMediaActivation::UserClick,
        deactivation: RichMediaDeactivation::PageClose,
    };

    let media_annot_id = add_rich_media_annotation(&mut doc, 0, &media_cfg).expect("add rich media");
    assert!(doc.objects.contains_key(&media_annot_id));

    let page_media = get_rich_media_annotations(&doc, 0).expect("get rich media");
    assert_eq!(page_media.len(), 1);
    assert_eq!(page_media[0].title, "Product Overview Video");
    assert!(page_media[0].is_embedded);
    assert!(page_media[0].loop_playback);

    let extracted_media = extract_rich_media_content(&doc, media_annot_id)
        .expect("extract media")
        .expect("embedded bytes found");
    assert_eq!(extracted_media, fake_mp4);

    // 2. 3D Model Annotation (FR-28.1 - FR-28.7)
    let fake_u3d = b"U3D\x00\x01\x00\x00\x00SampleUniversal3DModelStream".to_vec();
    let threed_view = ThreeDView {
        name: "Isometric".to_string(),
        render_mode: ThreeDRenderMode::SolidWireframe,
        lighting: ThreeDLighting::Cad,
        ..Default::default()
    };

    let threed_cfg = ThreeDConfig {
        format: ThreeDFormat::U3d,
        model_data: fake_u3d.clone(),
        rect: [100.0, 150.0, 400.0, 350.0],
        views: vec![threed_view],
        default_view_index: 0,
        model_tree: None,
    };

    let threed_annot_id = add_3d_annotation(&mut doc, 0, &threed_cfg).expect("add 3d annot");
    assert!(doc.objects.contains_key(&threed_annot_id));

    let page_3d = get_3d_annotations(&doc, 0).expect("get 3d annots");
    assert_eq!(page_3d.len(), 1);
    assert_eq!(page_3d[0].format, ThreeDFormat::U3d);
    assert_eq!(page_3d[0].stream_size_bytes, fake_u3d.len());

    let extracted_3d = extract_3d_stream(&doc, threed_annot_id)
        .expect("extract 3d")
        .expect("3d bytes found");
    assert_eq!(extracted_3d, fake_u3d);
}

#[test]
fn test_action_wizard_batch_workflows() {
    use ravpdf_tools::{
        execute_batch_cancellable, ActionSequence, BatchAction, BatchJob,
    };

    // 1. Templates generation & Recipe Serialization (FR-30.3, FR-30.7)
    let dist_template = ActionSequence::template_prepare_for_distribution();
    assert_eq!(dist_template.name, "Prepare for Distribution");
    assert_eq!(dist_template.actions.len(), 3);

    let json_recipe = serde_json::to_string_pretty(&dist_template).expect("serialize recipe");
    let loaded_recipe: ActionSequence = serde_json::from_str(&json_recipe).expect("deserialize recipe");
    assert_eq!(loaded_recipe.name, dist_template.name);
    assert_eq!(loaded_recipe.actions.len(), dist_template.actions.len());

    // 2. Multi-step Execution Across Files (FR-30.1, FR-30.2, FR-30.8)
    let temp_dir = std::env::temp_dir().join("ravpdf_cli_action_wizard_test");
    let in_dir = temp_dir.join("inputs");
    let out_dir = temp_dir.join("outputs");
    let _ = std::fs::create_dir_all(&in_dir);
    let _ = std::fs::create_dir_all(&out_dir);

    let mut doc_a = PdfDocument::new();
    doc_a.add_blank_page(612.0, 792.0);
    let path_a = in_dir.join("report_a.pdf");
    doc_a.save_to_file(&path_a).unwrap();

    let mut doc_b = PdfDocument::new();
    doc_b.add_blank_page(612.0, 792.0);
    let path_b = in_dir.join("report_b.pdf");
    doc_b.save_to_file(&path_b).unwrap();

    let job = BatchJob {
        input_files: vec![path_a, path_b],
        output_directory: out_dir.clone(),
        actions: vec![
            BatchAction::Watermark {
                text: "INTERNAL ONLY".into(),
                opacity: 0.2,
            },
            BatchAction::SanitizeMetadata,
            BatchAction::Optimize,
        ],
    };

    let progress_count = std::sync::atomic::AtomicUsize::new(0);
    let res = execute_batch_cancellable(
        &job,
        None,
        Some(&|_curr, _total, _file| {
            progress_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        }),
    );

    assert_eq!(res.total_files, 2);
    assert_eq!(res.successful_files, 2);
    assert_eq!(res.failed_files, 0);
    assert_eq!(progress_count.load(std::sync::atomic::Ordering::SeqCst), 2);
    assert!(out_dir.join("report_a.pdf").exists());
    assert!(out_dir.join("report_b.pdf").exists());
}



