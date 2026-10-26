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

#[test]
fn test_ocr_workflows() {
    use image::{Rgba, RgbaImage};
    use ravpdf_tools::{
        create_pdf_from_image_with_ocr, perform_ocr_document, ActionSequence, OcrConfig,
        OcrOutputMode,
    };

    // 1. Test OCR on scanned raster image creating a Searchable Image PDF (FR-16.1, FR-16.2, FR-16.4, FR-16.5)
    let mut img = RgbaImage::from_pixel(300, 150, Rgba([255, 255, 255, 255]));
    for y in 60..90 {
        for x in 40..260 {
            img.put_pixel(x, y, Rgba([0, 0, 0, 255]));
        }
    }

    let config = OcrConfig {
        language: "eng".to_string(),
        output_mode: OcrOutputMode::SearchableImage,
        dpi: 300,
        auto_rotate: true,
        min_confidence: 0.60,
        preserve_appearance: true,
    };

    let (mut doc, ocr_res) = create_pdf_from_image_with_ocr(&img, &config).expect("create pdf from image with OCR");
    assert_eq!(doc.page_count(), 1);
    assert!(!ocr_res.lines.is_empty());
    assert!(ocr_res.average_confidence > 0.0);

    // 2. Test multi-page OCR document execution (FR-16.7)
    doc.add_blank_page(612.0, 792.0);
    let doc_res = perform_ocr_document(&mut doc, &config, None).expect("perform OCR on document");
    assert_eq!(doc_res.pages.len(), 2);

    // 3. Verify content stream contains invisible text rendering mode 3 Tr (FR-16.4, FR-16.5)
    let page0_content = doc.get_page_content(0).expect("get page 0 content");
    let page0_str = String::from_utf8_lossy(&page0_content);
    assert!(page0_str.contains("3 Tr"));

    // 4. Verify ActionSequence template: "Scan and Make Searchable" (FR-30.3, FR-16.7)
    let scan_template = ActionSequence::template_scan_and_make_searchable();
    assert_eq!(scan_template.name, "Scan and Make Searchable");
    assert_eq!(scan_template.actions.len(), 2);
    let json_seq = scan_template.to_json().expect("serialize template");
    let restored_seq = ActionSequence::from_json(&json_seq).expect("deserialize template");
    assert_eq!(restored_seq.name, "Scan and Make Searchable");
}

#[test]
fn test_sign_preflight_and_color_workflows() {
    use ravpdf_crypto::{get_document_signatures, DigitalId, SignatureStatus, sign_pdf_document};
    use ravpdf_tools::{
        convert_document_colors, embed_output_intent, execute_preflight_fixup, get_output_intents,
        run_preflight, PreflightFixup, PreflightProfile, StandardIccProfile,
    };

    // 1. Digital Signing Workflow (FR-13.1, FR-13.6, FR-13.13)
    let mut doc = PdfDocument::new();
    doc.add_blank_page(612.0, 792.0);

    let digital_id = DigitalId::generate("Authorized Certifier", "Legal Dept", "certifier@org.com", 365)
        .expect("generate digital ID");
    let signed_bytes = sign_pdf_document(
        doc,
        0,
        "SignField1",
        &digital_id,
        "Approved for Publication",
        "Headquarters",
        Some([72.0, 72.0, 260.0, 130.0]),
    )
    .expect("sign document");

    let signed_doc = PdfDocument::load_from_bytes(&signed_bytes).expect("load signed doc");
    let sigs = get_document_signatures(&signed_doc, Some(&signed_bytes)).expect("get sigs");
    assert_eq!(sigs.len(), 1);
    assert_eq!(sigs[0].field_name, "SignField1");
    assert_eq!(sigs[0].signer_name.as_deref(), Some("Authorized Certifier"));
    assert!(matches!(sigs[0].status, SignatureStatus::Valid(_)));

    // 2. Preflight Audit & Automated Fixup Workflow (FR-22, FR-23.1.4)
    let mut audit_doc = PdfDocument::new();
    let sample_content = b"BT\n/F1 12 Tf\n100 700 Td\n(Uncompressed text stream) Tj\nET\n";
    audit_doc.add_page_with_content(612.0, 792.0, sample_content.to_vec());

    let initial_report = run_preflight(&audit_doc, PreflightProfile::PdfA1b);
    assert!(!initial_report.is_compliant);

    let fixup_msg = execute_preflight_fixup(&mut audit_doc, PreflightFixup::FixAll)
        .expect("execute preflight fixups");
    assert!(fixup_msg.contains("Populated") || fixup_msg.contains("OutputIntent"));

    let fixed_report = run_preflight(&audit_doc, PreflightProfile::PdfA1b);
    assert!(fixed_report.total_passed >= initial_report.total_passed);

    // 3. Color Management & Output Intent Workflow (FR-24.1, FR-24.4, FR-24.6)
    let oi_id = embed_output_intent(
        &mut audit_doc,
        StandardIccProfile::USWebCoatedSWOP,
        "U.S. Web Coated (SWOP) v2",
        "http://www.color.org",
    )
    .expect("embed output intent");
    assert!(oi_id.number > 0);

    let oi_list = get_output_intents(&audit_doc).expect("get output intents");
    assert_eq!(oi_list.len(), 2); // 1 from preflight fixup, 1 from embed_output_intent
    assert!(oi_list.iter().any(|oi| oi.output_condition_identifier == "U.S. Web Coated (SWOP) v2"));

    let converted_count = convert_document_colors(&mut audit_doc, "DeviceCMYK")
        .expect("convert document colors");
    assert_eq!(converted_count, 0); // content had no color operators, handled safely without error
}

#[test]
fn test_prepress_marks_flattening_and_standards_compliance() {
    use ravpdf_tools::{
        add_printer_marks, apply_ink_manager, convert_to_standard, flatten_document_transparency,
        get_document_inks, get_trap_presets, set_trap_presets, validate_standard_compliance,
        InkManagerConfig, InkMapping, PrinterMarksConfig, StandardConversionConfig, StandardFormat,
        StandardIccProfile, TransparencyFlattenConfig, TrapPreset,
    };

    // 1. Prepress Printer Marks Injection (FR-23.3.4)
    let mut doc = PdfDocument::new();
    doc.add_blank_page(612.0, 792.0);

    let marks_cfg = PrinterMarksConfig {
        trim_marks: true,
        bleed_marks: true,
        registration_marks: true,
        color_bars: true,
        page_information: true,
        slug_margin: 36.0,
        bleed_offset: 9.0,
        ..Default::default()
    };
    let marked_pages = add_printer_marks(&mut doc, &marks_cfg).expect("add printer marks");
    assert_eq!(marked_pages, 1);

    let page = doc.get_page(0).expect("page 0");
    assert_eq!(page.media_box, [-36.0, -36.0, 648.0, 828.0]);

    // 2. Prepress Transparency Flattening (FR-23.3.3)
    let flat_cfg = TransparencyFlattenConfig {
        raster_vector_balance: 80,
        flatten_opacity: true,
        normalize_blend_modes: true,
        remove_page_transparency_groups: true,
    };
    let flat_rep = flatten_document_transparency(&mut doc, &flat_cfg).expect("flatten");
    assert_eq!(flat_rep.total_pages_processed, 1);

    // 3. Prepress Trap Presets (FR-23.3.5)
    let trap_preset = TrapPreset {
        name: "Commercial Web Press".into(),
        trap_width: 0.25,
        black_width: 0.50,
        black_color_limit: 1.0,
        trap_color_reduction: 0.5,
    };
    set_trap_presets(&mut doc, &[trap_preset.clone()]).expect("set trap presets");
    let retrieved_presets = get_trap_presets(&doc);
    assert_eq!(retrieved_presets.len(), 1);
    assert_eq!(retrieved_presets[0].name, "Commercial Web Press");

    // 4. Prepress Ink Manager (FR-23.3.2)
    let inks = get_document_inks(&doc);
    assert_eq!(inks.len(), 4); // Standard CMYK process plates
    let ink_cfg = InkManagerConfig {
        mappings: vec![InkMapping {
            spot_name: "PANTONE 286 C".into(),
            cmyk: (1.0, 0.75, 0.0, 0.0),
        }],
        convert_all_spots_to_process: true,
    };
    let mapped = apply_ink_manager(&mut doc, &ink_cfg).expect("apply ink manager");
    assert_eq!(mapped, 0);

    // 5. Standards Compliance: PDF/A-1b Archival Conversion & Validation (FR-22.1, FR-22.4, FR-22.6)
    let mut std_doc = PdfDocument::new();
    std_doc.add_blank_page(612.0, 792.0);

    let conv_cfg = StandardConversionConfig {
        target: StandardFormat::PdfA1b,
        icc_profile: StandardIccProfile::SRgb,
        remove_javascript: true,
        remove_encryption: true,
        convert_colors_to_cmyk: false,
        embed_output_intent: true,
        embed_xmp_metadata: true,
    };
    let conv_rep = convert_to_standard(&mut std_doc, &conv_cfg).expect("convert to PDF/A-1b");
    assert!(conv_rep.is_success);
    assert_eq!(conv_rep.target_version, (1, 4));

    let val_rep = validate_standard_compliance(&std_doc, StandardFormat::PdfA1b);
    assert!(val_rep.is_compliant);
    assert!(val_rep.checks.iter().all(|c| c.passed));

    // 6. Standards Compliance: PDF/X-1a Commercial Prepress Conversion & Validation (FR-22.2, FR-22.4)
    let mut x_doc = PdfDocument::new();
    x_doc.add_blank_page(612.0, 792.0);

    let x_cfg = StandardConversionConfig {
        target: StandardFormat::PdfX1a,
        icc_profile: StandardIccProfile::USWebCoatedSWOP,
        remove_javascript: false,
        remove_encryption: true,
        convert_colors_to_cmyk: true,
        embed_output_intent: true,
        embed_xmp_metadata: true,
    };
    let x_rep = convert_to_standard(&mut x_doc, &x_cfg).expect("convert to PDF/X-1a");
    assert!(x_rep.is_success);
    assert_eq!(x_rep.target_version, (1, 4));

    let x_val = validate_standard_compliance(&x_doc, StandardFormat::PdfX1a);
    assert!(x_val.is_compliant);
}

#[test]
fn test_aes256_encryption_and_certification_signature_ltv_workflow() {
    use ravpdf_crypto::{
        decrypt_document, encrypt_document_with_algorithm, get_document_signatures,
        sign_pdf_document_advanced, DigitalId, DocMdpPermission, EncryptionAlgorithm, Permissions,
        SignatureStatus, SigningOptions,
    };

    // 1. Modern AES-256 (Revision 6) PDF 2.0 Document Encryption & Decryption (FR-14.4)
    let mut enc_doc = PdfDocument::new();
    let stream_payload = b"q\n0.2 0.4 0.8 rg\n50 50 200 100 re\nf\nQ\n";
    enc_doc.add_page_with_content(612.0, 792.0, stream_payload.to_vec());

    encrypt_document_with_algorithm(
        &mut enc_doc,
        "SecretUserPass#2026",
        "MasterOwnerPass#2026",
        Permissions::all_permitted(),
        EncryptionAlgorithm::Aes256,
    )
    .expect("encrypt with AES-256");

    let enc_id = enc_doc.trailer.get_ref("Encrypt").expect("Encrypt dictionary");
    let enc_dict = enc_doc.objects.get(&enc_id).unwrap().as_dict().unwrap();
    assert_eq!(enc_dict.get_i64("Length"), Some(256));
    assert_eq!(enc_dict.get_i64("V"), Some(5));
    assert_eq!(enc_dict.get_i64("R"), Some(6));

    decrypt_document(&mut enc_doc, "SecretUserPass#2026").expect("decrypt AES-256 document");
    assert!(!enc_doc.trailer.contains_key("Encrypt"));

    // 2. Author Certification Signature (DocMDP) with LTV (/DSS) (FR-13.3, FR-13.8)
    let mut cert_doc = PdfDocument::new();
    cert_doc.add_blank_page(612.0, 792.0);

    let digital_id = DigitalId::generate(
        "Chief Executive Officer",
        "Global Enterprise Corp",
        "ceo@enterprise.com",
        365,
    )
    .expect("generate digital ID");

    let opts = SigningOptions {
        field_name: "AuthorCertSig".into(),
        reason: "Author Certification with Form Filling Permitted".into(),
        location: "Zurich, Switzerland".into(),
        contact_info: Some("legal@enterprise.com".into()),
        rect: Some([50.0, 50.0, 250.0, 120.0]),
        is_certification: true,
        certification_permission: Some(DocMdpPermission::AnnotationsAndFormFilling),
        embed_ltv: true,
    };

    let signed_bytes = sign_pdf_document_advanced(cert_doc, 0, &digital_id, &opts)
        .expect("sign certified document with ltv");
    assert!(!signed_bytes.is_empty());

    let verified_doc = PdfDocument::load_from_bytes(&signed_bytes).expect("parse certified signed doc");
    let sigs = get_document_signatures(&verified_doc, Some(&signed_bytes)).expect("get sigs");
    assert_eq!(sigs.len(), 1);

    let s = &sigs[0];
    assert_eq!(s.field_name, "AuthorCertSig");
    assert_eq!(s.signer_name.as_deref(), Some("Chief Executive Officer"));
    assert_eq!(s.reason.as_deref(), Some("Author Certification with Form Filling Permitted"));
    assert_eq!(s.location.as_deref(), Some("Zurich, Switzerland"));
    assert!(s.is_certification);
    assert_eq!(s.doc_mdp_permission, Some(DocMdpPermission::AnnotationsAndFormFilling));
    assert!(s.has_ltv);
    assert!(matches!(s.status, SignatureStatus::Valid(_)));
}

#[test]
fn test_text_editing_and_markdown_creation_workflows() {
    use ravpdf_edit::{
        create_pdf_from_markdown, extract_page_text_spans, insert_formatted_text_block,
        replace_text_span_in_page, suggest_font_substitute, FormattedTextBlockOptions,
        MarkdownPdfOptions, TextAlignment,
    };

    // 1. Markdown document generation with headers, lists, code block, and pagination (FR-02.5, FR-02.6, FR-03.1.6)
    let md = r#"# Technical Architecture Specification

RavPDF is engineered with pure Rust for memory safety and maximum execution throughput.

## Core Capabilities

- In-place text editing and font substitution
- Interactive AcroForms and Digital Signatures
- ISO 32000-1 and ISO 32000-2 compliance

> "Rust provides memory safety guarantees without garbage collection overhead."

```rust
fn engine_start() -> bool {
    true
}
```

---

Section continued with detailed specifications.
"#;

    let md_opts = MarkdownPdfOptions {
        page_size: (500.0, 400.0),
        margin: 36.0,
        base_font_size: 11.0,
        line_spacing: 15.0,
        title: Some("RavPDF Architecture".to_string()),
        include_page_numbers: true,
    };

    let mut doc = create_pdf_from_markdown(md, &md_opts).expect("generate markdown PDF");
    assert!(doc.page_count() >= 1);

    // 2. Extract structured text spans (FR-04.1, FR-04.3)
    let spans = extract_page_text_spans(&doc, 0).expect("extract text spans");
    assert!(!spans.is_empty());
    assert!(spans.iter().any(|s| s.text.contains("Technical Architecture Specification")));

    // 3. In-place text replacement with font & color update (FR-04.1, FR-04.3, FR-04.12)
    let rep_count = replace_text_span_in_page(
        &mut doc,
        0,
        "Technical Architecture Specification",
        "Executive Engineering Report",
        Some("Helvetica-Bold"),
        Some(20.0),
        Some((0.15, 0.35, 0.75)),
    )
    .expect("replace text span");
    assert_eq!(rep_count, 1);

    let updated_spans = extract_page_text_spans(&doc, 0).expect("extract updated spans");
    assert!(updated_spans.iter().any(|s| s.text.contains("Executive Engineering Report")));

    // 4. Insert formatted text block with Justify alignment and Underline (FR-04.2, FR-04.4)
    let block_opts = FormattedTextBlockOptions {
        rect: [36.0, 50.0, 428.0, 80.0],
        font_family: "Times-Roman".to_string(),
        font_size: 11.0,
        alignment: TextAlignment::Justify,
        line_spacing: 14.0,
        char_spacing: 0.25,
        color: (0.1, 0.1, 0.1),
        underline: true,
        strikethrough: false,
    };

    insert_formatted_text_block(
        &mut doc,
        0,
        "Approved by executive steering committee for immediate release and implementation across all target desktop environments.",
        &block_opts,
    )
    .expect("insert formatted text block");

    let final_content = doc.get_page_content(0).expect("get final page content");
    let final_str = String::from_utf8_lossy(&final_content);
    assert!(final_str.contains("Executive Engineering Report"));
    assert!(final_str.contains("approved by executive steering committee") || final_str.contains("Approved by executive"));

    // 5. Font substitution verification (FR-04.12)
    let arial_sub = suggest_font_substitute("Arial");
    assert_eq!(arial_sub.substitute_font, "Helvetica");
    assert!(!arial_sub.is_exact_match);

    let times_sub = suggest_font_substitute("Times-Roman");
    assert_eq!(times_sub.substitute_font, "Times-Roman");
    assert!(times_sub.is_exact_match);
}





