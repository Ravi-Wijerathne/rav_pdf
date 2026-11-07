//! RavPDF Desktop GUI Application.
//!
//! Traceability:
//! - FR-01 (PDF Viewing and Navigation: continuous/fit modes, keyboard navigation shortcuts)
//! - FR-01.2.1 (Page navigation: Page Up/Down, Home, End, Arrows)
//! - FR-01.2.4 & FR-19.1 - FR-19.8 (Bookmarks / Outline tree panel and bookmark creation)
//! - FR-01.2.6 (Go to Page direct navigation)
//! - FR-01.3.2 (Fit to Page, Fit to Width, 100% actual size)
//! - FR-01.4 (Multi-Tab Document Interface)
//! - FR-08.1 - FR-08.2 (Annotation and Markup Tools: sticky notes, text boxes, highlights)
//! - FR-09.5 (Drawing Tools: Rectangles / Shapes)
//! - FR-11.1 - FR-11.3 (Interactive AcroForms: on-canvas text fields, checkboxes, form flattening)
//! - FR-18.1 (Headers and Footers dynamic tokens and dialog)
//! - FR-18.2 (Watermarking dialog and application)
//! - FR-18.3 (Bates Numbering dialog and application)
//! - FR-34.1 (Find & Search Ctrl+F with match navigation)
//! - EI-01.1 - EI-01.9 (Main menu bar, toolbar, canvas, side panels, status bar)
//! - NFR-05.1 (Usability, modern UI conventions)

mod tab;

use eframe::egui::{
    self, Align, Align2, Button, Color32, FontId, Key, Layout, Pos2, Rect, RichText, ScrollArea,
    Stroke, Vec2,
};
use ravpdf_annot::{
    add_annotation_to_page, add_callout_annotation, add_ink_annotation, add_shape_annotation,
    add_stamp_annotation, add_typewriter_annotation, delete_annotation, erase_ink_strokes,
    get_page_annotations, set_annotation_locked, update_annotation_color,
    update_annotation_contents, update_annotation_rect, Annotation, AnnotationType, BorderEffect,
    BorderStyle, CalloutConfig, InkAnnotationConfig, LineEndingStyle, LinkAction,
    ShapeAnnotationConfig, ShapeType, StampConfig, StandardStamp, TypewriterConfig,
};
use ravpdf_core::destinations::{
    add_named_destination, get_named_destinations, remove_named_destination, DestinationView,
};
use ravpdf_core::document::{paper_sizes, PdfDocument};
use ravpdf_core::object::ObjectId;
use ravpdf_core::outline::{add_bookmark, Bookmark};
use ravpdf_crypto::{
    create_signature_field, encrypt_document_with_algorithm, get_document_signatures,
    sign_pdf_document_advanced, DigitalId, DocMdpPermission, EncryptionAlgorithm, Permissions,
    SignatureStatus, SigningOptions,
};
use ravpdf_edit::{
    create_pdf_from_images, create_pdf_from_markdown, create_pdf_from_text, crop_page_image,
    delete_page, delete_page_image, duplicate_page, extract_page_images, extract_page_text_spans,
    extract_pages, get_page_boxes, get_page_label, get_page_labels, insert_blank_page,
    insert_formatted_text_block, insert_image_file, merge_documents, reorder_page_image,
    reorder_pages, replace_page_image_from_file, replace_text_span_in_page, rotate_page,
    set_page_box, set_page_image_opacity, set_page_labels, snap_to_alignment_guides,
    split_document, suggest_font_substitute, transform_page_image, FormattedTextBlockOptions,
    ImagePdfOptions, ImageTransformOptions, MarkdownPdfOptions, PageBoxType, PageImage,
    PageLabelRange, PageLabelStyle, SnapGuide, TextAlignment, TextPdfOptions, TextSpan,
    ZOrderAction,
};
use ravpdf_forms::{
    add_form_field, apply_auto_detected_fields, apply_page_tab_order, auto_detect_form_fields,
    delete_form_field, export_form_data, flatten_form, get_form_fields, get_page_form_fields,
    import_form_data, set_form_field_value, update_form_field_properties, update_form_field_rect,
    BarcodeSymbology, FormDataFormat, FormField, FormFieldType, TabOrder,
};
use ravpdf_render::{pixmap_to_png, render_page, RenderOptions};
use ravpdf_tools::perform_ocr_document;
use ravpdf_tools::{
    add_3d_annotation, add_attachment, add_measurement_line_annotation, add_printer_marks,
    add_rich_media_annotation, apply_batch_redactions, apply_bates_numbering,
    apply_headers_and_footers, apply_ink_manager, apply_text_watermark, audit_accessibility,
    audit_space_usage, batch_scan_to_pdf, calculate_area, calculate_distance, compare_documents,
    convert_document_colors, convert_to_standard, create_pdf_from_rtf, create_portfolio,
    embed_output_intent, execute_all_field_calculations, execute_batch, execute_preflight_fixup,
    execute_script, export_comments_to_csv, export_page_to_image, export_to_csv_table,
    export_to_html, export_to_plain_text, export_to_rtf, extract_all_text, find_and_replace,
    find_and_replace_page, fix_document_language, fix_document_title, fix_ensure_tagged_pdf,
    fix_tab_order_to_structure, flatten_document_transparency, generate_postscript,
    generate_print_preview_layout, get_attachments, get_document_comments, get_document_inks,
    get_page_geospatial_viewports, get_portfolio, get_trap_presets, inspect_page_objects,
    is_linearized, is_portfolio, linearize_document, merge_reviewer_annotations, optimize_document,
    redact_pattern, remediate_all_automatic, remove_attachment, run_preflight, sanitize_document,
    search_document, set_document_info, set_trap_presets, validate_standard_compliance,
    AccessibilityReport, AccessibilityStatus, AreaUnit, BatchAction, BatchJob, BatchResult,
    BookletBinding, ComparisonReport, ExportImageFormat, FileAttachment, HeaderFooterConfig,
    InkManagerConfig, JsExecutionContext, LinearUnit, MediaContent, NUpCount, NUpOrder,
    ObjectInspectionReport, OcrConfig, OcrDocumentResult, OcrOutputMode, OptimizationStats,
    PlateSeparationConfig, PluginPermission, PluginRegistry, PortfolioItem, PortfolioLayout,
    PreflightFixup, PreflightProfile, PreflightReport, PreflightSeverity, PrintJobConfig,
    PrintLayoutMode, PrinterMarksConfig, RedactionAppearance, RenderingIntent, ReviewComment,
    RichMediaActivation, RichMediaConfig, RichMediaDeactivation, SampleWatermarkPlugin,
    SanitizeOptions, SanitizeReport, ScaleRatio, ScannerJobConfig, SearchMatch, SeparationAnalysis,
    SnapOptions, SoftProofConfig, SpaceAudit, StandardConversionConfig, StandardFormat,
    StandardIccProfile, StandardValidationReport, ThreeDConfig, ThreeDFormat, ThreeDLighting,
    ThreeDRenderMode, ThreeDView, TransparencyFlattenConfig, TrapPreset,
};
use std::collections::HashSet;
use std::path::PathBuf;
use tab::DocumentTab;

/// Action triggered on a page from the thumbnail sidebar (FR-01.2.3, FR-06).
#[derive(Debug, Clone, Copy)]
enum PageAction {
    Select(usize),
    Rotate(usize),
    Delete(usize),
    Duplicate(usize),
    Crop(usize),
    MoveUp(usize),
    MoveDown(usize),
    Extract(usize),
}

/// Action triggered on pages in Organize Pages grid mode (FR-06.1 - FR-06.11).
#[derive(Debug, Clone, Copy)]
enum OrganizeAction {
    InsertBlank,
    RotateCW,
    RotateCCW,
    Extract,
    Duplicate,
    Delete,
    SingleRotate(usize, i64),
    SingleDelete(usize),
    MoveLeft(usize),
    MoveRight(usize),
}

/// Parses 1-based page range string (e.g. "1-2, 3, 5-6") into 0-based index pairs (start, end) inclusive (FR-07.3).
fn parse_page_ranges(input: &str, total_pages: usize) -> Vec<(usize, usize)> {
    let mut ranges = Vec::new();
    for part in input.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        if let Some((start_s, end_s)) = part.split_once('-') {
            if let (Ok(s), Ok(e)) = (
                start_s.trim().parse::<usize>(),
                end_s.trim().parse::<usize>(),
            ) {
                if s >= 1 && e >= s && s <= total_pages {
                    let end_bounded = e.min(total_pages);
                    ranges.push((s - 1, end_bounded - 1));
                }
            }
        } else if let Ok(p) = part.parse::<usize>() {
            if p >= 1 && p <= total_pages {
                ranges.push((p - 1, p - 1));
            }
        }
    }
    ranges
}

#[derive(Debug, PartialEq, Clone, Copy)]
enum SidebarTab {
    Thumbnails,
    Bookmarks,
    Destinations,
    Annotations,
    Attachments,
    Signatures,
    Forms,
    Properties,
    Search,
    Accessibility,
}

#[derive(Debug, PartialEq, Clone, Copy)]
enum ActiveTool {
    Select,
    Hand,
    Loupe,
    MarqueeZoom,
    StickyNote,
    Highlight,
    Underline,
    Squiggly,
    StrikeOut,
    Rectangle,
    Circle,
    Line,
    Polygon,
    Polyline,
    Pencil,
    Eraser,
    Text,
    Typewriter,
    Callout,
    Link,
    Redact,
    MeasureDistance,
    MeasureArea,
    FormField(FormFieldType),
}

/// Page display layout mode conforming to FR-01.2.2 and FR-06.1.
#[derive(Debug, PartialEq, Clone, Copy)]
pub enum PageViewMode {
    SinglePage,
    ContinuousScroll,
    TwoPageFacing,
    TwoPageContinuous,
    OrganizePages,
}

/// Represents an active or selected annotation on the canvas conforming to FR-08.3 and FR-09.12.
#[derive(Debug, Clone, PartialEq)]
pub struct SelectedAnnotation {
    pub id: ObjectId,
    pub page_index: usize,
    pub annot_type: AnnotationType,
    pub rect: [f64; 4],
    pub contents: Option<String>,
    pub author: Option<String>,
    pub color: Option<(f32, f32, f32)>,
    pub opacity: f32,
    pub is_dragging: bool,
    pub drag_start_rect: [f64; 4],
    pub drag_start_pointer: Pos2,
}

/// Represents an active or selected interactive form field on the canvas conforming to FR-11.1 - FR-11.3.
#[derive(Debug, Clone, PartialEq)]
pub struct SelectedFormField {
    pub id: ObjectId,
    pub page_index: usize,
    pub field_type: FormFieldType,
    pub name: String,
    pub value: Option<String>,
    pub tooltip: Option<String>,
    pub default_value: Option<String>,
    pub rect: [f64; 4],
    pub read_only: bool,
    pub required: bool,
    pub font_size: Option<f32>,
    pub text_color: Option<[f32; 3]>,
    pub is_dragging: bool,
    pub drag_start_rect: [f64; 4],
    pub drag_start_pointer: Pos2,
}

/// Represents a marked redaction region awaiting irreversible execution conforming to FR-15.1, FR-15.6.
#[derive(Debug, Clone, PartialEq)]
pub struct MarkedRedaction {
    pub id: usize,
    pub page_index: usize,
    pub rect: [f64; 4],
    pub appearance: RedactionAppearance,
}

/// State for interactive linear and box text selection conforming to FR-01.1.2, FR-04.6, and FR-04.7.
#[derive(Debug, Clone, Default)]
pub struct TextSelectionState {
    pub is_selecting: bool,
    pub start_pos: Option<Pos2>,
    pub current_pos: Option<Pos2>,
    pub selected_text: String,
    pub selected_spans: Vec<TextSpan>,
}

struct RavPdfApp {
    tabs: Vec<DocumentTab>,
    active_tab_idx: Option<usize>,
    next_tab_id: usize,

    sidebar_tab: SidebarTab,
    sidebar_open: bool,
    active_tool: ActiveTool,
    status_message: String,

    // Search bar state (FR-34)
    search_bar_open: bool,
    search_query: String,
    search_results: Vec<SearchMatch>,
    current_search_idx: usize,

    // Dialog modals (FR-18)
    show_watermark_dialog: bool,
    watermark_text: String,
    watermark_opacity: f32,

    show_bates_dialog: bool,
    bates_prefix: String,
    bates_start: u32,
    bates_digits: usize,

    show_header_footer_dialog: bool,
    hf_config: HeaderFooterConfig,

    // Combine & Merge Dialog (FR-07.1)
    show_combine_dialog: bool,
    combine_file_paths: Vec<PathBuf>,

    // Split Document Dialog (FR-07.3)
    show_split_dialog: bool,
    split_ranges_input: String,

    // Document Properties dialog state (FR-33)
    show_properties_dialog: bool,
    prop_title: String,
    prop_author: String,
    prop_subject: String,
    prop_keywords: String,

    // Redaction dialog state (FR-15.4)
    show_redact_pattern_dialog: bool,
    redact_pattern_input: String,
    redact_replacement_input: String,

    // Encryption dialog state (FR-14)
    show_encrypt_dialog: bool,
    encrypt_user_pass: String,
    encrypt_owner_pass: String,
    encrypt_allow_print: bool,
    encrypt_allow_copy: bool,
    encrypt_allow_modify: bool,
    encrypt_algorithm_idx: usize,

    // Optimization & Space Audit state (FR-25)
    show_optimize_dialog: bool,
    optimization_stats: Option<OptimizationStats>,
    show_space_audit_dialog: bool,
    space_audit: Option<SpaceAudit>,

    // Document Comparison state (FR-17)
    show_compare_dialog: bool,
    compare_file_a: Option<PathBuf>,
    compare_file_b: Option<PathBuf>,
    comparison_report: Option<ComparisonReport>,

    // Preflight state (FR-22, FR-23)
    show_preflight_dialog: bool,
    preflight_profile: PreflightProfile,
    preflight_report: Option<PreflightReport>,

    // Action Wizard / Batch processing state (FR-30)
    show_batch_dialog: bool,
    batch_input_files: Vec<PathBuf>,
    batch_output_dir: Option<PathBuf>,
    batch_action_watermark: bool,
    batch_watermark_text: String,
    batch_action_bates: bool,
    batch_bates_prefix: String,
    batch_action_flatten: bool,
    batch_action_optimize: bool,
    batch_action_linearize: bool,
    batch_action_sanitize: bool,
    batch_result: Option<BatchResult>,

    // Bookmark creation modal state (FR-19.2)
    new_bookmark_title: String,
    // Named destination creation state (FR-20.4)
    new_dest_name: String,

    // Annotation drawing state (FR-08, FR-09)
    drag_start_pos: Option<Pos2>,
    drag_current_pos: Option<Pos2>,
    show_free_text_dialog: bool,
    free_text_content: String,
    free_text_coords: Option<[f64; 4]>,

    // Inline form editing buffer: field_id -> current string value
    form_field_text_buffers: std::collections::HashMap<ravpdf_core::object::ObjectId, String>,

    // Find & Replace state (FR-04.8)
    search_replace_mode: bool,
    search_replace_text: String,
    search_case_sensitive: bool,
    search_whole_word: bool,
    search_is_regex: bool,

    // Page Crop Modal (FR-06.8)
    show_crop_dialog: bool,
    crop_target_page: usize,
    crop_box_type: PageBoxType,
    crop_left: f64,
    crop_bottom: f64,
    crop_right: f64,
    crop_top: f64,
    crop_all_pages: bool,

    // Digital Signatures state (FR-13)
    show_add_signature_dialog: bool,
    sig_mode_idx: usize,
    sig_target_page: usize,
    sig_field_name: String,
    sig_signer_name: String,
    sig_org: String,
    sig_email: String,
    sig_reason: String,
    sig_location: String,
    sig_is_visible: bool,
    sig_is_certify: bool,
    sig_cert_perm_idx: usize,
    sig_embed_ltv: bool,

    // Page Labels state (FR-06.9)
    show_page_labels_dialog: bool,
    page_labels_start_page: usize,
    page_labels_style_idx: usize,
    page_labels_prefix: String,
    page_labels_start_num: u32,

    // Hyperlinks state (FR-20)
    show_link_dialog: bool,
    link_type_idx: usize,
    link_url: String,
    link_target_page: usize,
    link_border_width: f64,
    link_rect: [f64; 4],

    // Interactive Form Creation state (FR-11.1)
    show_add_form_field_dialog: bool,
    form_field_type_idx: usize,
    form_field_name: String,
    form_field_value: String,
    form_field_options_csv: String,
    form_field_rect: [f64; 4],
    form_field_radio_export: String,
    form_field_multiselect: bool,
    form_field_barcode_symbology_idx: usize,

    // Form Data Export / Import & Tab Order dialog state (FR-11.2, FR-11.3)
    show_export_form_dialog: bool,
    form_export_format_idx: usize,
    show_tab_order_dialog: bool,
    tab_order_idx: usize,

    // Rich Media state (FR-27)
    show_rich_media_dialog: bool,
    rich_media_url_or_path: String,
    rich_media_title: String,
    rich_media_is_url: bool,
    rich_media_autoplay: bool,
    rich_media_loop: bool,
    rich_media_rect: [f64; 4],

    // 3D Content state (FR-28)
    show_threed_dialog: bool,
    threed_model_path: Option<PathBuf>,
    threed_render_mode_idx: usize,
    threed_lighting_idx: usize,
    threed_rect: [f64; 4],

    // Accessibility state (FR-21)
    show_accessibility_dialog: bool,
    accessibility_report: Option<AccessibilityReport>,
    accessibility_fix_title_input: String,
    accessibility_fix_lang_input: String,

    // Export Document state (FR-03.2)
    show_export_dialog: bool,
    export_format_idx: usize,
    export_dpi: f32,
    export_pages_all: bool,
    export_pages_range: String,

    // Collaboration & Comments state (FR-31)
    show_comments_summary_dialog: bool,
    comments_list: Vec<ReviewComment>,
    show_merge_comments_dialog: bool,

    // Measurement & Geospatial state (FR-26)
    show_measure_scale_dialog: bool,
    measure_scale: ScaleRatio,
    measure_unit: LinearUnit,
    measure_area_unit: AreaUnit,
    measure_snap: SnapOptions,
    measure_scale_preset_idx: usize,
    measure_scale_custom_drawing_val: f64,
    measure_scale_custom_world_val: f64,
    measure_calib_real_val: f64,
    measure_bake_as_annot: bool,

    // PDF Portfolio Studio state (FR-29.2)
    show_portfolio_dialog: bool,
    portfolio_title_input: String,
    portfolio_desc_input: String,
    portfolio_layout_idx: usize,
    portfolio_input_paths: Vec<PathBuf>,

    // Fast Web View / Linearization state (FR-02.8, FR-25.8)
    show_linearize_dialog: bool,

    // JavaScript Console state (FR-12.9)
    show_js_console_dialog: bool,
    js_console_input: String,
    js_console_output: Vec<String>,

    // Scan & Cleanup state (FR-32)
    show_scan_dialog: bool,
    scan_auto_deskew: bool,
    scan_auto_crop: bool,
    scan_despeckle: bool,
    scan_whiten: bool,
    scan_binarize: bool,
    scan_input_paths: Vec<PathBuf>,

    // Print & Imposition state (FR-35)
    show_print_dialog: bool,
    print_copies: u32,
    print_collate: bool,
    print_pages_range: String,
    print_layout_idx: usize,

    // Plugins state (FR-36)
    show_plugins_dialog: bool,
    plugin_registry: PluginRegistry,

    // Optical Character Recognition (OCR) state (FR-16)
    show_ocr_dialog: bool,
    ocr_lang_idx: usize,
    ocr_mode_idx: usize,
    ocr_dpi: u32,
    ocr_auto_rotate: bool,
    ocr_all_pages: bool,
    ocr_result: Option<OcrDocumentResult>,

    // Output Preview & Plate Separations state (FR-23.2)
    show_output_preview_dialog: bool,
    plate_config: PlateSeparationConfig,
    separation_analysis: Option<SeparationAnalysis>,
    object_inspection_report: Option<ObjectInspectionReport>,

    // Color Management & Soft-Proofing state (FR-24)
    show_color_management_dialog: bool,
    soft_proof_active: bool,
    soft_proof_config: SoftProofConfig,
    color_convert_target_idx: usize,
    output_intent_profile_idx: usize,
    output_intent_condition: String,
    output_intent_registry: String,

    // Prepress Tools state (FR-23.3)
    show_prepress_dialog: bool,
    prepress_tab_idx: usize,
    printer_marks_config: PrinterMarksConfig,
    transparency_flatten_config: TransparencyFlattenConfig,
    new_trap_preset_name: String,
    new_trap_width: f64,
    new_black_width: f64,

    // Standards Compliance Wizard state (FR-22)
    show_standards_dialog: bool,
    standards_format_idx: usize,
    standards_icc_idx: usize,
    standards_remove_js: bool,
    standards_remove_enc: bool,
    standards_convert_cmyk: bool,
    standards_validation_report: Option<StandardValidationReport>,

    // In-Place Text Editor dialog state (FR-04.1 - FR-04.5, FR-04.12)
    show_text_editor_dialog: bool,
    text_editor_spans: Vec<TextSpan>,
    text_editor_selected_idx: Option<usize>,
    text_editor_target_text: String,
    text_editor_new_text: String,
    text_editor_font_idx: usize,
    text_editor_font_size: f64,
    text_editor_block_mode: bool,
    text_editor_align_idx: usize,
    text_editor_underline: bool,
    text_editor_strikethrough: bool,
    text_editor_insert_text: String,

    // Markdown Generation dialog state (FR-02.5, FR-02.6, FR-03.1.6)
    show_markdown_dialog: bool,
    markdown_input: String,
    markdown_title: String,
    markdown_font_size: f64,
    markdown_margin: f64,

    // Image Editor & Transform state (FR-05.1 - FR-05.12)
    show_image_editor_dialog: bool,
    image_editor_list: Vec<PageImage>,
    image_editor_selected_idx: Option<usize>,
    image_editor_pos_x: f64,
    image_editor_pos_y: f64,
    image_editor_width: f64,
    image_editor_height: f64,
    image_editor_rotation: f64,
    image_editor_opacity: f32,
    image_editor_lock_aspect: bool,
    image_editor_flip_h: bool,
    image_editor_flip_v: bool,
    image_editor_crop_x: f64,
    image_editor_crop_y: f64,
    image_editor_crop_w: f64,
    image_editor_crop_h: f64,
    image_editor_snapping_enabled: bool,
    image_editor_active_guides: Vec<SnapGuide>,

    // Rubber Stamps state (FR-10.1 - FR-10.7)
    show_stamp_dialog: bool,
    stamp_type_idx: usize,
    stamp_category_idx: usize,
    stamp_author_input: String,
    stamp_include_dynamic_date: bool,
    stamp_custom_label: String,
    stamp_pos_x: f64,
    stamp_pos_y: f64,
    stamp_width: f64,
    stamp_height: f64,
    stamp_tilt_deg: f64,
    stamp_opacity: f32,
    stamp_note_input: String,

    // Drawing & Shape Tools state (FR-09)
    show_drawing_dialog: bool,
    drawing_category_idx: usize,
    drawing_shape_idx: usize,
    drawing_pos_x: f64,
    drawing_pos_y: f64,
    drawing_width: f64,
    drawing_height: f64,
    drawing_line_start_x: f64,
    drawing_line_start_y: f64,
    drawing_line_end_x: f64,
    drawing_line_end_y: f64,
    drawing_line_start_style_idx: usize,
    drawing_line_end_style_idx: usize,
    drawing_stroke_color: [f32; 3],
    drawing_has_fill: bool,
    drawing_fill_color: [f32; 3],
    drawing_stroke_width: f64,
    drawing_border_dash_idx: usize,
    drawing_is_cloudy: bool,
    drawing_cloudy_intensity: f64,
    drawing_opacity: f32,
    drawing_locked: bool,
    drawing_note_input: String,
    drawing_polygon_vertices_input: String,
    drawing_ink_strokes_input: String,
    drawing_ink_smooth: bool,
    drawing_eraser_x: f64,
    drawing_eraser_y: f64,
    drawing_eraser_radius: f64,

    // Active freehand pencil drawing on canvas (FR-09.1)
    active_pencil_points: Vec<Pos2>,

    // Polygon & Polyline Vector Shapes (FR-09.2, FR-09.3)
    polygon_vertices: Vec<[f64; 2]>,

    // Callout text annotation modal state (FR-08.1.3)
    show_callout_dialog: bool,
    callout_text_input: String,
    callout_box_x: f64,
    callout_box_y: f64,
    callout_box_w: f64,
    callout_box_h: f64,
    callout_leader_x: f64,
    callout_leader_y: f64,
    callout_knee_x: f64,
    callout_knee_y: f64,
    callout_border_color: [f32; 3],

    // Typewriter annotation modal state (FR-08.1.4)
    show_typewriter_dialog: bool,
    typewriter_text_input: String,
    typewriter_pos_x: f64,
    typewriter_pos_y: f64,
    typewriter_font_size: f64,
    typewriter_color: [f32; 3],

    // View mode (Single page vs continuous scroll vs organize pages) (FR-01.2.2, FR-06.1)
    page_view_mode: PageViewMode,

    // Interactive annotation selection & movement (FR-08.3, FR-09.12)
    selected_annotation: Option<SelectedAnnotation>,
    show_annot_inspector: bool,
    annot_inspector_contents: String,
    annot_inspector_author: String,
    annot_inspector_color: [f32; 3],
    annot_inspector_opacity: f32,

    // Interactive Form Field selection, dragging, and inspection (FR-11.1 - FR-11.3)
    selected_form_field: Option<SelectedFormField>,
    show_form_field_inspector: bool,
    form_field_inspector_name: String,
    form_field_inspector_tooltip: String,
    form_field_inspector_default_val: String,
    form_field_inspector_required: bool,
    form_field_inspector_readonly: bool,
    form_field_inspector_font_size: f32,
    form_field_inspector_color: [f32; 3],

    // Marked Redactions and Document Sanitization (FR-15.1 - FR-15.6)
    marked_redactions: Vec<MarkedRedaction>,
    next_redaction_id: usize,
    show_apply_redactions_confirm_dialog: bool,
    show_sanitize_dialog: bool,
    sanitize_options: SanitizeOptions,
    sanitize_report: Option<SanitizeReport>,

    // Organize Pages Multi-Page Selection (FR-06.1 - FR-06.11)
    organize_selected_pages: HashSet<usize>,

    // Canvas text selection and clipboard copy (FR-01.1.2, FR-04.6, FR-04.7)
    text_selection: TextSelectionState,

    // Navigation History Stack (FR-01.2.7)
    nav_history: Vec<(usize, usize)>,
    nav_history_idx: usize,

    // View & Presentation Modes (FR-01.3.3, FR-01.3.5, FR-01.3.6)
    is_full_screen: bool,
    split_view_enabled: bool,
    split_view_second_page: usize,
    loupe_magnification: f32,

    // Form Field Preview Mode (FR-11.3.6)
    form_test_mode: bool,

    // Form Field Duplication Dialog (FR-11.3.5)
    show_duplicate_field_dialog: bool,
    duplicate_field_target_pages_input: String,

    // XFDF Import / Export Dialog (FR-08.3.6)
    show_xfdf_dialog: bool,
    xfdf_buffer: String,

    // Theme and Viewing options (FR-01.2.2, NFR-05)
    dark_theme: bool,
    two_page_cover_page: bool,
    marquee_zoom_start: Option<Pos2>,
}

impl Default for RavPdfApp {
    fn default() -> Self {
        Self {
            tabs: Vec::new(),
            active_tab_idx: None,
            next_tab_id: 1,

            sidebar_tab: SidebarTab::Thumbnails,
            sidebar_open: true,
            active_tool: ActiveTool::Select,
            status_message: "Ready • No document open".into(),

            dark_theme: true,
            two_page_cover_page: false,
            marquee_zoom_start: None,

            search_bar_open: false,
            search_query: String::new(),
            search_results: Vec::new(),
            current_search_idx: 0,

            search_replace_mode: false,
            search_replace_text: String::new(),
            search_case_sensitive: false,
            search_whole_word: false,
            search_is_regex: false,

            show_crop_dialog: false,
            crop_target_page: 0,
            crop_box_type: PageBoxType::CropBox,
            crop_left: 0.0,
            crop_bottom: 0.0,
            crop_right: 612.0,
            crop_top: 792.0,
            crop_all_pages: false,

            show_add_signature_dialog: false,
            sig_mode_idx: 0,
            sig_target_page: 0,
            sig_field_name: "Signature1".into(),
            sig_signer_name: "Authorized Signer".into(),
            sig_org: "Certified Authority".into(),
            sig_email: "signer@organization.org".into(),
            sig_reason: "Document Approved".into(),
            sig_location: "Headquarters".into(),
            sig_is_visible: true,
            sig_is_certify: false,
            sig_cert_perm_idx: 0,
            sig_embed_ltv: false,

            show_page_labels_dialog: false,
            page_labels_start_page: 1,
            page_labels_style_idx: 0,
            page_labels_prefix: String::new(),
            page_labels_start_num: 1,

            show_link_dialog: false,
            link_type_idx: 0,
            link_url: "https://".into(),
            link_target_page: 1,
            link_border_width: 0.0,
            link_rect: [72.0, 700.0, 200.0, 720.0],

            show_add_form_field_dialog: false,
            form_field_type_idx: 0,
            form_field_name: "FieldName1".into(),
            form_field_value: String::new(),
            form_field_options_csv: "Option 1, Option 2, Option 3".into(),
            form_field_rect: [72.0, 650.0, 220.0, 672.0],
            form_field_radio_export: "Choice1".into(),
            form_field_multiselect: false,
            form_field_barcode_symbology_idx: 0,

            show_export_form_dialog: false,
            form_export_format_idx: 0,
            show_tab_order_dialog: false,
            tab_order_idx: 0,

            show_rich_media_dialog: false,
            rich_media_url_or_path: String::new(),
            rich_media_title: "Embedded Media".into(),
            rich_media_is_url: false,
            rich_media_autoplay: false,
            rich_media_loop: false,
            rich_media_rect: [72.0, 400.0, 360.0, 600.0],

            show_threed_dialog: false,
            threed_model_path: None,
            threed_render_mode_idx: 0,
            threed_lighting_idx: 0,
            threed_rect: [72.0, 300.0, 400.0, 600.0],

            show_watermark_dialog: false,
            watermark_text: "CONFIDENTIAL".into(),
            watermark_opacity: 0.3,

            show_bates_dialog: false,
            bates_prefix: "DOC".into(),
            bates_start: 1,
            bates_digits: 6,

            show_header_footer_dialog: false,
            hf_config: HeaderFooterConfig::default(),

            show_combine_dialog: false,
            combine_file_paths: Vec::new(),

            show_split_dialog: false,
            split_ranges_input: "1-2".into(),

            show_properties_dialog: false,
            prop_title: String::new(),
            prop_author: String::new(),
            prop_subject: String::new(),
            prop_keywords: String::new(),

            show_redact_pattern_dialog: false,
            redact_pattern_input: String::new(),
            redact_replacement_input: "[REDACTED]".into(),

            show_encrypt_dialog: false,
            encrypt_user_pass: String::new(),
            encrypt_owner_pass: String::new(),
            encrypt_allow_print: true,
            encrypt_allow_copy: true,
            encrypt_allow_modify: false,
            encrypt_algorithm_idx: 0,

            show_optimize_dialog: false,
            optimization_stats: None,
            show_space_audit_dialog: false,
            space_audit: None,

            show_compare_dialog: false,
            compare_file_a: None,
            compare_file_b: None,
            comparison_report: None,

            show_preflight_dialog: false,
            preflight_profile: PreflightProfile::PdfA1b,
            preflight_report: None,

            show_batch_dialog: false,
            batch_input_files: Vec::new(),
            batch_output_dir: None,
            batch_action_watermark: false,
            batch_watermark_text: "APPROVED".into(),
            batch_action_bates: false,
            batch_bates_prefix: "BATCH".into(),
            batch_action_flatten: false,
            batch_action_optimize: true,
            batch_action_linearize: false,
            batch_action_sanitize: false,
            batch_result: None,

            new_bookmark_title: String::new(),
            new_dest_name: String::new(),

            drag_start_pos: None,
            drag_current_pos: None,
            show_free_text_dialog: false,
            free_text_content: String::new(),
            free_text_coords: None,

            form_field_text_buffers: std::collections::HashMap::new(),

            show_accessibility_dialog: false,
            accessibility_report: None,
            accessibility_fix_title_input: String::new(),
            accessibility_fix_lang_input: "en-US".into(),

            show_export_dialog: false,
            export_format_idx: 0,
            export_dpi: 300.0,
            export_pages_all: true,
            export_pages_range: "1".into(),

            show_comments_summary_dialog: false,
            comments_list: Vec::new(),
            show_merge_comments_dialog: false,

            show_measure_scale_dialog: false,
            measure_scale: ScaleRatio::default(),
            measure_unit: LinearUnit::Inches,
            measure_area_unit: AreaUnit::SquareInches,
            measure_snap: SnapOptions::default(),
            measure_scale_preset_idx: 0,
            measure_scale_custom_drawing_val: 1.0,
            measure_scale_custom_world_val: 1.0,
            measure_calib_real_val: 10.0,
            measure_bake_as_annot: false,

            show_portfolio_dialog: false,
            portfolio_title_input: "PDF Portfolio".into(),
            portfolio_desc_input: String::new(),
            portfolio_layout_idx: 0,
            portfolio_input_paths: Vec::new(),

            show_linearize_dialog: false,

            show_js_console_dialog: false,
            js_console_input: "app.alert('Hello from RavPDF JavaScript!');".to_string(),
            js_console_output: Vec::new(),

            show_scan_dialog: false,
            scan_auto_deskew: true,
            scan_auto_crop: true,
            scan_despeckle: true,
            scan_whiten: true,
            scan_binarize: false,
            scan_input_paths: Vec::new(),

            show_print_dialog: false,
            print_copies: 1,
            print_collate: true,
            print_pages_range: "all".to_string(),
            print_layout_idx: 0,

            show_plugins_dialog: false,
            show_ocr_dialog: false,
            ocr_lang_idx: 0,
            ocr_mode_idx: 0,
            ocr_dpi: 300,
            ocr_auto_rotate: true,
            ocr_all_pages: true,
            ocr_result: None,

            show_output_preview_dialog: false,
            plate_config: PlateSeparationConfig::default(),
            separation_analysis: None,
            object_inspection_report: None,

            show_color_management_dialog: false,
            soft_proof_active: false,
            soft_proof_config: SoftProofConfig::default(),
            color_convert_target_idx: 0,
            output_intent_profile_idx: 3,
            output_intent_condition: "U.S. Web Coated (SWOP) v2".into(),
            output_intent_registry: "http://www.color.org".into(),

            show_prepress_dialog: false,
            prepress_tab_idx: 0,
            printer_marks_config: PrinterMarksConfig::default(),
            transparency_flatten_config: TransparencyFlattenConfig::default(),
            new_trap_preset_name: "Custom Trapping".into(),
            new_trap_width: 0.25,
            new_black_width: 0.50,

            show_standards_dialog: false,
            standards_format_idx: 0,
            standards_icc_idx: 0,
            standards_remove_js: true,
            standards_remove_enc: true,
            standards_convert_cmyk: false,
            standards_validation_report: None,

            show_text_editor_dialog: false,
            text_editor_spans: Vec::new(),
            text_editor_selected_idx: None,
            text_editor_target_text: String::new(),
            text_editor_new_text: String::new(),
            text_editor_font_idx: 0,
            text_editor_font_size: 12.0,
            text_editor_block_mode: false,
            text_editor_align_idx: 0,
            text_editor_underline: false,
            text_editor_strikethrough: false,
            text_editor_insert_text: String::new(),

            show_markdown_dialog: false,
            markdown_input: "# New Document\n\nEnter markdown content here...".to_string(),
            markdown_title: "Document Title".to_string(),
            markdown_font_size: 11.0,
            markdown_margin: 54.0,

            show_image_editor_dialog: false,
            image_editor_list: Vec::new(),
            image_editor_selected_idx: None,
            image_editor_pos_x: 72.0,
            image_editor_pos_y: 500.0,
            image_editor_width: 200.0,
            image_editor_height: 150.0,
            image_editor_rotation: 0.0,
            image_editor_opacity: 1.0,
            image_editor_lock_aspect: true,
            image_editor_flip_h: false,
            image_editor_flip_v: false,
            image_editor_crop_x: 0.0,
            image_editor_crop_y: 0.0,
            image_editor_crop_w: 200.0,
            image_editor_crop_h: 150.0,
            image_editor_snapping_enabled: true,
            image_editor_active_guides: Vec::new(),

            show_stamp_dialog: false,
            stamp_type_idx: 0,
            stamp_category_idx: 0,
            stamp_author_input: "Reviewer".to_string(),
            stamp_include_dynamic_date: true,
            stamp_custom_label: "CONFIDENTIAL".to_string(),
            stamp_pos_x: 72.0,
            stamp_pos_y: 650.0,
            stamp_width: 180.0,
            stamp_height: 56.0,
            stamp_tilt_deg: -8.0,
            stamp_opacity: 0.92,
            stamp_note_input: String::new(),

            show_drawing_dialog: false,
            drawing_category_idx: 0,
            drawing_shape_idx: 0,
            drawing_pos_x: 100.0,
            drawing_pos_y: 500.0,
            drawing_width: 200.0,
            drawing_height: 120.0,
            drawing_line_start_x: 100.0,
            drawing_line_start_y: 400.0,
            drawing_line_end_x: 350.0,
            drawing_line_end_y: 400.0,
            drawing_line_start_style_idx: 0,
            drawing_line_end_style_idx: 4, // OpenArrow
            drawing_stroke_color: [0.85, 0.15, 0.15],
            drawing_has_fill: false,
            drawing_fill_color: [0.95, 0.95, 0.95],
            drawing_stroke_width: 2.0,
            drawing_border_dash_idx: 0,
            drawing_is_cloudy: false,
            drawing_cloudy_intensity: 1.0,
            drawing_opacity: 1.0,
            drawing_locked: false,
            drawing_note_input: String::new(),
            drawing_polygon_vertices_input: "100,100; 250,180; 300,80; 180,40".to_string(),
            drawing_ink_strokes_input: "100,100; 120,130; 160,150; 200,120".to_string(),
            drawing_ink_smooth: true,
            drawing_eraser_x: 150.0,
            drawing_eraser_y: 130.0,
            drawing_eraser_radius: 20.0,

            active_pencil_points: Vec::new(),
            polygon_vertices: Vec::new(),

            show_callout_dialog: false,
            callout_text_input: "Note: Check area details".to_string(),
            callout_box_x: 200.0,
            callout_box_y: 600.0,
            callout_box_w: 160.0,
            callout_box_h: 40.0,
            callout_leader_x: 120.0,
            callout_leader_y: 520.0,
            callout_knee_x: 160.0,
            callout_knee_y: 560.0,
            callout_border_color: [0.1, 0.45, 0.85],

            show_typewriter_dialog: false,
            typewriter_text_input: "Sample Typewriter Text".to_string(),
            typewriter_pos_x: 72.0,
            typewriter_pos_y: 700.0,
            typewriter_font_size: 12.0,
            typewriter_color: [0.0, 0.0, 0.0],

            page_view_mode: PageViewMode::SinglePage,
            selected_annotation: None,
            show_annot_inspector: false,
            annot_inspector_contents: String::new(),
            annot_inspector_author: "User".into(),
            annot_inspector_color: [0.85, 0.15, 0.15],
            annot_inspector_opacity: 1.0,

            selected_form_field: None,
            show_form_field_inspector: false,
            form_field_inspector_name: String::new(),
            form_field_inspector_tooltip: String::new(),
            form_field_inspector_default_val: String::new(),
            form_field_inspector_required: false,
            form_field_inspector_readonly: false,
            form_field_inspector_font_size: 12.0,
            form_field_inspector_color: [0.0, 0.0, 0.0],

            marked_redactions: Vec::new(),
            next_redaction_id: 1,
            show_apply_redactions_confirm_dialog: false,
            show_sanitize_dialog: false,
            sanitize_options: SanitizeOptions::default(),
            sanitize_report: None,

            organize_selected_pages: HashSet::new(),

            text_selection: TextSelectionState::default(),

            nav_history: Vec::new(),
            nav_history_idx: 0,
            is_full_screen: false,
            split_view_enabled: false,
            split_view_second_page: 0,
            loupe_magnification: 3.0,
            form_test_mode: false,
            show_duplicate_field_dialog: false,
            duplicate_field_target_pages_input: "1".into(),
            show_xfdf_dialog: false,
            xfdf_buffer: String::new(),

            plugin_registry: {
                let mut reg = PluginRegistry::new();
                let sample = Box::new(SampleWatermarkPlugin::new());
                let perms = [
                    PluginPermission::UiExtension,
                    PluginPermission::DocumentTransform,
                ];
                let _ = reg.register_plugin(sample, &perms);
                reg
            },
        }
    }
}

impl RavPdfApp {
    /// Records page navigation into the history stack (FR-01.2.7).
    pub fn record_page_navigation(&mut self, target_page: usize) {
        let (tab_id, current) = match self.active_tab() {
            Some(tab) => {
                if tab.current_page == target_page {
                    return;
                }
                (tab.id, tab.current_page)
            }
            None => return,
        };

        if self.nav_history_idx < self.nav_history.len() {
            self.nav_history.truncate(self.nav_history_idx + 1);
        }
        if self.nav_history.is_empty() || self.nav_history.last() != Some(&(tab_id, current)) {
            self.nav_history.push((tab_id, current));
        }
        self.nav_history.push((tab_id, target_page));
        self.nav_history_idx = self.nav_history.len().saturating_sub(1);

        if let Some(tab) = self.active_tab_mut() {
            tab.current_page = target_page;
            tab.page_input_buffer = (target_page + 1).to_string();
            tab.invalidate_render();
        }
    }

    /// Navigates to previous view in the history stack (FR-01.2.7).
    pub fn navigate_back(&mut self) {
        if self.nav_history_idx > 0 && !self.nav_history.is_empty() {
            self.nav_history_idx -= 1;
            let (target_tab_id, target_page) = self.nav_history[self.nav_history_idx];
            if let Some(tab_idx) = self.tabs.iter().position(|t| t.id == target_tab_id) {
                self.active_tab_idx = Some(tab_idx);
                let tab = &mut self.tabs[tab_idx];
                if target_page < tab.doc.page_count() {
                    tab.current_page = target_page;
                    tab.page_input_buffer = (target_page + 1).to_string();
                    tab.invalidate_render();
                    self.status_message = format!("Previous View: Page {}", target_page + 1);
                }
            }
        }
    }

    /// Navigates to next view in the history stack (FR-01.2.7).
    pub fn navigate_forward(&mut self) {
        if self.nav_history_idx + 1 < self.nav_history.len() {
            self.nav_history_idx += 1;
            let (target_tab_id, target_page) = self.nav_history[self.nav_history_idx];
            if let Some(tab_idx) = self.tabs.iter().position(|t| t.id == target_tab_id) {
                self.active_tab_idx = Some(tab_idx);
                let tab = &mut self.tabs[tab_idx];
                if target_page < tab.doc.page_count() {
                    tab.current_page = target_page;
                    tab.page_input_buffer = (target_page + 1).to_string();
                    tab.invalidate_render();
                    self.status_message = format!("Next View: Page {}", target_page + 1);
                }
            }
        }
    }

    /// Returns a reference to the currently active document tab, if any.
    pub fn active_tab(&self) -> Option<&DocumentTab> {
        self.active_tab_idx.and_then(|idx| self.tabs.get(idx))
    }

    /// Returns a mutable reference to the currently active document tab, if any.
    pub fn active_tab_mut(&mut self) -> Option<&mut DocumentTab> {
        self.active_tab_idx.and_then(|idx| self.tabs.get_mut(idx))
    }

    /// Opens a PDF document from a file path into a new tab (FR-01.4.1).
    pub fn open_file(&mut self, path: PathBuf) {
        let raw_bytes = std::fs::read(&path).ok();
        match PdfDocument::load_from_file(&path) {
            Ok(doc) => {
                let file_name = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("Document.pdf")
                    .to_string();

                let id = self.next_tab_id;
                self.next_tab_id += 1;

                let mut tab = DocumentTab::new(id, file_name, doc, Some(path.clone()));
                tab.raw_file_bytes = raw_bytes;
                self.tabs.push(tab);
                self.active_tab_idx = Some(self.tabs.len() - 1);

                self.status_message = format!("Opened: {}", path.display());
            }
            Err(e) => {
                self.status_message = format!("Error opening file: {}", e);
            }
        }
    }

    /// Creates a new blank PDF document in a new tab (FR-02.1).
    pub fn create_new_blank_document(&mut self) {
        let mut doc = PdfDocument::new();
        doc.add_blank_page(paper_sizes::LETTER.0, paper_sizes::LETTER.1);

        let id = self.next_tab_id;
        self.next_tab_id += 1;

        let title = format!("Untitled-{}.pdf", id);
        let mut tab = DocumentTab::new(id, title, doc, None);
        tab.is_modified = true;

        self.tabs.push(tab);
        self.active_tab_idx = Some(self.tabs.len() - 1);
        self.status_message = "Created new blank document".into();
    }

    /// Closes a tab by index.
    pub fn close_tab(&mut self, idx: usize) {
        if idx < self.tabs.len() {
            self.tabs.remove(idx);
            if self.tabs.is_empty() {
                self.active_tab_idx = None;
                self.status_message = "Ready • No document open".into();
                self.search_results.clear();
                self.search_bar_open = false;
            } else {
                let new_idx = idx.min(self.tabs.len() - 1);
                self.active_tab_idx = Some(new_idx);
                self.status_message = format!("Active: {}", self.tabs[new_idx].title);
            }
        }
    }

    /// Shows native file dialog to open a PDF file (EI-01.7).
    fn open_file_dialog(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("PDF Document", &["pdf"])
            .pick_file()
        {
            self.open_file(path);
        }
    }

    /// Shows native file dialog to save active PDF document.
    fn save_file_dialog(&mut self) {
        if let Some(tab) = self.active_tab_mut() {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("PDF Document", &["pdf"])
                .save_file()
            {
                tab.file_path = Some(path.clone());
                tab.title = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("Document.pdf")
                    .to_string();

                match tab.save() {
                    Ok(_) => {
                        self.status_message = format!("Saved: {}", path.display());
                    }
                    Err(e) => {
                        self.status_message = format!("Save error: {}", e);
                    }
                }
            }
        }
    }

    /// Creates a new PDF from selected image files (FR-02.4).
    fn create_pdf_from_images_dialog(&mut self) {
        if let Some(files) = rfd::FileDialog::new()
            .add_filter(
                "Image Files",
                &["png", "jpg", "jpeg", "bmp", "webp", "tiff", "gif"],
            )
            .pick_files()
        {
            if files.is_empty() {
                return;
            }
            match create_pdf_from_images(&files, &ImagePdfOptions::default()) {
                Ok(doc) => {
                    let id = self.next_tab_id;
                    self.next_tab_id += 1;
                    let title = format!("Images ({} pages).pdf", files.len());
                    let mut tab = DocumentTab::new(id, title, doc, None);
                    tab.is_modified = true;
                    self.tabs.push(tab);
                    self.active_tab_idx = Some(self.tabs.len() - 1);
                    self.status_message = format!("Created PDF from {} images", files.len());
                }
                Err(e) => {
                    self.status_message = format!("Failed to create PDF from images: {}", e);
                }
            }
        }
    }

    /// Creates a new PDF from a plain text file (FR-02.5).
    fn create_pdf_from_text_dialog(&mut self) {
        if let Some(file) = rfd::FileDialog::new()
            .add_filter("Plain Text File", &["txt"])
            .pick_file()
        {
            match std::fs::read_to_string(&file) {
                Ok(content) => match create_pdf_from_text(&content, &TextPdfOptions::default()) {
                    Ok(doc) => {
                        let id = self.next_tab_id;
                        self.next_tab_id += 1;
                        let title = file
                            .file_name()
                            .and_then(|n| n.to_str())
                            .map(|n| format!("{}.pdf", n))
                            .unwrap_or_else(|| "Text Document.pdf".into());
                        let mut tab = DocumentTab::new(id, title, doc, None);
                        tab.is_modified = true;
                        self.tabs.push(tab);
                        self.active_tab_idx = Some(self.tabs.len() - 1);
                        self.status_message =
                            format!("Created PDF from text file '{}'", file.display());
                    }
                    Err(e) => {
                        self.status_message = format!("Failed to create PDF from text: {}", e);
                    }
                },
                Err(e) => {
                    self.status_message = format!("Failed to read text file: {}", e);
                }
            }
        }
    }

    /// Creates a new PDF from a Rich Text Format (.rtf) file (FR-03.1.6).
    fn create_pdf_from_rtf_dialog(&mut self) {
        if let Some(file) = rfd::FileDialog::new()
            .add_filter("Rich Text Format", &["rtf"])
            .pick_file()
        {
            match std::fs::read_to_string(&file) {
                Ok(content) => match create_pdf_from_rtf(&content, &TextPdfOptions::default()) {
                    Ok(doc) => {
                        let id = self.next_tab_id;
                        self.next_tab_id += 1;
                        let title = file
                            .file_name()
                            .and_then(|n| n.to_str())
                            .map(|n| format!("{}.pdf", n))
                            .unwrap_or_else(|| "RTF Document.pdf".into());
                        let mut tab = DocumentTab::new(id, title, doc, None);
                        tab.is_modified = true;
                        self.tabs.push(tab);
                        self.active_tab_idx = Some(self.tabs.len() - 1);
                        self.status_message =
                            format!("Created PDF from RTF file '{}'", file.display());
                    }
                    Err(e) => {
                        self.status_message = format!("Failed to create PDF from RTF: {}", e);
                    }
                },
                Err(e) => {
                    self.status_message = format!("Failed to read RTF file: {}", e);
                }
            }
        }
    }

    /// Opens the in-place text editor dialog and extracts text spans from current page (FR-04.1 - FR-04.5, FR-04.12).
    fn open_text_editor_dialog(&mut self) {
        if let Some(tab) = self.active_tab() {
            let curr = tab.current_page;
            match extract_page_text_spans(&tab.doc, curr) {
                Ok(spans) => {
                    self.text_editor_spans = spans;
                    self.text_editor_selected_idx = None;
                    self.text_editor_target_text.clear();
                    self.text_editor_new_text.clear();
                    self.show_text_editor_dialog = true;
                    self.status_message = format!("Extracted text spans from page {}", curr + 1);
                }
                Err(e) => {
                    self.status_message = format!("Failed to extract page text: {}", e);
                }
            }
        }
    }

    /// Opens the image editor dialog and inspects all image XObjects on the active page (FR-05.1 - FR-05.12).
    fn open_image_editor_dialog(&mut self) {
        if let Some(tab) = self.active_tab() {
            let curr = tab.current_page;
            match extract_page_images(&tab.doc, curr) {
                Ok(imgs) => {
                    self.image_editor_list = imgs;
                    if !self.image_editor_list.is_empty() {
                        self.image_editor_selected_idx = Some(0);
                        let first = &self.image_editor_list[0];
                        self.image_editor_pos_x = first.display_x;
                        self.image_editor_pos_y = first.display_y;
                        self.image_editor_width = first.display_width;
                        self.image_editor_height = first.display_height;
                        self.image_editor_rotation = first.rotation_deg;
                        self.image_editor_opacity = first.opacity;
                        self.image_editor_flip_h = first.flipped_h;
                        self.image_editor_flip_v = first.flipped_v;
                        if let Some(c) = first.clip_rect {
                            self.image_editor_crop_x = c[0];
                            self.image_editor_crop_y = c[1];
                            self.image_editor_crop_w = c[2];
                            self.image_editor_crop_h = c[3];
                        } else {
                            self.image_editor_crop_x = first.display_x;
                            self.image_editor_crop_y = first.display_y;
                            self.image_editor_crop_w = first.display_width;
                            self.image_editor_crop_h = first.display_height;
                        }
                    } else {
                        self.image_editor_selected_idx = None;
                    }
                    self.show_image_editor_dialog = true;
                    self.status_message = format!(
                        "Loaded {} image(s) from page {}",
                        self.image_editor_list.len(),
                        curr + 1
                    );
                }
                Err(e) => {
                    self.status_message = format!("Failed to extract page images: {}", e);
                }
            }
        }
    }

    /// Opens the Rubber Stamps modal dialog (FR-10.1 - FR-10.7).
    fn open_stamp_dialog(&mut self) {
        if let Some(tab) = self.active_tab() {
            let curr = tab.current_page;
            if let Some(page) = tab.doc.get_page(curr) {
                let w = (page.crop_box[2] - page.crop_box[0]).abs();
                let h = (page.crop_box[3] - page.crop_box[1]).abs();
                self.stamp_pos_x = ((w - self.stamp_width).max(36.0)) / 2.0;
                self.stamp_pos_y = h * 0.75;
            }
            self.show_stamp_dialog = true;
        }
    }

    /// Exports all pages of the active document as PNG images (FR-03.2.4).
    fn export_pages_as_png_dialog(&mut self) {
        if let Some(tab) = self.active_tab() {
            if let Some(folder) = rfd::FileDialog::new().pick_folder() {
                let page_count = tab.doc.page_count();
                let mut exported = 0;
                let options = RenderOptions {
                    zoom: 2.0, // 144 DPI
                    rotation: 0,
                    background_color: ravpdf_render::Color::WHITE,
                };
                for i in 0..page_count {
                    if let Ok(pixmap) = render_page(&tab.doc, i, &options) {
                        if let Ok(png_data) = pixmap_to_png(&pixmap) {
                            let out_path = folder.join(format!("page_{}.png", i + 1));
                            if std::fs::write(&out_path, png_data).is_ok() {
                                exported += 1;
                            }
                        }
                    }
                }
                self.status_message = format!(
                    "Exported {}/{} pages as PNG to '{}'",
                    exported,
                    page_count,
                    folder.display()
                );
            }
        }
    }

    /// Exports all text content of the active document to a .txt file (FR-03.2.5).
    fn export_text_dialog(&mut self) {
        if let Some(tab) = self.active_tab() {
            if let Some(file) = rfd::FileDialog::new()
                .add_filter("Text File", &["txt"])
                .save_file()
            {
                match extract_all_text(&tab.doc) {
                    Ok(text) => {
                        if std::fs::write(&file, text).is_ok() {
                            self.status_message = format!("Exported text to '{}'", file.display());
                        } else {
                            self.status_message = "Failed to write exported text file".into();
                        }
                    }
                    Err(e) => {
                        self.status_message = format!("Failed to extract text: {}", e);
                    }
                }
            }
        }
    }

    /// Exports document to HTML file with structure and embedded styling (FR-03.2.6).
    fn export_html_dialog(&mut self) {
        if let Some(tab) = self.active_tab() {
            if let Some(file) = rfd::FileDialog::new()
                .add_filter("HTML Document", &["html", "htm"])
                .save_file()
            {
                match export_to_html(&tab.doc, None) {
                    Ok(html) => {
                        if std::fs::write(&file, html).is_ok() {
                            self.status_message = format!("Exported HTML to '{}'", file.display());
                        } else {
                            self.status_message = "Failed to write HTML export file".into();
                        }
                    }
                    Err(e) => {
                        self.status_message = format!("HTML export error: {}", e);
                    }
                }
            }
        }
    }

    /// Exports document to RTF file with structure and embedded styling (FR-03.2.7).
    fn export_rtf_dialog(&mut self) {
        if let Some(tab) = self.active_tab() {
            if let Some(file) = rfd::FileDialog::new()
                .add_filter("Rich Text Format", &["rtf"])
                .save_file()
            {
                match export_to_rtf(&tab.doc, None) {
                    Ok(rtf) => {
                        if std::fs::write(&file, rtf).is_ok() {
                            self.status_message = format!("Exported RTF to '{}'", file.display());
                        } else {
                            self.status_message = "Failed to write RTF export file".into();
                        }
                    }
                    Err(e) => {
                        self.status_message = format!("RTF export error: {}", e);
                    }
                }
            }
        }
    }

    /// Exports document tables to CSV file (FR-03.2.2).
    fn export_csv_dialog(&mut self) {
        if let Some(tab) = self.active_tab() {
            if let Some(file) = rfd::FileDialog::new()
                .add_filter("CSV Spreadsheet", &["csv"])
                .save_file()
            {
                match export_to_csv_table(&tab.doc, None) {
                    Ok(csv) => {
                        if std::fs::write(&file, csv).is_ok() {
                            self.status_message = format!("Exported CSV to '{}'", file.display());
                        } else {
                            self.status_message = "Failed to write CSV export file".into();
                        }
                    }
                    Err(e) => {
                        self.status_message = format!("CSV export error: {}", e);
                    }
                }
            }
        }
    }

    /// Opens the Document Properties dialog and populates editable fields (FR-33).
    fn open_properties_dialog(&mut self) {
        let meta = self.active_tab().and_then(|t| t.metadata.clone());
        if let Some(meta) = meta {
            self.prop_title = meta.title.unwrap_or_default();
            self.prop_author = meta.author.unwrap_or_default();
            self.prop_subject = meta.subject.unwrap_or_default();
            self.prop_keywords = meta.keywords.unwrap_or_default();
        } else {
            self.prop_title.clear();
            self.prop_author.clear();
            self.prop_subject.clear();
            self.prop_keywords.clear();
        }
        self.show_properties_dialog = true;
    }

    /// Inserts a standard rubber stamp onto the current page (FR-10.1 - FR-10.4).
    pub fn insert_rubber_stamp(&mut self, stamp_name: &str, color: (f32, f32, f32)) {
        if let Some(tab) = self.active_tab_mut() {
            let curr_page = tab.current_page;
            if let Some(page) = tab.doc.get_page(curr_page) {
                let p_w = (page.crop_box[2] - page.crop_box[0]).abs();
                let p_h = (page.crop_box[3] - page.crop_box[1]).abs();

                // Position stamp at top-right of page
                let stamp_w = 160.0;
                let stamp_h = 48.0;
                let x1 = page.crop_box[0] + (p_w - stamp_w - 40.0).max(10.0);
                let y1 = page.crop_box[1] + (p_h - stamp_h - 40.0).max(10.0);
                let rect = [x1, y1, x1 + stamp_w, y1 + stamp_h];

                let mut annot =
                    Annotation::new(AnnotationType::Stamp(stamp_name.to_string()), rect);
                annot.color = Some(color);
                let _ = add_annotation_to_page(&mut tab.doc, curr_page, &annot);
                tab.is_modified = true;
                tab.invalidate_render();
                self.status_message =
                    format!("Inserted '{}' stamp on page {}", stamp_name, curr_page + 1);
            }
        }
    }

    /// Executes full-text search on the active document (FR-34.1).
    fn execute_search(&mut self) {
        self.search_results.clear();
        self.current_search_idx = 0;

        if let Some(tab) = self.active_tab() {
            if !self.search_query.trim().is_empty() {
                if let Ok(matches) = search_document(
                    &tab.doc,
                    &self.search_query,
                    self.search_is_regex,
                    self.search_case_sensitive,
                ) {
                    self.search_results = matches;
                    if let Some(first) = self.search_results.first() {
                        let page_idx = first.page_index;
                        if let Some(t) = self.active_tab_mut() {
                            t.current_page = page_idx;
                            t.page_input_buffer = (page_idx + 1).to_string();
                            t.invalidate_render();
                        }
                    }
                }
            }
        }
    }

    /// Opens file dialog to insert an image onto the current page (FR-05.1).
    fn insert_image_dialog(&mut self) {
        if let Some(tab) = self.active_tab_mut() {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("Images", &["png", "jpg", "jpeg", "bmp", "gif", "webp"])
                .pick_file()
            {
                let curr = tab.current_page;
                let (page_w, page_h) = if let Some(p) = tab.doc.get_page(curr) {
                    (
                        (p.media_box[2] - p.media_box[0]).abs(),
                        (p.media_box[3] - p.media_box[1]).abs(),
                    )
                } else {
                    (612.0, 792.0)
                };

                let img_w = 200.0;
                let img_h = 200.0;
                let x = (page_w - img_w) * 0.5;
                let y = (page_h - img_h) * 0.5;

                match insert_image_file(&mut tab.doc, curr, &path, x, y, img_w, img_h) {
                    Ok(()) => {
                        tab.is_modified = true;
                        tab.invalidate_render();
                        let fname = path.file_name().and_then(|n| n.to_str()).unwrap_or("image");
                        self.status_message =
                            format!("Inserted image '{}' onto page {}", fname, curr + 1);
                    }
                    Err(e) => {
                        self.status_message = format!("Failed to insert image: {}", e);
                    }
                }
            }
        }
    }

    /// Handles keyboard shortcuts (FR-01.2.1, FR-34.1).
    fn handle_keyboard_shortcuts(&mut self, ctx: &egui::Context) {
        let has_doc = self.active_tab().is_some();
        let total_pages = self.active_tab().map(|t| t.doc.page_count()).unwrap_or(0);

        ctx.input(|i| {
            // Open / New / Save
            if i.modifiers.ctrl && i.key_pressed(Key::O) {
                self.open_file_dialog();
            }
            if i.modifiers.ctrl && i.key_pressed(Key::N) {
                self.create_new_blank_document();
            }
            if i.modifiers.ctrl && i.key_pressed(Key::S) && has_doc {
                let needs_save_dialog = if let Some(tab) = self.active_tab_mut() {
                    if tab.file_path.is_some() {
                        let _ = tab.save();
                        self.status_message = format!("Saved: {}", tab.title);
                        false
                    } else {
                        true
                    }
                } else {
                    false
                };
                if needs_save_dialog {
                    self.save_file_dialog();
                }
            }
            if i.modifiers.ctrl && i.key_pressed(Key::W) && has_doc {
                if let Some(idx) = self.active_tab_idx {
                    self.close_tab(idx);
                }
            }

            // Find (Ctrl+F)
            if i.modifiers.ctrl && i.key_pressed(Key::F) && has_doc {
                self.search_bar_open = !self.search_bar_open;
                self.search_replace_mode = false;
            }

            // Find & Replace (Ctrl+H) (FR-04.8)
            if i.modifiers.ctrl && i.key_pressed(Key::H) && has_doc {
                self.search_bar_open = true;
                self.search_replace_mode = true;
            }

            // Document Properties (Ctrl+D)
            if i.modifiers.ctrl && i.key_pressed(Key::D) && has_doc {
                self.open_properties_dialog();
            }

            // Print (Ctrl+P) (FR-35)
            if i.modifiers.ctrl && i.key_pressed(Key::P) && has_doc {
                self.show_print_dialog = true;
            }

            // JavaScript Console (Ctrl+J) (FR-12.9)
            if i.modifiers.ctrl && i.key_pressed(Key::J) && has_doc {
                self.show_js_console_dialog = true;
            }

            // Zoom shortcuts
            if i.modifiers.ctrl
                && (i.key_pressed(Key::Plus) || i.key_pressed(Key::Equals))
                && has_doc
            {
                if let Some(tab) = self.active_tab_mut() {
                    tab.target_zoom = (tab.target_zoom * 1.25).min(64.0);
                    ctx.request_repaint();
                }
            }
            if i.modifiers.ctrl && i.key_pressed(Key::Minus) && has_doc {
                if let Some(tab) = self.active_tab_mut() {
                    tab.target_zoom = (tab.target_zoom / 1.25).max(0.1);
                    ctx.request_repaint();
                }
            }

            // Full Screen Presentation Mode (F11 / Ctrl+L / Esc) (FR-01.3.6)
            if self.is_full_screen && i.key_pressed(Key::Escape) {
                self.is_full_screen = false;
            }
            if (i.key_pressed(Key::F11) || (i.modifiers.ctrl && i.key_pressed(Key::L))) && has_doc {
                self.is_full_screen = !self.is_full_screen;
            }

            // Split View toggle (Ctrl+Shift+T) (FR-01.3.5)
            if i.modifiers.ctrl && i.modifiers.shift && i.key_pressed(Key::T) && has_doc {
                self.split_view_enabled = !self.split_view_enabled;
                if self.split_view_enabled {
                    self.split_view_second_page = self
                        .active_tab()
                        .map(|t| (t.current_page + 1).min(total_pages.saturating_sub(1)))
                        .unwrap_or(0);
                }
            }

            // Navigation history (Alt+Left / Alt+Right) (FR-01.2.7)
            if i.modifiers.alt && i.key_pressed(Key::ArrowLeft) {
                self.navigate_back();
            }
            if i.modifiers.alt && i.key_pressed(Key::ArrowRight) {
                self.navigate_forward();
            }

            // Navigation shortcuts (FR-01.2.1)
            if has_doc && total_pages > 0 {
                let nav_step = if self.page_view_mode == PageViewMode::TwoPageFacing {
                    2
                } else {
                    1
                };
                if let Some(tab) = self.active_tab_mut() {
                    if (i.key_pressed(Key::PageDown) || i.key_pressed(Key::ArrowRight))
                        && tab.current_page + 1 < total_pages
                    {
                        tab.current_page =
                            (tab.current_page + nav_step).min(total_pages.saturating_sub(1));
                        tab.page_input_buffer = (tab.current_page + 1).to_string();
                        tab.invalidate_render();
                    }
                    if (i.key_pressed(Key::PageUp) || i.key_pressed(Key::ArrowLeft))
                        && tab.current_page > 0
                    {
                        tab.current_page = tab.current_page.saturating_sub(nav_step);
                        tab.page_input_buffer = (tab.current_page + 1).to_string();
                        tab.invalidate_render();
                    }
                    if i.key_pressed(Key::Home) {
                        tab.current_page = 0;
                        tab.page_input_buffer = "1".into();
                        tab.invalidate_render();
                    }
                    if i.key_pressed(Key::End) {
                        tab.current_page = total_pages.saturating_sub(1);
                        tab.page_input_buffer = (tab.current_page + 1).to_string();
                        tab.invalidate_render();
                    }
                }
            }
        });
    }

    /// Renders the multi-tab strip (FR-01.4).
    fn render_tab_bar(&mut self, ctx: &egui::Context) {
        if self.tabs.is_empty() {
            return;
        }

        let mut tab_to_close: Option<usize> = None;
        let mut tab_to_select: Option<usize> = None;
        let mut create_new = false;

        egui::TopBottomPanel::top("tab_bar")
            .min_height(30.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(3.0, 0.0);

                    for (idx, tab) in self.tabs.iter().enumerate() {
                        let is_active = self.active_tab_idx == Some(idx);
                        let mod_mark = if tab.is_modified { "● " } else { "" };
                        let display_title = if tab.title.len() > 26 {
                            format!("{}…", &tab.title[..24])
                        } else {
                            tab.title.clone()
                        };

                        let (bg_color, stroke_color, text_color) = if is_active {
                            (
                                Color32::from_rgb(37, 99, 235), // Accent blue
                                Color32::from_rgb(59, 130, 246),
                                Color32::WHITE,
                            )
                        } else {
                            (
                                Color32::from_rgb(34, 34, 39),
                                Color32::from_rgb(55, 55, 63),
                                Color32::from_rgb(180, 180, 188),
                            )
                        };

                        egui::Frame::none()
                            .fill(bg_color)
                            .stroke(Stroke::new(1.0_f32, stroke_color))
                            .rounding(egui::Rounding {
                                nw: 5.0,
                                ne: 5.0,
                                sw: 0.0,
                                se: 0.0,
                            })
                            .inner_margin(egui::Margin::symmetric(10.0, 5.0))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    let label = egui::RichText::new(format!("📄 {}{}", mod_mark, display_title))
                                        .color(text_color)
                                        .size(12.0);
                                    let resp = ui.add(egui::Label::new(label).sense(egui::Sense::click()));
                                    if resp.clicked() {
                                        tab_to_select = Some(idx);
                                    }
                                    if ui
                                        .small_button("✕")
                                        .on_hover_text("Close tab (Ctrl+W)")
                                        .clicked()
                                    {
                                        tab_to_close = Some(idx);
                                    }
                                });
                            });
                    }

                    // Plus button to open new tab
                    if ui
                        .button(" ➕ ")
                        .on_hover_text("New blank document (Ctrl+N)")
                        .clicked()
                    {
                        create_new = true;
                    }
                });
            });

        if let Some(idx) = tab_to_select {
            self.active_tab_idx = Some(idx);
            self.status_message = format!("Active: {}", self.tabs[idx].title);
        }
        if let Some(idx) = tab_to_close {
            self.close_tab(idx);
        }
        if create_new {
            self.create_new_blank_document();
        }
    }

    /// Renders top application menu bar (EI-01.1).
    fn render_menu_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                let has_doc = self.active_tab().is_some();

                ui.menu_button("File", |ui| {
                    if ui.button("New Blank PDF (Ctrl+N)").clicked() {
                        self.create_new_blank_document();
                        ui.close_menu();
                    }
                    if ui.button("New PDF from Images...").clicked() {
                        self.create_pdf_from_images_dialog();
                        ui.close_menu();
                    }
                    if ui.button("New PDF from Text File...").clicked() {
                        self.create_pdf_from_text_dialog();
                        ui.close_menu();
                    }
                    if ui.button("New PDF from RTF File...").clicked() {
                        self.create_pdf_from_rtf_dialog();
                        ui.close_menu();
                    }
                    if ui
                        .button("New PDF from Markdown...")
                        .clicked()
                    {
                        self.show_markdown_dialog = true;
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Open... (Ctrl+O)").clicked() {
                        self.open_file_dialog();
                        ui.close_menu();
                    }
                    if ui.button("Combine / Merge Files...").clicked() {
                        self.show_combine_dialog = true;
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui
                        .add_enabled(has_doc, Button::new("Save (Ctrl+S)"))
                        .clicked()
                    {
                        let needs_save_dialog = if let Some(tab) = self.active_tab_mut() {
                            if tab.file_path.is_some() {
                                let _ = tab.save();
                                self.status_message = format!("Saved: {}", tab.title);
                                false
                            } else {
                                true
                            }
                        } else {
                            false
                        };
                        if needs_save_dialog {
                            self.save_file_dialog();
                        }
                        ui.close_menu();
                    }
                    if ui.add_enabled(has_doc, Button::new("Save As...")).clicked() {
                        self.save_file_dialog();
                        ui.close_menu();
                    }
                    ui.separator();
                    ui.menu_button("Export", |ui| {
                        if ui
                            .add_enabled(
                                has_doc,
                                Button::new("Export Document / Pages..."),
                            )
                            .clicked()
                        {
                            self.show_export_dialog = true;
                            ui.close_menu();
                        }
                        if ui
                            .add_enabled(has_doc, Button::new("Export HTML (.html)..."))
                            .clicked()
                        {
                            self.export_html_dialog();
                            ui.close_menu();
                        }
                        if ui
                            .add_enabled(has_doc, Button::new("Export Pages as PNG..."))
                            .clicked()
                        {
                            self.export_pages_as_png_dialog();
                            ui.close_menu();
                        }
                        if ui
                            .add_enabled(has_doc, Button::new("Export Rich Text (.rtf)..."))
                            .clicked()
                        {
                            self.export_rtf_dialog();
                            ui.close_menu();
                        }
                        if ui
                            .add_enabled(has_doc, Button::new("Export Table as CSV (.csv)..."))
                            .clicked()
                        {
                            self.export_csv_dialog();
                            ui.close_menu();
                        }
                        if ui
                            .add_enabled(has_doc, Button::new("Export Text (.txt)..."))
                            .clicked()
                        {
                            self.export_text_dialog();
                            ui.close_menu();
                        }
                    });
                    ui.separator();
                    if ui
                        .add_enabled(has_doc, Button::new("Document Properties... (Ctrl+D)"))
                        .clicked()
                    {
                        self.open_properties_dialog();
                        ui.close_menu();
                    }
                    if ui
                        .add_enabled(has_doc, Button::new("Reduce File Size..."))
                        .clicked()
                    {
                        self.show_optimize_dialog = true;
                        ui.close_menu();
                    }
                    if ui.button("Create PDF Portfolio...").clicked() {
                        self.show_portfolio_dialog = true;
                        ui.close_menu();
                    }
                    if ui
                        .add_enabled(
                            has_doc,
                            Button::new("Linearize for Fast Web View..."),
                        )
                        .clicked()
                    {
                        self.show_linearize_dialog = true;
                        ui.close_menu();
                    }
                    if ui.button("Scan Documents to PDF...").clicked() {
                        self.show_scan_dialog = true;
                        ui.close_menu();
                    }
                    if ui
                        .add_enabled(has_doc, Button::new("Print... (Ctrl+P)"))
                        .clicked()
                    {
                        self.show_print_dialog = true;
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui
                        .add_enabled(has_doc, Button::new("Close Tab (Ctrl+W)"))
                        .clicked()
                    {
                        if let Some(idx) = self.active_tab_idx {
                            self.close_tab(idx);
                        }
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Exit").clicked() {
                        std::process::exit(0);
                    }
                });

                ui.menu_button("Edit", |ui| {
                    if ui
                        .add_enabled(
                            !self.text_selection.selected_text.is_empty(),
                            Button::new("Copy Selected Text (Ctrl+C)"),
                        )
                        .clicked()
                    {
                        ctx.output_mut(|o| {
                            o.copied_text = self.text_selection.selected_text.clone()
                        });
                        self.status_message = format!(
                            "Copied {} chars to clipboard",
                            self.text_selection.selected_text.len()
                        );
                        ui.close_menu();
                    }
                    if ui
                        .add_enabled(
                            has_doc,
                            Button::new("Select All Text on Page (Ctrl+A)"),
                        )
                        .clicked()
                    {
                        if let Some(tab) = self.active_tab() {
                            let spans = extract_page_text_spans(&tab.doc, tab.current_page)
                                .unwrap_or_default();
                            self.text_selection.selected_text = spans
                                .iter()
                                .map(|s| s.text.as_str())
                                .collect::<Vec<_>>()
                                .join(" ");
                            self.text_selection.selected_spans = spans;
                            self.status_message = format!(
                                "Selected all text ({} chars)",
                                self.text_selection.selected_text.len()
                            );
                        }
                        ui.close_menu();
                    }
                    if ui
                        .add_enabled(
                            self.selected_annotation.is_some(),
                            Button::new("Delete Selected Annotation (Del)"),
                        )
                        .clicked()
                    {
                        if let Some(sel) = self.selected_annotation.take() {
                            if let Some(tab) = self.active_tab_mut() {
                                let _ = delete_annotation(&mut tab.doc, sel.page_index, sel.id);
                                tab.is_modified = true;
                                tab.invalidate_render();
                                self.status_message = "Deleted annotation".into();
                            }
                        }
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui
                        .add_enabled(has_doc, Button::new("Find in Document (Ctrl+F)"))
                        .clicked()
                    {
                        self.search_bar_open = true;
                        self.search_replace_mode = false;
                        ui.close_menu();
                    }
                    if ui
                        .add_enabled(has_doc, Button::new("Find & Replace... (Ctrl+H)"))
                        .clicked()
                    {
                        self.search_bar_open = true;
                        self.search_replace_mode = true;
                        ui.close_menu();
                    }
                    if ui
                        .add_enabled(
                            has_doc,
                            Button::new("In-Place Text Editor..."),
                        )
                        .clicked()
                    {
                        self.open_text_editor_dialog();
                        ui.close_menu();
                    }
                    if ui
                        .add_enabled(has_doc, Button::new("Insert Image File..."))
                        .clicked()
                    {
                        self.insert_image_dialog();
                        ui.close_menu();
                    }
                    if ui
                        .add_enabled(
                            has_doc,
                            Button::new("🖼️ Edit & Transform Images..."),
                        )
                        .clicked()
                    {
                        self.open_image_editor_dialog();
                        ui.close_menu();
                    }
                    if ui
                        .add_enabled(
                            has_doc,
                            Button::new("🏷️ Rubber Stamps & Sign Here..."),
                        )
                        .clicked()
                    {
                        self.open_stamp_dialog();
                        ui.close_menu();
                    }
                    if ui
                        .add_enabled(has_doc, Button::new("📐 Drawing & Shape Tools..."))
                        .clicked()
                    {
                        self.show_drawing_dialog = true;
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui
                        .add_enabled(has_doc, Button::new("Insert Blank Page"))
                        .clicked()
                    {
                        if let Some(tab) = self.active_tab_mut() {
                            let curr = tab.current_page;
                            let _ = insert_blank_page(
                                &mut tab.doc,
                                curr + 1,
                                paper_sizes::LETTER.0,
                                paper_sizes::LETTER.1,
                            );
                            tab.current_page += 1;
                            tab.page_input_buffer = (tab.current_page + 1).to_string();
                            tab.is_modified = true;
                            tab.invalidate_render();
                            self.status_message = "Inserted new page".into();
                        }
                        ui.close_menu();
                    }
                    if ui
                        .add_enabled(has_doc, Button::new("Duplicate Current Page"))
                        .clicked()
                    {
                        if let Some(tab) = self.active_tab_mut() {
                            let curr = tab.current_page;
                            if let Ok(_new_id) = duplicate_page(&mut tab.doc, curr) {
                                tab.current_page += 1;
                                tab.page_input_buffer = (tab.current_page + 1).to_string();
                                tab.is_modified = true;
                                tab.invalidate_render();
                                self.status_message = format!("Duplicated page {}", curr + 1);
                            }
                        }
                        ui.close_menu();
                    }
                    if ui
                        .add_enabled(has_doc, Button::new("Page Boundary Boxes / Crop..."))
                        .clicked()
                    {
                        let box_data = self.active_tab().and_then(|tab| {
                            let curr = tab.current_page;
                            let boxes = get_page_boxes(&tab.doc, curr).ok()?;
                            Some((curr, boxes))
                        });

                        if let Some((curr, boxes)) = box_data {
                            self.crop_target_page = curr;
                            let rect = match self.crop_box_type {
                                PageBoxType::CropBox => boxes.crop_box,
                                PageBoxType::MediaBox => boxes.media_box,
                                PageBoxType::BleedBox => boxes.bleed_box,
                                PageBoxType::TrimBox => boxes.trim_box,
                                PageBoxType::ArtBox => boxes.art_box,
                            };
                            self.crop_left = rect[0];
                            self.crop_bottom = rect[1];
                            self.crop_right = rect[2];
                            self.crop_top = rect[3];
                        }
                        self.show_crop_dialog = true;
                        ui.close_menu();
                    }
                    if ui
                        .add_enabled(has_doc, Button::new("Delete Current Page"))
                        .clicked()
                    {
                        if let Some(tab) = self.active_tab_mut() {
                            if tab.doc.page_count() > 1 {
                                let _ = delete_page(&mut tab.doc, tab.current_page);
                                if tab.current_page >= tab.doc.page_count() {
                                    tab.current_page = tab.doc.page_count().saturating_sub(1);
                                }
                                tab.page_input_buffer = (tab.current_page + 1).to_string();
                                tab.is_modified = true;
                                tab.invalidate_render();
                                self.status_message = "Deleted current page".into();
                            }
                        }
                        ui.close_menu();
                    }
                    if ui
                        .add_enabled(has_doc, Button::new("Page Numbering & Labels..."))
                        .clicked()
                    {
                        if let Some(tab) = self.active_tab() {
                            self.page_labels_start_page = tab.current_page + 1;
                        }
                        self.show_page_labels_dialog = true;
                        ui.close_menu();
                    }
                    if ui
                        .add_enabled(has_doc, Button::new("Add Hyperlink..."))
                        .clicked()
                    {
                        if let Some(tab) = self.active_tab() {
                            self.link_target_page =
                                (tab.current_page + 2).min(tab.doc.page_count());
                        }
                        self.show_link_dialog = true;
                        ui.close_menu();
                    }
                    if ui
                        .add_enabled(has_doc, Button::new("Add Form Field..."))
                        .clicked()
                    {
                        self.show_add_form_field_dialog = true;
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui
                        .add_enabled(has_doc, Button::new("Split Document..."))
                        .clicked()
                    {
                        self.show_split_dialog = true;
                        ui.close_menu();
                    }
                });

                ui.menu_button("View", |ui| {
                    if ui
                        .add_enabled(has_doc, Button::new("Fit to Page (Ctrl+0)"))
                        .clicked()
                    {
                        let screen = ctx.screen_rect();
                        let avail_w =
                            screen.width() - (if self.sidebar_open { 260.0 } else { 60.0 });
                        let avail_h = screen.height() - 140.0;
                        if let Some(tab) = self.active_tab_mut() {
                            tab.fit_to_page(avail_w, avail_h);
                        }
                        ui.close_menu();
                    }
                    if ui
                        .add_enabled(has_doc, Button::new("Fit to Width (Ctrl+9)"))
                        .clicked()
                    {
                        let screen = ctx.screen_rect();
                        let avail_w =
                            screen.width() - (if self.sidebar_open { 240.0 } else { 40.0 });
                        if let Some(tab) = self.active_tab_mut() {
                            tab.fit_to_width(avail_w);
                        }
                        ui.close_menu();
                    }
                    if ui
                        .add_enabled(has_doc, Button::new("Actual Size (100%)"))
                        .clicked()
                    {
                        if let Some(tab) = self.active_tab_mut() {
                            tab.zoom = 1.0;
                            tab.invalidate_render();
                        }
                        ui.close_menu();
                    }
                    ui.separator();
                    let is_single = self.page_view_mode == PageViewMode::SinglePage;
                    let is_cont = self.page_view_mode == PageViewMode::ContinuousScroll;
                    let is_facing = self.page_view_mode == PageViewMode::TwoPageFacing;
                    let is_two_cont = self.page_view_mode == PageViewMode::TwoPageContinuous;
                    let is_org = self.page_view_mode == PageViewMode::OrganizePages;
                    if ui
                        .add_enabled(
                            has_doc,
                            egui::SelectableLabel::new(
                                is_single,
                                "📄 Single Page View",
                            ),
                        )
                        .clicked()
                    {
                        self.page_view_mode = PageViewMode::SinglePage;
                        ui.close_menu();
                    }
                    if ui
                        .add_enabled(
                            has_doc,
                            egui::SelectableLabel::new(
                                is_cont,
                                "📜 Continuous Scroll View",
                            ),
                        )
                        .clicked()
                    {
                        self.page_view_mode = PageViewMode::ContinuousScroll;
                        ui.close_menu();
                    }
                    if ui
                        .add_enabled(
                            has_doc,
                            egui::SelectableLabel::new(
                                is_facing,
                                "📖 Two-Page Facing View",
                            ),
                        )
                        .clicked()
                    {
                        self.page_view_mode = PageViewMode::TwoPageFacing;
                        ui.close_menu();
                    }
                    if ui
                        .add_enabled(
                            has_doc,
                            egui::SelectableLabel::new(
                                is_two_cont,
                                "📚 Two-Page Continuous View",
                            ),
                        )
                        .clicked()
                    {
                        self.page_view_mode = PageViewMode::TwoPageContinuous;
                        ui.close_menu();
                    }
                    if ui
                        .add_enabled(
                            has_doc,
                            egui::SelectableLabel::new(is_org, "🗂 Organize Pages Grid"),
                        )
                        .clicked()
                    {
                        self.page_view_mode = PageViewMode::OrganizePages;
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui
                        .checkbox(
                            &mut self.two_page_cover_page,
                            "Show First Page as Cover in Facing View",
                        )
                        .clicked()
                    {
                        ui.close_menu();
                    }
                    ui.separator();
                    let theme_label = if self.dark_theme {
                        "☀ Switch to Clean Light Theme"
                    } else {
                        "🌙 Switch to Modern Dark Theme"
                    };
                    if ui.button(theme_label).clicked() {
                        self.dark_theme = !self.dark_theme;
                        setup_custom_theme(ctx, self.dark_theme);
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui
                        .add_enabled(has_doc, Button::new("Rotate Clockwise 90°"))
                        .clicked()
                    {
                        if let Some(tab) = self.active_tab_mut() {
                            let curr = tab.current_page;
                            let _ = rotate_page(&mut tab.doc, curr, 90);
                            tab.is_modified = true;
                            tab.invalidate_render();
                        }
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui
                        .add_enabled(
                            has_doc,
                            egui::SelectableLabel::new(
                                self.is_full_screen,
                                "⛶ Full Screen Presentation (F11 / Ctrl+L)",
                            ),
                        )
                        .clicked()
                    {
                        self.is_full_screen = !self.is_full_screen;
                        ui.close_menu();
                    }
                    if ui
                        .add_enabled(
                            has_doc,
                            egui::SelectableLabel::new(
                                self.split_view_enabled,
                                "◫ Split View Mode (Ctrl+Shift+T)",
                            ),
                        )
                        .clicked()
                    {
                        self.split_view_enabled = !self.split_view_enabled;
                        if self.split_view_enabled {
                            let total = self.active_tab().map(|t| t.doc.page_count()).unwrap_or(0);
                            self.split_view_second_page = self
                                .active_tab()
                                .map(|t| (t.current_page + 1).min(total.saturating_sub(1)))
                                .unwrap_or(0);
                        }
                        ui.close_menu();
                    }
                    if ui
                        .add_enabled(
                            has_doc,
                            egui::SelectableLabel::new(
                                self.active_tool == ActiveTool::Loupe,
                                "🔎 Magnifying Glass / Loupe",
                            ),
                        )
                        .clicked()
                    {
                        self.active_tool = ActiveTool::Loupe;
                        ui.close_menu();
                    }
                    ui.separator();
                    let can_back = self.nav_history_idx > 0 && !self.nav_history.is_empty();
                    let can_fwd = self.nav_history_idx + 1 < self.nav_history.len();
                    if ui
                        .add_enabled(
                            can_back,
                            Button::new("⮌ Previous View (Alt+Left)"),
                        )
                        .clicked()
                    {
                        self.navigate_back();
                        ui.close_menu();
                    }
                    if ui
                        .add_enabled(can_fwd, Button::new("⮎ Next View (Alt+Right)"))
                        .clicked()
                    {
                        self.navigate_forward();
                        ui.close_menu();
                    }
                });

                ui.menu_button("Tools", |ui| {
                    if ui
                        .add_enabled(
                            has_doc,
                            Button::new("Auto-Generate Bookmarks from Headings"),
                        )
                        .clicked()
                    {
                        if let Some(tab) = self.active_tab_mut() {
                            match ravpdf_core::outline::auto_generate_bookmarks_from_headings(
                                &mut tab.doc,
                            ) {
                                Ok(count) => {
                                    tab.reload_bookmarks();
                                    tab.is_modified = true;
                                    self.status_message = format!(
                                        "Auto-generated {} bookmark(s) from headings",
                                        count
                                    );
                                }
                                Err(e) => {
                                    self.status_message =
                                        format!("Auto-generating bookmarks failed: {}", e);
                                }
                            }
                        }
                        ui.close_menu();
                    }
                    if ui
                        .add_enabled(
                            has_doc,
                            Button::new("Export Annotations to XFDF..."),
                        )
                        .clicked()
                    {
                        if let Some(tab) = self.active_tab() {
                            if let Ok(xml) = ravpdf_annot::export_annotations_to_xfdf(&tab.doc) {
                                self.xfdf_buffer = xml;
                                self.show_xfdf_dialog = true;
                            }
                        }
                        ui.close_menu();
                    }
                    if ui
                        .add_enabled(
                            has_doc,
                            Button::new("Import Annotations from XFDF..."),
                        )
                        .clicked()
                    {
                        self.show_xfdf_dialog = true;
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui
                        .add_enabled(has_doc, Button::new("Add Watermark..."))
                        .clicked()
                    {
                        self.show_watermark_dialog = true;
                        ui.close_menu();
                    }
                    if ui
                        .add_enabled(has_doc, Button::new("Bates Numbering..."))
                        .clicked()
                    {
                        self.show_bates_dialog = true;
                        ui.close_menu();
                    }
                    if ui
                        .add_enabled(has_doc, Button::new("Headers & Footers..."))
                        .clicked()
                    {
                        self.show_header_footer_dialog = true;
                        ui.close_menu();
                    }
                    ui.separator();
                    ui.menu_button("Rubber Stamps", |ui| {
                        if ui
                            .add_enabled(has_doc, Button::new("✓ APPROVED (Green)"))
                            .clicked()
                        {
                            self.insert_rubber_stamp("APPROVED", (0.1, 0.7, 0.2));
                            ui.close_menu();
                        }
                        if ui
                            .add_enabled(has_doc, Button::new("🔒 CONFIDENTIAL (Red)"))
                            .clicked()
                        {
                            self.insert_rubber_stamp("CONFIDENTIAL", (0.8, 0.1, 0.1));
                            ui.close_menu();
                        }
                        if ui
                            .add_enabled(has_doc, Button::new("📝 DRAFT (Blue)"))
                            .clicked()
                        {
                            self.insert_rubber_stamp("DRAFT", (0.1, 0.4, 0.8));
                            ui.close_menu();
                        }
                        if ui
                            .add_enabled(has_doc, Button::new("★ FINAL (Purple)"))
                            .clicked()
                        {
                            self.insert_rubber_stamp("FINAL", (0.5, 0.2, 0.7));
                            ui.close_menu();
                        }
                        if ui
                            .add_enabled(has_doc, Button::new("⊘ VOID (Dark Red)"))
                            .clicked()
                        {
                            self.insert_rubber_stamp("VOID", (0.7, 0.0, 0.0));
                            ui.close_menu();
                        }
                    });
                    ui.separator();
                    if ui
                        .add_enabled(has_doc, Button::new("Flatten All Forms"))
                        .clicked()
                    {
                        if let Some(tab) = self.active_tab_mut() {
                            let _ = flatten_form(&mut tab.doc);
                            tab.is_modified = true;
                            tab.invalidate_render();
                            self.status_message =
                                "All form fields flattened to page content".into();
                        }
                        ui.close_menu();
                    }
                    if ui
                        .add_enabled(has_doc, Button::new("Export Form Data..."))
                        .clicked()
                    {
                        self.show_export_form_dialog = true;
                        ui.close_menu();
                    }
                    if ui
                        .add_enabled(has_doc, Button::new("Set Page Tab Order..."))
                        .clicked()
                    {
                        self.show_tab_order_dialog = true;
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui
                        .add_enabled(has_doc, Button::new("🛡 Sanitize Document..."))
                        .clicked()
                    {
                        self.show_sanitize_dialog = true;
                        ui.close_menu();
                    }
                    if ui
                        .add_enabled(has_doc, Button::new("Redact Patterns / Text..."))
                        .clicked()
                    {
                        self.show_redact_pattern_dialog = true;
                        ui.close_menu();
                    }
                    if ui
                        .add_enabled(
                            has_doc,
                            Button::new("Protect & Encrypt Document..."),
                        )
                        .clicked()
                    {
                        self.show_encrypt_dialog = true;
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui
                        .add_enabled(has_doc, Button::new("Audit Space Usage..."))
                        .clicked()
                    {
                        if let Some(tab) = self.active_tab() {
                            self.space_audit = Some(audit_space_usage(&tab.doc));
                            self.show_space_audit_dialog = true;
                        }
                        ui.close_menu();
                    }
                    if ui.button("Compare Two Documents...").clicked() {
                        if let Some(tab) = self.active_tab() {
                            self.compare_file_a = tab.file_path.clone();
                        }
                        self.show_compare_dialog = true;
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui
                        .add_enabled(
                            has_doc,
                            Button::new("Preflight & Standards Validation..."),
                        )
                        .clicked()
                    {
                        self.show_preflight_dialog = true;
                        ui.close_menu();
                    }
                    if ui
                        .add_enabled(
                            has_doc,
                            Button::new("Output Preview & Separations..."),
                        )
                        .clicked()
                    {
                        if let Some(tab) = self.active_tab_mut() {
                            tab.invalidate_render();
                        }
                        self.show_output_preview_dialog = true;
                        ui.close_menu();
                    }
                    if ui
                        .add_enabled(
                            has_doc,
                            Button::new("Color Management & Soft Proofing..."),
                        )
                        .clicked()
                    {
                        self.show_color_management_dialog = true;
                        ui.close_menu();
                    }
                    if ui
                        .add_enabled(
                            has_doc,
                            Button::new("Prepress Tools & Printer Marks..."),
                        )
                        .clicked()
                    {
                        self.show_prepress_dialog = true;
                        ui.close_menu();
                    }
                    if ui
                        .add_enabled(
                            has_doc,
                            Button::new(
                                "Standards Compliance Wizard (PDF/A, PDF/X, PDF/E)...",
                            ),
                        )
                        .clicked()
                    {
                        self.show_standards_dialog = true;
                        ui.close_menu();
                    }
                    if ui
                        .add_enabled(
                            has_doc,
                            Button::new("Digital Signatures & Certificates..."),
                        )
                        .clicked()
                    {
                        self.sidebar_tab = SidebarTab::Signatures;
                        self.sidebar_open = true;
                        ui.close_menu();
                    }
                    if ui
                        .button("Action Wizard (Batch Processing)...")
                        .clicked()
                    {
                        self.show_batch_dialog = true;
                        ui.close_menu();
                    }
                    if ui
                        .add_enabled(has_doc, Button::new("Embed Rich Media / Video..."))
                        .clicked()
                    {
                        self.show_rich_media_dialog = true;
                        ui.close_menu();
                    }
                    if ui
                        .add_enabled(has_doc, Button::new("Embed 3D Model..."))
                        .clicked()
                    {
                        self.show_threed_dialog = true;
                        ui.close_menu();
                    }
                    if ui
                        .add_enabled(has_doc, Button::new("Recognize Text (OCR)..."))
                        .clicked()
                    {
                        self.show_ocr_dialog = true;
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui
                        .add_enabled(has_doc, Button::new("Accessibility Checker..."))
                        .clicked()
                    {
                        if let Some(tab) = self.active_tab() {
                            let rep = audit_accessibility(&tab.doc).ok();
                            let title = tab
                                .metadata
                                .as_ref()
                                .and_then(|m| m.title.clone())
                                .unwrap_or_else(|| tab.title.clone());
                            let lang = tab
                                .doc
                                .catalog()
                                .and_then(|c| c.get_str("Lang"))
                                .unwrap_or_else(|| "en-US".to_string());
                            self.accessibility_report = rep;
                            self.accessibility_fix_title_input = title;
                            self.accessibility_fix_lang_input = lang;
                            self.show_accessibility_dialog = true;
                        }
                        ui.close_menu();
                    }
                    if ui
                        .add_enabled(
                            has_doc,
                            Button::new("Review & Comments Summary..."),
                        )
                        .clicked()
                    {
                        if let Some(tab) = self.active_tab() {
                            self.comments_list =
                                get_document_comments(&tab.doc).unwrap_or_default();
                            self.show_comments_summary_dialog = true;
                        }
                        ui.close_menu();
                    }
                    if ui
                        .add_enabled(
                            has_doc,
                            Button::new("Merge Comments from Reviewer..."),
                        )
                        .clicked()
                    {
                        self.show_merge_comments_dialog = true;
                        ui.close_menu();
                    }
                    if ui
                        .add_enabled(
                            has_doc,
                            Button::new("Measurement Scale & Calibration..."),
                        )
                        .clicked()
                    {
                        self.show_measure_scale_dialog = true;
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui
                        .add_enabled(
                            has_doc,
                            Button::new("JavaScript Console & Debugger... (Ctrl+J)"),
                        )
                        .clicked()
                    {
                        self.show_js_console_dialog = true;
                        ui.close_menu();
                    }
                    if ui
                        .button("Plugins & Extensions Manager...")
                        .clicked()
                    {
                        self.show_plugins_dialog = true;
                        ui.close_menu();
                    }
                });

                ui.menu_button("Help", |ui| {
                    if ui.button("About RAV PDF").clicked() {
                        self.status_message = "RAV PDF 1.0 (ISO 32000 Conformance)".into();
                        ui.close_menu();
                    }
                });
            });
        });
    }

    /// Renders primary toolbar (EI-01.2).
    fn render_toolbar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("toolbar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui.button("📂 Open").clicked() {
                    self.open_file_dialog();
                }

                let has_doc = self.active_tab().is_some();

                if ui.add_enabled(has_doc, Button::new("💾 Save")).clicked() {
                    self.save_file_dialog();
                }

                ui.separator();

                if ui
                    .button(if self.sidebar_open {
                        "◀ Sidebar"
                    } else {
                        "▶ Sidebar"
                    })
                    .clicked()
                {
                    self.sidebar_open = !self.sidebar_open;
                }

                ui.separator();

                let total_pages = self.active_tab().map(|t| t.doc.page_count()).unwrap_or(0);
                let current_page = self.active_tab().map(|t| t.current_page).unwrap_or(0);
                let can_prev = has_doc && current_page > 0;
                let can_next = has_doc && current_page + 1 < total_pages;
                let nav_step = if self.page_view_mode == PageViewMode::TwoPageFacing {
                    2
                } else {
                    1
                };

                if ui.add_enabled(can_prev, Button::new("⏮")).clicked() {
                    self.record_page_navigation(0);
                }
                if ui.add_enabled(can_prev, Button::new("◀")).clicked() {
                    self.record_page_navigation(current_page.saturating_sub(nav_step));
                }

                // Go to page quick input box (FR-01.2.6) with Page Label support (FR-06.9)
                if has_doc {
                    let page_label_opt = self
                        .active_tab()
                        .map(|t| get_page_label(&t.doc, current_page));
                    ui.label("Page");
                    let mut jump_target = None;
                    if let Some(tab) = self.active_tab_mut() {
                        let text_edit = ui.add(
                            egui::TextEdit::singleline(&mut tab.page_input_buffer)
                                .desired_width(32.0),
                        );
                        if text_edit.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                            if let Ok(p) = tab.page_input_buffer.trim().parse::<usize>() {
                                if p >= 1 && p <= total_pages {
                                    jump_target = Some(p - 1);
                                } else {
                                    tab.page_input_buffer = (tab.current_page + 1).to_string();
                                }
                            }
                        }
                    }
                    if let Some(target) = jump_target {
                        self.record_page_navigation(target);
                    }
                    if let Some(lbl) = page_label_opt {
                        if lbl != (current_page + 1).to_string() {
                            ui.label(format!("(\"{}\") of {}", lbl, total_pages));
                        } else {
                            ui.label(format!("of {}", total_pages));
                        }
                    } else {
                        ui.label(format!("of {}", total_pages));
                    }
                } else {
                    ui.label("Page 0 of 0");
                }

                if ui.add_enabled(can_next, Button::new("▶")).clicked() {
                    self.record_page_navigation(
                        (current_page + nav_step).min(total_pages.saturating_sub(1)),
                    );
                }
                if ui.add_enabled(can_next, Button::new("⏭")).clicked() {
                    self.record_page_navigation(total_pages.saturating_sub(1));
                }

                ui.separator();

                // Navigation History Buttons (FR-01.2.7)
                let can_back = self.nav_history_idx > 0 && !self.nav_history.is_empty();
                let can_fwd = self.nav_history_idx + 1 < self.nav_history.len();
                if ui
                    .add_enabled(can_back, Button::new("⮌"))
                    .on_hover_text("Previous View (Alt+Left)")
                    .clicked()
                {
                    self.navigate_back();
                }
                if ui
                    .add_enabled(can_fwd, Button::new("⮎"))
                    .on_hover_text("Next View (Alt+Right)")
                    .clicked()
                {
                    self.navigate_forward();
                }

                ui.separator();

                // Zoom controls & fit modes (FR-01.3.2)
                let zoom_pct = self.active_tab().map(|t| t.zoom * 100.0).unwrap_or(100.0);
                if ui.add_enabled(has_doc, Button::new("➖")).clicked() {
                    if let Some(tab) = self.active_tab_mut() {
                        tab.target_zoom = (tab.target_zoom / 1.2).max(0.1);
                        ctx.request_repaint();
                    }
                }
                ui.label(format!("{:.0}%", zoom_pct));
                if ui.add_enabled(has_doc, Button::new("➕")).clicked() {
                    if let Some(tab) = self.active_tab_mut() {
                        tab.target_zoom = (tab.target_zoom * 1.2).min(64.0);
                        ctx.request_repaint();
                    }
                }
                if ui.add_enabled(has_doc, Button::new("Fit Page")).clicked() {
                    let screen = ctx.screen_rect();
                    let avail_w = screen.width() - (if self.sidebar_open { 260.0 } else { 60.0 });
                    let avail_h = screen.height() - 140.0;
                    if let Some(tab) = self.active_tab_mut() {
                        tab.fit_to_page(avail_w, avail_h);
                    }
                }
                if ui.add_enabled(has_doc, Button::new("Fit Width")).clicked() {
                    let screen = ctx.screen_rect();
                    let avail_w = screen.width() - (if self.sidebar_open { 240.0 } else { 40.0 });
                    if let Some(tab) = self.active_tab_mut() {
                        tab.fit_to_width(avail_w);
                    }
                }

                ui.separator();

                if ui.add_enabled(has_doc, Button::new("⟳ Rotate")).clicked() {
                    if let Some(tab) = self.active_tab_mut() {
                        let curr = tab.current_page;
                        let _ = rotate_page(&mut tab.doc, curr, 90);
                        tab.is_modified = true;
                        tab.invalidate_render();
                    }
                }

                ui.separator();

                // View Mode selector (Single Page, Continuous, Two-Page Facing, Two-Page Continuous, Organize)
                let is_single = self.page_view_mode == PageViewMode::SinglePage;
                let is_cont = self.page_view_mode == PageViewMode::ContinuousScroll;
                let is_facing = self.page_view_mode == PageViewMode::TwoPageFacing;
                let is_two_cont = self.page_view_mode == PageViewMode::TwoPageContinuous;
                let is_org = self.page_view_mode == PageViewMode::OrganizePages;
                if ui
                    .add_enabled(has_doc, egui::SelectableLabel::new(is_single, "📄 Single"))
                    .on_hover_text("Single Page View")
                    .clicked()
                {
                    self.page_view_mode = PageViewMode::SinglePage;
                }
                if ui
                    .add_enabled(
                        has_doc,
                        egui::SelectableLabel::new(is_cont, "📜 Continuous"),
                    )
                    .on_hover_text("Continuous Scroll View")
                    .clicked()
                {
                    self.page_view_mode = PageViewMode::ContinuousScroll;
                }
                if ui
                    .add_enabled(
                        has_doc,
                        egui::SelectableLabel::new(is_facing, "📖 Facing"),
                    )
                    .on_hover_text("Two-Page Facing View")
                    .clicked()
                {
                    self.page_view_mode = PageViewMode::TwoPageFacing;
                }
                if ui
                    .add_enabled(
                        has_doc,
                        egui::SelectableLabel::new(is_two_cont, "📚 2-Page Scroll"),
                    )
                    .on_hover_text("Two-Page Continuous Scroll View")
                    .clicked()
                {
                    self.page_view_mode = PageViewMode::TwoPageContinuous;
                }
                if ui
                    .add_enabled(has_doc, egui::SelectableLabel::new(is_org, "🗂 Organize"))
                    .on_hover_text("Organize Pages Grid View")
                    .clicked()
                {
                    self.page_view_mode = PageViewMode::OrganizePages;
                }

                ui.separator();

                // Find & Replace in document triggers (FR-34.1, FR-04.8)
                if ui.add_enabled(has_doc, Button::new("🔍 Find")).clicked() {
                    self.search_bar_open = !self.search_bar_open;
                    self.search_replace_mode = false;
                }
                if ui
                    .add_enabled(has_doc, Button::new("🔍✏ Replace"))
                    .clicked()
                {
                    self.search_bar_open = true;
                    self.search_replace_mode = true;
                }
                if ui
                    .add_enabled(has_doc, Button::new("🖼 Image"))
                    .on_hover_text("Insert image onto page")
                    .clicked()
                {
                    self.insert_image_dialog();
                }

                ui.separator();

                // Tool Selector Buttons
                ui.add_enabled_ui(has_doc, |ui| {
                    ui.selectable_value(&mut self.active_tool, ActiveTool::Select, "👆 Select");
                    ui.selectable_value(&mut self.active_tool, ActiveTool::Hand, "✋ Pan");
                    ui.separator();

                    // Markup Tools
                    let markup_label = match self.active_tool {
                        ActiveTool::StickyNote => "💬 Note",
                        ActiveTool::Highlight => "🖍 Highlight",
                        ActiveTool::Underline => "U̲ Underline",
                        ActiveTool::Squiggly => "〰 Squiggly",
                        ActiveTool::StrikeOut => "S̶ Strike",
                        _ => "💬 Markup",
                    };
                    ui.menu_button(markup_label, |ui| {
                        if ui
                            .selectable_label(
                                self.active_tool == ActiveTool::StickyNote,
                                "💬 Sticky Note",
                            )
                            .clicked()
                        {
                            self.active_tool = ActiveTool::StickyNote;
                            ui.close_menu();
                        }
                        if ui
                            .selectable_label(
                                self.active_tool == ActiveTool::Highlight,
                                "🖍 Text Highlight",
                            )
                            .clicked()
                        {
                            self.active_tool = ActiveTool::Highlight;
                            ui.close_menu();
                        }
                        if ui
                            .selectable_label(
                                self.active_tool == ActiveTool::Underline,
                                "U̲ Text Underline",
                            )
                            .clicked()
                        {
                            self.active_tool = ActiveTool::Underline;
                            ui.close_menu();
                        }
                        if ui
                            .selectable_label(
                                self.active_tool == ActiveTool::Squiggly,
                                "〰 Squiggly Underline",
                            )
                            .clicked()
                        {
                            self.active_tool = ActiveTool::Squiggly;
                            ui.close_menu();
                        }
                        if ui
                            .selectable_label(
                                self.active_tool == ActiveTool::StrikeOut,
                                "S̶ Strikethrough",
                            )
                            .clicked()
                        {
                            self.active_tool = ActiveTool::StrikeOut;
                            ui.close_menu();
                        }
                    });

                    // Drawing Tools
                    let draw_label = match self.active_tool {
                        ActiveTool::Rectangle => "▭ Rect",
                        ActiveTool::Circle => "◯ Circle",
                        ActiveTool::Line => "➔ Line",
                        ActiveTool::Polygon => "⬡ Polygon",
                        ActiveTool::Polyline => "⚡ Polyline",
                        ActiveTool::Pencil => "✏ Pencil",
                        ActiveTool::Eraser => "🧹 Eraser",
                        _ => "📐 Shapes",
                    };
                    ui.menu_button(draw_label, |ui| {
                        if ui
                            .selectable_label(
                                self.active_tool == ActiveTool::Rectangle,
                                "▭ Rectangle",
                            )
                            .clicked()
                        {
                            self.active_tool = ActiveTool::Rectangle;
                            ui.close_menu();
                        }
                        if ui
                            .selectable_label(
                                self.active_tool == ActiveTool::Circle,
                                "◯ Circle / Oval",
                            )
                            .clicked()
                        {
                            self.active_tool = ActiveTool::Circle;
                            ui.close_menu();
                        }
                        if ui
                            .selectable_label(
                                self.active_tool == ActiveTool::Line,
                                "➔ Arrow / Line",
                            )
                            .clicked()
                        {
                            self.active_tool = ActiveTool::Line;
                            ui.close_menu();
                        }
                        if ui
                            .selectable_label(
                                self.active_tool == ActiveTool::Polygon,
                                "⬡ Polygon",
                            )
                            .clicked()
                        {
                            self.active_tool = ActiveTool::Polygon;
                            self.polygon_vertices.clear();
                            ui.close_menu();
                        }
                        if ui
                            .selectable_label(
                                self.active_tool == ActiveTool::Polyline,
                                "⚡ Polyline",
                            )
                            .clicked()
                        {
                            self.active_tool = ActiveTool::Polyline;
                            self.polygon_vertices.clear();
                            ui.close_menu();
                        }
                        if ui
                            .selectable_label(
                                self.active_tool == ActiveTool::Pencil,
                                "✏ Freehand Pencil",
                            )
                            .clicked()
                        {
                            self.active_tool = ActiveTool::Pencil;
                            ui.close_menu();
                        }
                        if ui
                            .selectable_label(
                                self.active_tool == ActiveTool::Eraser,
                                "🧹 Stroke Eraser",
                            )
                            .clicked()
                        {
                            self.active_tool = ActiveTool::Eraser;
                            ui.close_menu();
                        }
                    });

                    // Text & Callout Tools
                    let text_label = match self.active_tool {
                        ActiveTool::Text => "T Text Box",
                        ActiveTool::Typewriter => "⌨ Typewriter",
                        ActiveTool::Callout => "🗨 Callout",
                        _ => "T Text",
                    };
                    ui.menu_button(text_label, |ui| {
                        if ui
                            .selectable_label(self.active_tool == ActiveTool::Text, "T Text Box")
                            .clicked()
                        {
                            self.active_tool = ActiveTool::Text;
                            ui.close_menu();
                        }
                        if ui
                            .selectable_label(
                                self.active_tool == ActiveTool::Typewriter,
                                "⌨ Typewriter",
                            )
                            .clicked()
                        {
                            self.active_tool = ActiveTool::Typewriter;
                            ui.close_menu();
                        }
                        if ui
                            .selectable_label(self.active_tool == ActiveTool::Callout, "🗨 Callout")
                            .clicked()
                        {
                            self.active_tool = ActiveTool::Callout;
                            ui.close_menu();
                        }
                    });

                    // Forms Tools
                    ui.menu_button("📝 Forms", |ui| {
                        if ui
                            .selectable_label(
                                self.active_tool == ActiveTool::FormField(FormFieldType::Text),
                                "📝 Text Field",
                            )
                            .clicked()
                        {
                            self.active_tool = ActiveTool::FormField(FormFieldType::Text);
                            ui.close_menu();
                        }
                        if ui
                            .selectable_label(
                                self.active_tool == ActiveTool::FormField(FormFieldType::Checkbox),
                                "☑ Checkbox",
                            )
                            .clicked()
                        {
                            self.active_tool = ActiveTool::FormField(FormFieldType::Checkbox);
                            ui.close_menu();
                        }
                        if ui
                            .selectable_label(
                                self.active_tool == ActiveTool::FormField(FormFieldType::Radio),
                                "🔘 Radio Button",
                            )
                            .clicked()
                        {
                            self.active_tool = ActiveTool::FormField(FormFieldType::Radio);
                            ui.close_menu();
                        }
                        if ui
                            .selectable_label(
                                self.active_tool == ActiveTool::FormField(FormFieldType::Choice),
                                "▾ Dropdown List",
                            )
                            .clicked()
                        {
                            self.active_tool = ActiveTool::FormField(FormFieldType::Choice);
                            ui.close_menu();
                        }
                        if ui
                            .selectable_label(
                                self.active_tool
                                    == ActiveTool::FormField(FormFieldType::PushButton),
                                "🔘 Push Button",
                            )
                            .clicked()
                        {
                            self.active_tool = ActiveTool::FormField(FormFieldType::PushButton);
                            ui.close_menu();
                        }
                        if ui
                            .selectable_label(
                                self.active_tool == ActiveTool::FormField(FormFieldType::Signature),
                                "🔏 Signature Field",
                            )
                            .clicked()
                        {
                            self.active_tool = ActiveTool::FormField(FormFieldType::Signature);
                            ui.close_menu();
                        }
                    });

                    // More Tools Dropdown
                    let more_label = match self.active_tool {
                        ActiveTool::Link => "🔗 Link",
                        ActiveTool::Redact => "█ Redact",
                        ActiveTool::MeasureDistance => "📏 Distance",
                        ActiveTool::MeasureArea => "📐 Area",
                        ActiveTool::Loupe => "🔎 Loupe",
                        ActiveTool::MarqueeZoom => "🔍 Marquee",
                        _ => "🛠 Tools",
                    };
                    ui.menu_button(more_label, |ui| {
                        if ui
                            .selectable_label(
                                self.active_tool == ActiveTool::Link,
                                "🔗 Add Hyperlink",
                            )
                            .clicked()
                        {
                            self.active_tool = ActiveTool::Link;
                            ui.close_menu();
                        }
                        if ui
                            .selectable_label(
                                self.active_tool == ActiveTool::Redact,
                                "█ Mark Redaction",
                            )
                            .clicked()
                        {
                            self.active_tool = ActiveTool::Redact;
                            ui.close_menu();
                        }
                        if ui
                            .selectable_label(
                                self.active_tool == ActiveTool::MeasureDistance,
                                "📏 Distance",
                            )
                            .clicked()
                        {
                            self.active_tool = ActiveTool::MeasureDistance;
                            ui.close_menu();
                        }
                        if ui
                            .selectable_label(
                                self.active_tool == ActiveTool::MeasureArea,
                                "📐 Area",
                            )
                            .clicked()
                        {
                            self.active_tool = ActiveTool::MeasureArea;
                            ui.close_menu();
                        }
                        if ui
                            .selectable_label(
                                self.active_tool == ActiveTool::Loupe,
                                "🔎 Loupe Tool",
                            )
                            .clicked()
                        {
                            self.active_tool = ActiveTool::Loupe;
                            ui.close_menu();
                        }
                        if ui
                            .selectable_label(
                                self.active_tool == ActiveTool::MarqueeZoom,
                                "🔍 Marquee Zoom",
                            )
                            .clicked()
                        {
                            self.active_tool = ActiveTool::MarqueeZoom;
                            ui.close_menu();
                        }
                        ui.separator();
                        if ui
                            .selectable_label(self.split_view_enabled, "◫ Split View")
                            .clicked()
                        {
                            self.split_view_enabled = !self.split_view_enabled;
                            if self.split_view_enabled {
                                let total =
                                    self.active_tab().map(|t| t.doc.page_count()).unwrap_or(0);
                                let curr = self.active_tab().map(|t| t.current_page).unwrap_or(0);
                                self.split_view_second_page =
                                    (curr + 1).min(total.saturating_sub(1));
                            }
                            ui.close_menu();
                        }
                        if ui.button("⛶ Fullscreen Presentation").clicked() {
                            self.is_full_screen = true;
                            ui.close_menu();
                        }
                    });

                    // Interactive Form Field authoring tools (FR-11.1)
                    ui.menu_button("📝 Forms", |ui| {
                        if ui
                            .selectable_label(
                                self.active_tool == ActiveTool::FormField(FormFieldType::Text),
                                "📝 Text Field",
                            )
                            .clicked()
                        {
                            self.active_tool = ActiveTool::FormField(FormFieldType::Text);
                            ui.close_menu();
                        }
                        if ui
                            .selectable_label(
                                self.active_tool == ActiveTool::FormField(FormFieldType::Checkbox),
                                "☑ Checkbox",
                            )
                            .clicked()
                        {
                            self.active_tool = ActiveTool::FormField(FormFieldType::Checkbox);
                            ui.close_menu();
                        }
                        if ui
                            .selectable_label(
                                self.active_tool == ActiveTool::FormField(FormFieldType::Radio),
                                "🔘 Radio Button",
                            )
                            .clicked()
                        {
                            self.active_tool = ActiveTool::FormField(FormFieldType::Radio);
                            ui.close_menu();
                        }
                        if ui
                            .selectable_label(
                                self.active_tool == ActiveTool::FormField(FormFieldType::Choice),
                                "▾ Dropdown List",
                            )
                            .clicked()
                        {
                            self.active_tool = ActiveTool::FormField(FormFieldType::Choice);
                            ui.close_menu();
                        }
                        if ui
                            .selectable_label(
                                self.active_tool
                                    == ActiveTool::FormField(FormFieldType::PushButton),
                                "🔘 Push Button",
                            )
                            .clicked()
                        {
                            self.active_tool = ActiveTool::FormField(FormFieldType::PushButton);
                            ui.close_menu();
                        }
                        if ui
                            .selectable_label(
                                self.active_tool == ActiveTool::FormField(FormFieldType::Signature),
                                "🔏 Signature Field",
                            )
                            .clicked()
                        {
                            self.active_tool = ActiveTool::FormField(FormFieldType::Signature);
                            ui.close_menu();
                        }
                    });
                    ui.checkbox(&mut self.form_test_mode, "👁 Test Mode")
                        .on_hover_text("Toggle Form Test / Fill Preview Mode");

                    if self.active_tool == ActiveTool::MeasureDistance
                        || self.active_tool == ActiveTool::MeasureArea
                    {
                        ui.separator();
                        if ui
                            .small_button("⚙ Scale")
                            .on_hover_text("Configure scale ratio and measurement units")
                            .clicked()
                        {
                            self.show_measure_scale_dialog = true;
                        }
                        ui.checkbox(&mut self.measure_bake_as_annot, "Annotate")
                            .on_hover_text(
                                "Bake measurements into document as annotations",
                            );
                    }

                    if has_doc && self.selected_annotation.is_some() {
                        ui.separator();
                        if ui
                            .button(
                                RichText::new("🗑 Del Annot").color(Color32::from_rgb(239, 68, 68)),
                            )
                            .on_hover_text("Delete selected annotation (Del)")
                            .clicked()
                        {
                            if let Some(sel) = self.selected_annotation.take() {
                                if let Some(tab) = self.active_tab_mut() {
                                    let _ = delete_annotation(&mut tab.doc, sel.page_index, sel.id);
                                    tab.is_modified = true;
                                    tab.invalidate_render();
                                    self.status_message = "Deleted annotation".into();
                                }
                            }
                        }
                        if ui
                            .button("⚙ Annot Props")
                            .on_hover_text("Open annotation properties inspector")
                            .clicked()
                        {
                            self.show_annot_inspector = true;
                        }
                    }

                    if has_doc && self.selected_form_field.is_some() {
                        ui.separator();
                        if ui
                            .button(
                                RichText::new("🗑 Del Field").color(Color32::from_rgb(239, 68, 68)),
                            )
                            .on_hover_text("Delete selected form field (Del)")
                            .clicked()
                        {
                            if let Some(sel) = self.selected_form_field.take() {
                                if let Some(tab) = self.active_tab_mut() {
                                    let _ = delete_form_field(&mut tab.doc, sel.id);
                                    tab.is_modified = true;
                                    tab.invalidate_render();
                                    self.status_message =
                                        format!("Deleted form field '{}'", sel.name);
                                }
                                self.show_form_field_inspector = false;
                            }
                        }
                        if ui
                            .button("⚙ Field Props")
                            .on_hover_text("Configure form field properties")
                            .clicked()
                        {
                            self.show_form_field_inspector = true;
                        }
                        if ui
                            .button("📋 Duplicate to Pages...")
                            .on_hover_text("Duplicate field across multiple pages")
                            .clicked()
                        {
                            self.show_duplicate_field_dialog = true;
                        }
                    }

                    if !self.marked_redactions.is_empty() {
                        ui.separator();
                        let count = self.marked_redactions.len();
                        if ui
                            .button(
                                RichText::new(format!("⚠ Apply {} Redaction(s)", count))
                                    .color(Color32::from_rgb(239, 68, 68))
                                    .strong(),
                            )
                            .on_hover_text("Permanently apply marked redactions")
                            .clicked()
                        {
                            self.show_apply_redactions_confirm_dialog = true;
                        }
                        if ui
                            .small_button("Clear")
                            .on_hover_text("Clear all marked redactions")
                            .clicked()
                        {
                            self.marked_redactions.clear();
                        }
                    }

                    if has_doc && !self.text_selection.selected_text.is_empty() {
                        ui.separator();
                        if ui
                            .button(
                                RichText::new("📋 Copy Text")
                                    .color(Color32::from_rgb(59, 130, 246)),
                            )
                            .on_hover_text("Copy selected text to clipboard (Ctrl+C)")
                            .clicked()
                        {
                            ctx.output_mut(|o| {
                                o.copied_text = self.text_selection.selected_text.clone()
                            });
                            self.status_message = format!(
                                "Copied {} chars to clipboard",
                                self.text_selection.selected_text.len()
                            );
                        }
                    }
                });
            });
        });
    }

    /// Renders search and replace bar panel (FR-34.1, FR-04.8).
    fn render_search_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("search_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("🔍 Find:");
                let search_box = ui.add(
                    egui::TextEdit::singleline(&mut self.search_query)
                        .desired_width(180.0)
                        .hint_text("Search text..."),
                );

                if (search_box.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)))
                    || ui.button("Search").clicked()
                {
                    self.execute_search();
                }

                if !self.search_results.is_empty() {
                    ui.label(format!(
                        "Match {} of {}",
                        self.current_search_idx + 1,
                        self.search_results.len()
                    ));

                    if ui.button("▲ Prev").clicked() && self.current_search_idx > 0 {
                        self.current_search_idx -= 1;
                        let target_page = self.search_results[self.current_search_idx].page_index;
                        if let Some(tab) = self.active_tab_mut() {
                            tab.current_page = target_page;
                            tab.page_input_buffer = (target_page + 1).to_string();
                            tab.invalidate_render();
                        }
                    }
                    if ui.button("▼ Next").clicked()
                        && self.current_search_idx + 1 < self.search_results.len()
                    {
                        self.current_search_idx += 1;
                        let target_page = self.search_results[self.current_search_idx].page_index;
                        if let Some(tab) = self.active_tab_mut() {
                            tab.current_page = target_page;
                            tab.page_input_buffer = (target_page + 1).to_string();
                            tab.invalidate_render();
                        }
                    }
                } else if !self.search_query.is_empty() {
                    ui.weak("(No matches)");
                }

                ui.separator();
                ui.checkbox(&mut self.search_replace_mode, "Replace Mode");

                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui.button("✕ Close").clicked() {
                        self.search_bar_open = false;
                        self.search_results.clear();
                    }
                });
            });

            if self.search_replace_mode {
                ui.horizontal(|ui| {
                    ui.label("✏ Replace:");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.search_replace_text)
                            .desired_width(180.0)
                            .hint_text("Replacement text..."),
                    );

                    ui.checkbox(&mut self.search_case_sensitive, "Match Case");
                    ui.checkbox(&mut self.search_whole_word, "Whole Word");
                    ui.checkbox(&mut self.search_is_regex, "Regex");

                    let query = self.search_query.clone();
                    let replacement = self.search_replace_text.clone();
                    let case_sens = self.search_case_sensitive;
                    let whole_word = self.search_whole_word;
                    let is_regex = self.search_is_regex;

                    if ui.button("Replace Page").clicked() && !query.is_empty() {
                        let mut msg = None;
                        if let Some(tab) = self.active_tab_mut() {
                            let curr = tab.current_page;
                            match find_and_replace_page(
                                &mut tab.doc,
                                curr,
                                &query,
                                &replacement,
                                case_sens,
                                whole_word,
                                is_regex,
                            ) {
                                Ok(count) => {
                                    tab.is_modified = true;
                                    tab.invalidate_render();
                                    msg = Some(format!(
                                        "Replaced {} occurrences on page {}",
                                        count,
                                        curr + 1
                                    ));
                                }
                                Err(e) => {
                                    msg = Some(format!("Replace failed: {}", e));
                                }
                            }
                        }
                        if let Some(m) = msg {
                            self.status_message = m;
                        }
                    }

                    if ui.button("Replace All").clicked() && !query.is_empty() {
                        let mut msg = None;
                        if let Some(tab) = self.active_tab_mut() {
                            match find_and_replace(
                                &mut tab.doc,
                                &query,
                                &replacement,
                                case_sens,
                                whole_word,
                                is_regex,
                            ) {
                                Ok(count) => {
                                    tab.is_modified = true;
                                    tab.invalidate_render();
                                    msg = Some(format!(
                                        "Replaced {} occurrences across document",
                                        count
                                    ));
                                }
                                Err(e) => {
                                    msg = Some(format!("Replace all failed: {}", e));
                                }
                            }
                        }
                        if let Some(m) = msg {
                            self.status_message = m;
                        }
                    }
                });
            }
        });
    }

    /// Renders sidebar navigation and panels (EI-01.4, FR-01.2.4, FR-19).
    fn render_sidebar(&mut self, ctx: &egui::Context) {
        if !self.sidebar_open {
            return;
        }

        egui::SidePanel::left("left_sidebar")
            .default_width(240.0)
            .min_width(200.0)
            .show(ctx, |ui| {
                ui.add_space(2.0);
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(2.0, 2.0);
                    let row1 = [
                        (SidebarTab::Thumbnails, "📄", "Pages (Thumbnails)"),
                        (SidebarTab::Bookmarks, "📑", "Document Outline & Bookmarks"),
                        (SidebarTab::Destinations, "📌", "Named Destinations"),
                        (SidebarTab::Annotations, "💬", "Comments & Markup"),
                        (SidebarTab::Forms, "📝", "Interactive Form Fields"),
                    ];
                    for (tab, icon, tip) in row1 {
                        let selected = self.sidebar_tab == tab;
                        if ui
                            .selectable_label(selected, egui::RichText::new(icon).size(13.5))
                            .on_hover_text(tip)
                            .clicked()
                        {
                            self.sidebar_tab = tab;
                        }
                    }
                });
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(2.0, 2.0);
                    let row2 = [
                        (SidebarTab::Attachments, "📎", "Embedded Attachments"),
                        (SidebarTab::Signatures, "✍", "Digital Signatures"),
                        (SidebarTab::Properties, "ℹ", "Document Information"),
                        (SidebarTab::Search, "🔍", "Text Search Results"),
                        (SidebarTab::Accessibility, "♿", "Accessibility Audit"),
                    ];
                    for (tab, icon, tip) in row2 {
                        let selected = self.sidebar_tab == tab;
                        if ui
                            .selectable_label(selected, egui::RichText::new(icon).size(13.5))
                            .on_hover_text(tip)
                            .clicked()
                        {
                            self.sidebar_tab = tab;
                        }
                    }
                });

                ui.add_space(3.0);
                ui.separator();

                if self.active_tab().is_none() {
                    ui.vertical_centered(|ui| {
                        ui.add_space(30.0);
                        ui.weak("No document open");
                        ui.add_space(10.0);
                        if ui.button("Open PDF File").clicked() {
                            self.open_file_dialog();
                        }
                    });
                    return;
                }

                match self.sidebar_tab {
                    SidebarTab::Thumbnails => {
                        ui.horizontal(|ui| {
                            ui.heading("Pages");
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                if ui
                                    .button("➕ Blank")
                                    .on_hover_text("Insert blank A4 page at end")
                                    .clicked()
                                {
                                    if let Some(tab) = self.active_tab_mut() {
                                        let total = tab.doc.page_count();
                                        let _ = insert_blank_page(
                                            &mut tab.doc,
                                            total,
                                            paper_sizes::A4.0,
                                            paper_sizes::A4.1,
                                        );
                                        tab.current_page = total;
                                        tab.page_input_buffer = (total + 1).to_string();
                                        tab.is_modified = true;
                                        tab.invalidate_render();
                                        self.status_message = format!("Inserted blank page {}", total + 1);
                                    }
                                }
                                if ui
                                    .button("🏷 Labels")
                                    .on_hover_text("Configure page numbering schemes")
                                    .clicked()
                                {
                                    if let Some(tab) = self.active_tab() {
                                        self.page_labels_start_page = tab.current_page + 1;
                                    }
                                    self.show_page_labels_dialog = true;
                                }
                            });
                        });
                        ui.add_space(4.0);

                        let total_pages = self.active_tab().map(|t| t.doc.page_count()).unwrap_or(0);
                        let mut page_action = None;

                        ScrollArea::vertical().show(ui, |ui| {
                            for i in 0..total_pages {
                                let (thumb_tex, is_selected) = if let Some(tab) = self.active_tab_mut() {
                                    let tex = tab.get_or_render_thumbnail(ctx, i);
                                    let selected = tab.current_page == i;
                                    (tex, selected)
                                } else {
                                    (None, false)
                                };

                                let stroke = if is_selected {
                                    Stroke::new(2.0_f32, Color32::from_rgb(59, 130, 246))
                                } else {
                                    Stroke::new(1.0_f32, Color32::from_rgb(50, 50, 58))
                                };
                                let bg = if is_selected {
                                    Color32::from_rgb(30, 41, 59)
                                } else {
                                    Color32::from_rgb(24, 24, 28)
                                };

                                egui::Frame::none()
                                    .fill(bg)
                                    .stroke(stroke)
                                    .rounding(6.0)
                                    .inner_margin(egui::Margin::same(6.0))
                                    .show(ui, |ui| {
                                        ui.vertical_centered(|ui| {
                                            if let Some(tex) = thumb_tex {
                                                let (w, h) = (tex.size()[0] as f32, tex.size()[1] as f32);
                                                let target_w = 140.0_f32;
                                                let aspect = if w > 0.0 { h / w } else { 1.33 };
                                                let target_h = target_w * aspect;

                                                let img_btn = egui::ImageButton::new(
                                                    egui::Image::new(&tex)
                                                        .fit_to_exact_size(Vec2::new(target_w, target_h)),
                                                );
                                                if ui.add(img_btn).clicked() {
                                                    page_action = Some(PageAction::Select(i));
                                                }
                                            } else {
                                                let label = ui.selectable_label(
                                                    is_selected,
                                                    format!("Page {}", i + 1),
                                                );
                                                if label.clicked() {
                                                    page_action = Some(PageAction::Select(i));
                                                }
                                            }

                                            ui.add_space(3.0);
                                            ui.horizontal(|ui| {
                                                let lbl = if let Some(tab) = self.active_tab() {
                                                    get_page_label(&tab.doc, i)
                                                } else {
                                                    (i + 1).to_string()
                                                };
                                                ui.label(
                                                    RichText::new(lbl)
                                                        .size(11.0)
                                                        .strong(),
                                                );
                                                if ui
                                                    .small_button("⟳")
                                                    .on_hover_text("Rotate 90° clockwise")
                                                    .clicked()
                                                {
                                                    page_action = Some(PageAction::Rotate(i));
                                                }
                                                if i > 0
                                                    && ui
                                                        .small_button("▲")
                                                        .on_hover_text("Move page up")
                                                        .clicked()
                                                {
                                                    page_action = Some(PageAction::MoveUp(i));
                                                }
                                                if i + 1 < total_pages
                                                    && ui
                                                        .small_button("▼")
                                                        .on_hover_text("Move page down")
                                                        .clicked()
                                                {
                                                    page_action = Some(PageAction::MoveDown(i));
                                                }
                                                if ui
                                                    .small_button("📄")
                                                    .on_hover_text("Extract page to new tab")
                                                    .clicked()
                                                {
                                                    page_action = Some(PageAction::Extract(i));
                                                }
                                                if ui
                                                    .small_button("📋")
                                                    .on_hover_text("Duplicate page")
                                                    .clicked()
                                                {
                                                    page_action = Some(PageAction::Duplicate(i));
                                                }
                                                if ui
                                                    .small_button("✂")
                                                    .on_hover_text("Crop page")
                                                    .clicked()
                                                {
                                                    page_action = Some(PageAction::Crop(i));
                                                }
                                                if total_pages > 1
                                                    && ui
                                                        .small_button("🗑")
                                                        .on_hover_text("Delete page")
                                                        .clicked()
                                                {
                                                    page_action = Some(PageAction::Delete(i));
                                                }
                                            });
                                        });
                                    });
                                ui.add_space(6.0);
                            }
                        });

                        if let Some(act) = page_action {
                            match act {
                                PageAction::Select(idx) => {
                                    if let Some(tab) = self.active_tab_mut() {
                                        tab.current_page = idx;
                                        tab.page_input_buffer = (idx + 1).to_string();
                                        tab.invalidate_render();
                                    }
                                }
                                PageAction::Rotate(idx) => {
                                    if let Some(tab) = self.active_tab_mut() {
                                        let _ = rotate_page(&mut tab.doc, idx, 90);
                                        tab.is_modified = true;
                                        tab.invalidate_render();
                                        self.status_message = format!("Rotated page {}", idx + 1);
                                    }
                                }
                                PageAction::Delete(idx) => {
                                    if let Some(tab) = self.active_tab_mut() {
                                        if tab.doc.page_count() > 1 {
                                            let _ = delete_page(&mut tab.doc, idx);
                                            if tab.current_page >= tab.doc.page_count() {
                                                tab.current_page =
                                                    tab.doc.page_count().saturating_sub(1);
                                            }
                                            tab.page_input_buffer =
                                                (tab.current_page + 1).to_string();
                                            tab.is_modified = true;
                                            tab.invalidate_render();
                                            self.status_message = format!("Deleted page {}", idx + 1);
                                        }
                                    }
                                }
                                PageAction::MoveUp(idx) => {
                                    if idx > 0 {
                                        if let Some(tab) = self.active_tab_mut() {
                                            let total = tab.doc.page_count();
                                            let mut order: Vec<usize> = (0..total).collect();
                                            order.swap(idx, idx - 1);
                                            let _ = reorder_pages(&mut tab.doc, &order);
                                            if tab.current_page == idx {
                                                tab.current_page = idx - 1;
                                            } else if tab.current_page == idx - 1 {
                                                tab.current_page = idx;
                                            }
                                            tab.page_input_buffer =
                                                (tab.current_page + 1).to_string();
                                            tab.is_modified = true;
                                            tab.invalidate_render();
                                            self.status_message =
                                                format!("Moved page {} up", idx + 1);
                                        }
                                    }
                                }
                                PageAction::MoveDown(idx) => {
                                    if let Some(tab) = self.active_tab_mut() {
                                        let total = tab.doc.page_count();
                                        if idx + 1 < total {
                                            let mut order: Vec<usize> = (0..total).collect();
                                            order.swap(idx, idx + 1);
                                            let _ = reorder_pages(&mut tab.doc, &order);
                                            if tab.current_page == idx {
                                                tab.current_page = idx + 1;
                                            } else if tab.current_page == idx + 1 {
                                                tab.current_page = idx;
                                            }
                                            tab.page_input_buffer =
                                                (tab.current_page + 1).to_string();
                                            tab.is_modified = true;
                                            tab.invalidate_render();
                                            self.status_message =
                                                format!("Moved page {} down", idx + 1);
                                        }
                                    }
                                }
                                PageAction::Extract(idx) => {
                                    let extracted_info = self.active_tab().and_then(|tab| {
                                        extract_pages(&tab.doc, &[idx])
                                            .ok()
                                            .map(|extracted| (format!("{} (Page {})", tab.title, idx + 1), extracted))
                                    });
                                    if let Some((title, extracted)) = extracted_info {
                                        let tab_id = self.next_tab_id;
                                        self.next_tab_id += 1;
                                        let mut new_tab =
                                            DocumentTab::new(tab_id, title, extracted, None);
                                        new_tab.is_modified = true;
                                        self.tabs.push(new_tab);
                                        self.active_tab_idx = Some(self.tabs.len() - 1);
                                        self.status_message =
                                            format!("Extracted page {} into new tab", idx + 1);
                                    }
                                }
                                PageAction::Duplicate(idx) => {
                                    if let Some(tab) = self.active_tab_mut() {
                                        if let Ok(_new_id) = duplicate_page(&mut tab.doc, idx) {
                                            tab.current_page = idx + 1;
                                            tab.page_input_buffer = (idx + 2).to_string();
                                            tab.is_modified = true;
                                            tab.invalidate_render();
                                            self.status_message = format!("Duplicated page {} into page {}", idx + 1, idx + 2);
                                        }
                                    }
                                }
                                PageAction::Crop(idx) => {
                                    let boxes = self.active_tab().and_then(|tab| {
                                        get_page_boxes(&tab.doc, idx).ok()
                                    });
                                    self.crop_target_page = idx;
                                    if let Some(boxes) = boxes {
                                        let rect = match self.crop_box_type {
                                            PageBoxType::CropBox => boxes.crop_box,
                                            PageBoxType::MediaBox => boxes.media_box,
                                            PageBoxType::BleedBox => boxes.bleed_box,
                                            PageBoxType::TrimBox => boxes.trim_box,
                                            PageBoxType::ArtBox => boxes.art_box,
                                        };
                                        self.crop_left = rect[0];
                                        self.crop_bottom = rect[1];
                                        self.crop_right = rect[2];
                                        self.crop_top = rect[3];
                                    }
                                    self.show_crop_dialog = true;
                                }
                            }
                        }
                    }

                    SidebarTab::Bookmarks => {
                        // FR-19: Document Outlines / Bookmarks Tree
                        ui.heading("Bookmarks");
                        ui.add_space(4.0);

                        // Bookmark creation section (FR-19.2)
                        ui.horizontal(|ui| {
                            ui.text_edit_singleline(&mut self.new_bookmark_title);
                            if ui
                                .button("➕ Add")
                                .on_hover_text("Bookmark current page")
                                .clicked()
                                && !self.new_bookmark_title.trim().is_empty()
                            {
                                let bm_title = self.new_bookmark_title.clone();
                                if let Some(tab) = self.active_tab_mut() {
                                    let curr_page = tab.current_page;
                                    let _ = add_bookmark(&mut tab.doc, &bm_title, curr_page);
                                    tab.reload_bookmarks();
                                    tab.is_modified = true;
                                    self.status_message = format!("Bookmarked page {}", curr_page + 1);
                                }
                                self.new_bookmark_title.clear();
                            }
                        });

                        if ui.button("⚡ Auto-Generate from Headings").on_hover_text("Scan page headings and generate bookmark tree").clicked() {
                            if let Some(tab) = self.active_tab_mut() {
                                match ravpdf_core::outline::auto_generate_bookmarks_from_headings(&mut tab.doc) {
                                    Ok(count) => {
                                        tab.reload_bookmarks();
                                        tab.is_modified = true;
                                        self.status_message = format!("Auto-generated {} bookmark(s) from headings", count);
                                    }
                                    Err(e) => {
                                        self.status_message = format!("Failed to generate bookmarks: {}", e);
                                    }
                                }
                            }
                        }

                        ui.separator();

                        let mut target_page = None;
                        ScrollArea::vertical().show(ui, |ui| {
                            if let Some(tab) = self.active_tab() {
                                if tab.bookmarks.is_empty() {
                                    ui.weak("(No bookmarks in this document)");
                                } else {
                                    Self::render_bookmark_tree(ui, &tab.bookmarks, &mut target_page);
                                }
                            }
                        });

                        if let Some(page_idx) = target_page {
                            if let Some(tab) = self.active_tab_mut() {
                                tab.current_page = page_idx;
                                tab.page_input_buffer = (page_idx + 1).to_string();
                                tab.invalidate_render();
                            }
                        }
                    }

                    SidebarTab::Destinations => {
                        // FR-20.4: Named Destinations Panel conforming to ISO 32000-1 Section 12.3.2.3
                        ui.heading("Named Destinations");
                        ui.add_space(4.0);

                        // Destination creation section
                        ui.horizontal(|ui| {
                            ui.text_edit_singleline(&mut self.new_dest_name);
                            if ui
                                .button("➕ Add")
                                .on_hover_text("Add named destination to current page")
                                .clicked()
                                && !self.new_dest_name.trim().is_empty()
                            {
                                let dest_name = self.new_dest_name.clone();
                                if let Some(tab) = self.active_tab_mut() {
                                    let curr_page = tab.current_page;
                                    let view = DestinationView::Fit;
                                    if let Err(e) = add_named_destination(&mut tab.doc, &dest_name, curr_page, &view) {
                                        self.status_message = format!("Failed to add destination: {}", e);
                                    } else {
                                        tab.is_modified = true;
                                        self.status_message = format!("Added named destination '{}' (page {})", dest_name, curr_page + 1);
                                    }
                                }
                                self.new_dest_name.clear();
                            }
                        });

                        ui.separator();

                        let mut target_page = None;
                        let mut to_remove: Option<String> = None;

                        ScrollArea::vertical().show(ui, |ui| {
                            if let Some(tab) = self.active_tab() {
                                match get_named_destinations(&tab.doc) {
                                    Ok(dests) => {
                                        if dests.is_empty() {
                                            ui.weak("(No named destinations in this document)");
                                        } else {
                                            for d in dests {
                                                ui.horizontal(|ui| {
                                                    let label = format!("📌 {} (p. {})", d.name, d.page_index + 1);
                                                    if ui.link(label).on_hover_text(format!("Navigate to page {} with view {:?}", d.page_index + 1, d.view)).clicked() {
                                                        target_page = Some(d.page_index);
                                                    }
                                                    if ui.small_button("🗑").on_hover_text("Remove this destination").clicked() {
                                                        to_remove = Some(d.name.clone());
                                                    }
                                                });
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        ui.weak(format!("Error loading destinations: {}", e));
                                    }
                                }
                            }
                        });

                        if let Some(name) = to_remove {
                            if let Some(tab) = self.active_tab_mut() {
                                match remove_named_destination(&mut tab.doc, &name) {
                                    Ok(true) => {
                                        tab.is_modified = true;
                                        self.status_message = format!("Removed destination '{}'", name);
                                    }
                                    Ok(false) => {
                                        self.status_message = format!("Destination '{}' not found", name);
                                    }
                                    Err(e) => {
                                        self.status_message = format!("Failed to remove destination: {}", e);
                                    }
                                }
                            }
                        }

                        if let Some(page_idx) = target_page {
                            if let Some(tab) = self.active_tab_mut() {
                                tab.current_page = page_idx;
                                tab.page_input_buffer = (page_idx + 1).to_string();
                                tab.invalidate_render();
                            }
                        }
                    }

                    SidebarTab::Annotations => {
                        ui.heading("Comments & Markup");
                        ui.add_space(4.0);
                        let mut to_delete: Option<(usize, ObjectId)> = None;
                        let mut to_select: Option<SelectedAnnotation> = None;

                        if let Some(tab) = self.active_tab() {
                            let curr_page = tab.current_page;
                            if let Ok(annots) = get_page_annotations(&tab.doc, curr_page) {
                                if annots.is_empty() {
                                    ui.weak("(No annotations on this page)");
                                } else {
                                    ui.label(format!("{} annotation(s) on page {}:", annots.len(), curr_page + 1));
                                    ScrollArea::vertical().max_height(350.0).show(ui, |ui| {
                                        for (idx, annot) in annots.iter().enumerate() {
                                            let is_sel = self.selected_annotation.as_ref().map(|s| s.id) == annot.id;
                                            ui.group(|ui| {
                                                ui.horizontal(|ui| {
                                                    let icon = match annot.annot_type {
                                                        AnnotationType::Text => "💬 Note",
                                                        AnnotationType::Highlight => "🖍 Highlight",
                                                        AnnotationType::Underline => "U̲ Underline",
                                                        AnnotationType::StrikeOut => "S̶ StrikeOut",
                                                        AnnotationType::Square => "▭ Square",
                                                        AnnotationType::Circle => "◯ Circle",
                                                        AnnotationType::Ink => "✏ Ink",
                                                        AnnotationType::FreeText => "🗨 FreeText/Callout",
                                                        AnnotationType::Link { .. } => "🔗 Link",
                                                        AnnotationType::Stamp { .. } => "🏷 Stamp",
                                                        _ => "📌 Annotation",
                                                    };
                                                    ui.label(RichText::new(format!("#{}: {}", idx + 1, icon)).strong());
                                                    if let Some(annot_id) = annot.id {
                                                        if ui.small_button(if is_sel { "Selected" } else { "Select" }).clicked() {
                                                            to_select = Some(SelectedAnnotation {
                                                                id: annot_id,
                                                                page_index: curr_page,
                                                                annot_type: annot.annot_type.clone(),
                                                                rect: annot.rect,
                                                                contents: annot.contents.clone(),
                                                                author: annot.author.clone(),
                                                                color: annot.color,
                                                                opacity: annot.opacity,
                                                                is_dragging: false,
                                                                drag_start_rect: annot.rect,
                                                                drag_start_pointer: Pos2::ZERO,
                                                            });
                                                        }
                                                        if ui.small_button(RichText::new("🗑").color(Color32::from_rgb(239, 68, 68))).clicked() {
                                                            to_delete = Some((curr_page, annot_id));
                                                        }
                                                    }
                                                });
                                                if let Some(ref text) = annot.contents {
                                                    ui.label(RichText::new(text).size(11.0).italics());
                                                }
                                                if let Some(ref auth) = annot.author {
                                                    ui.label(RichText::new(format!("By: {}", auth)).size(10.0).weak());
                                                }
                                                ui.label(RichText::new(format!("Rect: [{:.0}, {:.0}, {:.0}, {:.0}]", annot.rect[0], annot.rect[1], annot.rect[2], annot.rect[3])).size(9.0).weak());
                                            });
                                        }
                                    });
                                }
                            }
                        }

                        if let Some(sel) = to_select {
                            self.annot_inspector_contents = sel.contents.clone().unwrap_or_default();
                            self.annot_inspector_author = sel.author.clone().unwrap_or_else(|| "User".into());
                            if let Some(c) = sel.color {
                                self.annot_inspector_color = [c.0, c.1, c.2];
                            }
                            self.annot_inspector_opacity = sel.opacity;
                            self.selected_annotation = Some(sel);
                            self.show_annot_inspector = true;
                        }

                        if let Some((p_idx, a_id)) = to_delete {
                            if let Some(tab) = self.active_tab_mut() {
                                let _ = delete_annotation(&mut tab.doc, p_idx, a_id);
                                tab.is_modified = true;
                                tab.invalidate_render();
                                self.status_message = "Deleted annotation".into();
                            }
                            if self.selected_annotation.as_ref().map(|s| s.id) == Some(a_id) {
                                self.selected_annotation = None;
                                self.show_annot_inspector = false;
                            }
                        }
                    }

                    SidebarTab::Properties => {
                        ui.heading("Properties & Metadata");
                        ui.add_space(4.0);

                        let mut open_full = false;
                        if let Some(tab) = self.active_tab_mut() {
                            let page_count = tab.doc.page_count();
                            let dims_str = if let Some(page) = tab.doc.get_page(tab.current_page) {
                                let pw = (page.crop_box[2] - page.crop_box[0]).abs();
                                let ph = (page.crop_box[3] - page.crop_box[1]).abs();
                                format!("{:.0} × {:.0} pt ({:.1} × {:.1} in)", pw, ph, pw / 72.0, ph / 72.0)
                            } else {
                                "Unknown".into()
                            };

                            let mut title_buf = tab.metadata.as_ref().and_then(|m| m.title.clone()).unwrap_or_default();
                            let mut author_buf = tab.metadata.as_ref().and_then(|m| m.author.clone()).unwrap_or_default();
                            let mut subject_buf = tab.metadata.as_ref().and_then(|m| m.subject.clone()).unwrap_or_default();
                            let mut kw_buf = tab.metadata.as_ref().and_then(|m| m.keywords.clone()).unwrap_or_default();

                            ui.label(RichText::new("Title:").strong());
                            let t_changed = ui.text_edit_singleline(&mut title_buf).changed();

                            ui.label(RichText::new("Author:").strong());
                            let a_changed = ui.text_edit_singleline(&mut author_buf).changed();

                            ui.label(RichText::new("Subject:").strong());
                            let s_changed = ui.text_edit_singleline(&mut subject_buf).changed();

                            ui.label(RichText::new("Keywords:").strong());
                            let k_changed = ui.text_edit_singleline(&mut kw_buf).changed();

                            if t_changed || a_changed || s_changed || k_changed {
                                let mut info = tab.metadata.clone().unwrap_or_default();
                                info.title = if title_buf.trim().is_empty() { None } else { Some(title_buf) };
                                info.author = if author_buf.trim().is_empty() { None } else { Some(author_buf) };
                                info.subject = if subject_buf.trim().is_empty() { None } else { Some(subject_buf) };
                                info.keywords = if kw_buf.trim().is_empty() { None } else { Some(kw_buf) };
                                let _ = set_document_info(&mut tab.doc, &info);
                                tab.metadata = Some(info);
                                tab.is_modified = true;
                            }

                            ui.separator();
                            ui.label(format!("Pages: {}", page_count));
                            ui.label(format!("Page Size: {}", dims_str));
                            ui.label(format!("PDF Version: {}.{}", tab.doc.version.0, tab.doc.version.1));
                            let enc = tab.doc.trailer.contains_key("Encrypt");
                            ui.label(format!("Security: {}", if enc { "AES-128 Encrypted" } else { "None" }));

                            ui.add_space(8.0);
                            if ui.button("Full Details Dialog (Ctrl+D)").clicked() {
                                open_full = true;
                            }
                        }

                        if open_full {
                            self.open_properties_dialog();
                        }
                    }

                    SidebarTab::Attachments => {
                        let is_port = self.active_tab().map(|t| is_portfolio(&t.doc)).unwrap_or(false);
                        if is_port {
                            ui.horizontal(|ui| {
                                ui.heading("🗂 Portfolio");
                                ui.label(RichText::new("ISO 32000-1").size(11.0).color(Color32::from_rgb(0, 180, 216)));
                            });
                            if let Some(tab) = self.active_tab() {
                                if let Ok(port) = get_portfolio(&tab.doc) {
                                    ui.weak(format!("Title: {}", port.title));
                                }
                            }
                        } else {
                            ui.heading("Attachments");
                        }
                        ui.add_space(4.0);

                        ui.horizontal(|ui| {
                            if ui.button("➕ Add File...").on_hover_text("Embed a file into this PDF").clicked() {
                                if let Some(file_path) = rfd::FileDialog::new().pick_file() {
                                    if let Ok(bytes) = std::fs::read(&file_path) {
                                        let filename = file_path.file_name().and_then(|n| n.to_str()).unwrap_or("attachment.bin").to_string();
                                        if let Some(tab) = self.active_tab_mut() {
                                            match add_attachment(&mut tab.doc, &filename, None, &bytes) {
                                                Ok(()) => {
                                                    tab.is_modified = true;
                                                    self.status_message = format!("Attached '{}' ({} bytes)", filename, bytes.len());
                                                }
                                                Err(e) => {
                                                    self.status_message = format!("Failed to attach file: {}", e);
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        });

                        ui.separator();

                        let attachments = self.active_tab().and_then(|tab| get_attachments(&tab.doc).ok()).unwrap_or_default();
                        if attachments.is_empty() {
                            ui.weak("(No embedded attachments in this document)");
                        } else {
                            let mut save_target: Option<FileAttachment> = None;
                            let mut remove_target: Option<String> = None;

                            ScrollArea::vertical().show(ui, |ui| {
                                for att in &attachments {
                                    ui.group(|ui| {
                                        ui.horizontal(|ui| {
                                            ui.label(RichText::new("📎").size(16.0));
                                            ui.vertical(|ui| {
                                                ui.label(RichText::new(&att.filename).strong());
                                                let sz_kb = att.size as f64 / 1024.0;
                                                ui.weak(format!("{:.1} KB ({} bytes)", sz_kb, att.size));
                                                if let Some(ref desc) = att.description {
                                                    ui.weak(desc);
                                                }
                                            });
                                        });
                                        ui.horizontal(|ui| {
                                            if ui.small_button("💾 Save As...").clicked() {
                                                save_target = Some(att.clone());
                                            }
                                            if ui.small_button("🗑 Remove").clicked() {
                                                remove_target = Some(att.filename.clone());
                                            }
                                        });
                                    });
                                    ui.add_space(4.0);
                                }
                            });

                            if let Some(att) = save_target {
                                if let Some(out_path) = rfd::FileDialog::new().set_file_name(&att.filename).save_file() {
                                    match std::fs::write(&out_path, &att.data) {
                                        Ok(()) => self.status_message = format!("Saved attachment to '{}'", out_path.display()),
                                        Err(e) => self.status_message = format!("Save failed: {}", e),
                                    }
                                }
                            }

                            if let Some(fname) = remove_target {
                                if let Some(tab) = self.active_tab_mut() {
                                    match remove_attachment(&mut tab.doc, &fname) {
                                        Ok(true) => {
                                            tab.is_modified = true;
                                            self.status_message = format!("Removed attachment '{}'", fname);
                                        }
                                        Ok(false) => {}
                                        Err(e) => self.status_message = format!("Removal failed: {}", e),
                                    }
                                }
                            }
                        }
                    }

                    SidebarTab::Signatures => {
                        ui.heading("Digital Signatures");
                        ui.add_space(4.0);

                        ui.horizontal(|ui| {
                            if ui.button("➕ Sign Document...").on_hover_text("Add digital signature field").clicked() {
                                if let Some(tab) = self.active_tab() {
                                    self.sig_target_page = tab.current_page;
                                }
                                self.show_add_signature_dialog = true;
                            }
                        });

                        ui.separator();

                        let signatures = self
                            .active_tab()
                            .and_then(|tab| {
                                get_document_signatures(&tab.doc, tab.raw_file_bytes.as_deref()).ok()
                            })
                            .unwrap_or_default();
                        if signatures.is_empty() {
                            ui.weak("(No digital signatures detected in this document)");
                        } else {
                            let mut jump_page = None;
                            ScrollArea::vertical().show(ui, |ui| {
                                for sig in &signatures {
                                    ui.group(|ui| {
                                        ui.horizontal(|ui| {
                                            let icon = match &sig.status {
                                                SignatureStatus::Valid(_) => RichText::new("✔").color(Color32::from_rgb(46, 204, 113)).strong(),
                                                SignatureStatus::Invalid(_) => RichText::new("✖").color(Color32::from_rgb(231, 76, 60)).strong(),
                                                SignatureStatus::Indeterminate(_) => RichText::new("❓").color(Color32::from_rgb(241, 196, 15)).strong(),
                                            };
                                            ui.label(icon);
                                            ui.label(RichText::new(&sig.field_name).strong());
                                        });

                                        if let Some(ref name) = sig.signer_name {
                                            ui.label(format!("Signer: {}", name));
                                        }
                                        if let Some(ref reason) = sig.reason {
                                            ui.weak(format!("Reason: {}", reason));
                                        }
                                        if let Some(ref loc) = sig.location {
                                            ui.weak(format!("Location: {}", loc));
                                        }
                                        if let Some(ref date) = sig.signing_time {
                                            ui.weak(format!("Date: {}", date));
                                        }

                                        match &sig.status {
                                            SignatureStatus::Valid(msg) => {
                                                ui.label(RichText::new(format!("Status: Valid ({})", msg)).color(Color32::from_rgb(46, 204, 113)).size(11.0));
                                            }
                                            SignatureStatus::Invalid(msg) => {
                                                ui.label(RichText::new(format!("Status: Invalid ({})", msg)).color(Color32::from_rgb(231, 76, 60)).size(11.0));
                                            }
                                            SignatureStatus::Indeterminate(msg) => {
                                                ui.label(RichText::new(format!("Status: Unverified ({})", msg)).color(Color32::from_rgb(241, 196, 15)).size(11.0));
                                            }
                                        }

                                        if sig.is_certification {
                                            let perm_desc = sig.doc_mdp_permission.map(|p| p.description()).unwrap_or("P=1: No changes allowed");
                                            ui.label(RichText::new(format!("🔒 Certified Author Signature ({})", perm_desc)).color(Color32::from_rgb(52, 152, 219)).size(11.0));
                                        }
                                        if sig.has_ltv {
                                            ui.label(RichText::new("🛡️ Long-Term Validation (LTV) Enabled (/DSS embedded)").color(Color32::from_rgb(39, 174, 96)).size(11.0));
                                        }

                                        if let Some(p_idx) = sig.page_index {
                                            if sig.is_visible
                                                && ui.small_button(format!("Go to page {}", p_idx + 1)).clicked()
                                            {
                                                jump_page = Some(p_idx);
                                            }
                                        }
                                    });
                                    ui.add_space(4.0);
                                }
                            });

                            if let Some(p) = jump_page {
                                if let Some(tab) = self.active_tab_mut() {
                                    tab.current_page = p;
                                    tab.page_input_buffer = (p + 1).to_string();
                                    tab.invalidate_render();
                                }
                            }
                        }
                    }

                    SidebarTab::Forms => {
                        ui.horizontal(|ui| {
                            ui.heading("Form Fields");
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                if ui
                                    .button("➕ Add")
                                    .on_hover_text("Add interactive form field")
                                    .clicked()
                                {
                                    self.show_add_form_field_dialog = true;
                                }
                            });
                        });
                        ui.add_space(4.0);

                        let mut flatten = false;
                        let mut auto_detect = false;
                        let mut import_data = false;

                        ui.horizontal(|ui| {
                            if ui.button("🔍 Auto-Detect").on_hover_text("Heuristically scan page for fields").clicked() {
                                auto_detect = true;
                            }
                            if ui.button("🔢 Tab Order").on_hover_text("Configure tab navigation sequence").clicked() {
                                self.show_tab_order_dialog = true;
                            }
                        });
                        ui.horizontal(|ui| {
                            if ui.button("📥 Import").on_hover_text("Import values from FDF/JSON/CSV/XML").clicked() {
                                import_data = true;
                            }
                            if ui.button("📤 Export").on_hover_text("Export values to FDF/JSON/CSV/XML").clicked() {
                                self.show_export_form_dialog = true;
                            }
                        });
                        ui.separator();

                        if let Some(tab) = self.active_tab() {
                            let fields = get_form_fields(&tab.doc).unwrap_or_default();
                            if fields.is_empty() {
                                ui.add_space(20.0);
                                ui.weak("No interactive form fields in document.");
                                ui.add_space(10.0);
                                if ui.button("➕ Add Interactive Field").clicked() {
                                    self.show_add_form_field_dialog = true;
                                }
                            } else {
                                ui.horizontal(|ui| {
                                    ui.label(RichText::new(format!("{} fields", fields.len())).strong());
                                    if ui
                                        .button("Flatten All")
                                        .on_hover_text("Permanently burn form fields into content")
                                        .clicked()
                                    {
                                        flatten = true;
                                    }
                                });
                                ui.separator();

                                ScrollArea::vertical().show(ui, |ui| {
                                    for f in &fields {
                                        egui::Frame::none()
                                            .fill(Color32::from_rgb(25, 30, 40))
                                            .stroke(Stroke::new(1.0_f32, Color32::from_gray(60)))
                                            .rounding(4.0)
                                            .inner_margin(egui::Margin::same(6.0))
                                            .show(ui, |ui| {
                                                let type_str = match f.field_type {
                                                    FormFieldType::Text => "📝 Text",
                                                    FormFieldType::Checkbox => "☑ Checkbox",
                                                    FormFieldType::Radio => "🔘 Radio",
                                                    FormFieldType::Choice => "▾ Dropdown",
                                                    FormFieldType::ListBox => "📋 List Box",
                                                    FormFieldType::PushButton => "🔘 Button",
                                                    FormFieldType::Barcode => "📊 Barcode",
                                                    FormFieldType::Signature => "🔏 Signature",
                                                };
                                                ui.horizontal(|ui| {
                                                    ui.label(
                                                        RichText::new(type_str)
                                                            .color(Color32::from_rgb(66, 153, 225))
                                                            .size(11.0),
                                                    );
                                                    ui.label(RichText::new(&f.name).strong());
                                                });
                                                if let Some(ref val) = f.value {
                                                    ui.label(RichText::new(format!("Value: {}", val)).size(11.0).weak());
                                                }
                                                if !f.options.is_empty() {
                                                    ui.label(
                                                        RichText::new(format!("Options: {}", f.options.join(", ")))
                                                            .size(10.0)
                                                            .weak(),
                                                    );
                                                }
                                            });
                                        ui.add_space(4.0);
                                    }
                                });
                            }
                        }

                        if auto_detect {
                            if let Some(tab) = self.active_tab_mut() {
                                let curr = tab.current_page;
                                if let Ok(fields) = auto_detect_form_fields(&tab.doc, curr) {
                                    let count = fields.len();
                                    if apply_auto_detected_fields(&mut tab.doc, curr, &fields).is_ok() {
                                        tab.is_modified = true;
                                        tab.invalidate_render();
                                        self.status_message = format!("Auto-detected and applied {} form field(s) on page {}", count, curr + 1);
                                    }
                                }
                            }
                        }

                        if import_data {
                            if let Some(path) = rfd::FileDialog::new()
                                .add_filter("Form Data Files (*.json, *.fdf, *.xfdf, *.csv, *.xml)", &["json", "fdf", "xfdf", "csv", "xml"])
                                .pick_file()
                            {
                                if let Ok(content) = std::fs::read_to_string(&path) {
                                    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
                                    let fmt = match ext.to_lowercase().as_str() {
                                        "fdf" => FormDataFormat::Fdf,
                                        "xfdf" => FormDataFormat::Xfdf,
                                        "csv" => FormDataFormat::Csv,
                                        "xml" => FormDataFormat::Xml,
                                        _ => FormDataFormat::Json,
                                    };
                                    if let Some(tab) = self.active_tab_mut() {
                                        match import_form_data(&mut tab.doc, &content, fmt) {
                                            Ok(count) => {
                                                tab.is_modified = true;
                                                tab.invalidate_render();
                                                self.status_message = format!("Imported {} form field value(s) from {}", count, path.display());
                                            }
                                            Err(e) => {
                                                self.status_message = format!("Form import failed: {}", e);
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        if flatten {
                            if let Some(tab) = self.active_tab_mut() {
                                let _ = flatten_form(&mut tab.doc);
                                tab.is_modified = true;
                                tab.invalidate_render();
                                self.status_message = "All form fields flattened to page content".into();
                            }
                        }
                    }

                    SidebarTab::Search => {
                        ui.label(format!("Search Results ({}):", self.search_results.len()));
                        let mut clicked_match = None;
                        ScrollArea::vertical().show(ui, |ui| {
                            for (idx, res) in self.search_results.iter().enumerate() {
                                let is_sel = self.current_search_idx == idx;
                                if ui
                                    .selectable_label(
                                        is_sel,
                                        format!("p.{}: {}", res.page_index + 1, res.match_text),
                                    )
                                    .clicked()
                                {
                                    clicked_match = Some((idx, res.page_index));
                                }
                                ui.weak(&res.context_snippet);
                                ui.separator();
                            }
                        });

                        if let Some((idx, p_idx)) = clicked_match {
                            self.current_search_idx = idx;
                            if let Some(tab) = self.active_tab_mut() {
                                tab.current_page = p_idx;
                                tab.page_input_buffer = (p_idx + 1).to_string();
                                tab.invalidate_render();
                            }
                        }
                    }

                    SidebarTab::Accessibility => {
                        ui.horizontal(|ui| {
                            ui.heading("Accessibility");
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                if ui.button("Auditing...").on_hover_text("Refresh accessibility audit").clicked() {
                                    if let Some(tab) = self.active_tab() {
                                        self.accessibility_report = audit_accessibility(&tab.doc).ok();
                                    }
                                }
                            });
                        });
                        ui.add_space(4.0);

                        let mut run_remediation = false;
                        let mut open_dialog = false;

                        if let Some(tab) = self.active_tab() {
                            if self.accessibility_report.is_none() {
                                self.accessibility_report = audit_accessibility(&tab.doc).ok();
                            }

                            if let Some(ref report) = self.accessibility_report {
                                let (badge_color, status_text) = if report.is_compliant {
                                    (Color32::from_rgb(46, 204, 113), "PDF/UA COMPLIANT")
                                } else {
                                    (Color32::from_rgb(231, 76, 60), "NON-COMPLIANT")
                                };

                                egui::Frame::none()
                                    .fill(Color32::from_rgb(25, 30, 40))
                                    .stroke(Stroke::new(1.0_f32, Color32::from_gray(60)))
                                    .rounding(4.0)
                                    .inner_margin(egui::Margin::same(8.0))
                                    .show(ui, |ui| {
                                        ui.horizontal(|ui| {
                                            ui.label(RichText::new(status_text).color(badge_color).strong().size(12.0));
                                        });
                                        ui.add_space(2.0);
                                        ui.label(
                                            RichText::new(format!(
                                                "✓ {} Passed   ✕ {} Failed   ⚠ {} Check",
                                                report.passed_count, report.failed_count, report.manual_check_count
                                            ))
                                            .size(11.0),
                                        );
                                    });

                                ui.add_space(6.0);
                                if !report.is_compliant {
                                    if ui
                                        .button("⚡ Quick Fix All Automatically")
                                        .on_hover_text("Remediate title, language, tagged PDF structure, and tab order")
                                        .clicked()
                                    {
                                        run_remediation = true;
                                    }
                                    ui.add_space(4.0);
                                }

                                ScrollArea::vertical().show(ui, |ui| {
                                    for rule in &report.rules {
                                        let (icon, icon_color) = match rule.status {
                                            AccessibilityStatus::Passed => ("✓", Color32::from_rgb(46, 204, 113)),
                                            AccessibilityStatus::Failed => ("✕", Color32::from_rgb(231, 76, 60)),
                                            AccessibilityStatus::NeedsManualCheck => ("⚠", Color32::from_rgb(241, 196, 15)),
                                        };

                                        egui::Frame::none()
                                            .fill(Color32::from_rgb(20, 24, 32))
                                            .stroke(Stroke::new(1.0_f32, Color32::from_gray(50)))
                                            .rounding(4.0)
                                            .inner_margin(egui::Margin::same(6.0))
                                            .show(ui, |ui| {
                                                ui.horizontal(|ui| {
                                                    ui.label(RichText::new(icon).color(icon_color).strong());
                                                    ui.label(RichText::new(&rule.title).strong().size(11.0));
                                                });
                                                ui.label(RichText::new(&rule.description).weak().size(10.0));
                                            });
                                        ui.add_space(3.0);
                                    }

                                    ui.add_space(8.0);
                                    if ui.button("Full Accessibility Report...").clicked() {
                                        open_dialog = true;
                                    }
                                });
                            }
                        }

                        if run_remediation {
                            let mut updated_opt = None;
                            if let Some(tab) = self.active_tab_mut() {
                                let default_title = tab.title.trim_end_matches(".pdf");
                                if let Ok(updated_report) = remediate_all_automatic(&mut tab.doc, default_title, "en-US") {
                                    tab.is_modified = true;
                                    tab.invalidate_render();
                                    updated_opt = Some(updated_report);
                                }
                            }
                            if let Some(updated) = updated_opt {
                                self.accessibility_report = Some(updated);
                                self.status_message = "Applied automatic accessibility remediations".into();
                            }
                        }

                        if open_dialog {
                            self.show_accessibility_dialog = true;
                        }
                    }
                }
            });
    }

    /// Renders a hierarchical list of bookmarks (FR-19.1).
    fn render_bookmark_tree(
        ui: &mut egui::Ui,
        bookmarks: &[Bookmark],
        target_page: &mut Option<usize>,
    ) {
        for bm in bookmarks {
            let label = if let Some(p) = bm.page_index {
                format!("🔖 {} [p.{}]", bm.title, p + 1)
            } else {
                format!("🔖 {}", bm.title)
            };

            let mut rich = RichText::new(label);
            if bm.is_bold {
                rich = rich.strong();
            }
            if bm.is_italic {
                rich = rich.italics();
            }

            if bm.children.is_empty() {
                if ui.button(rich).clicked() {
                    if let Some(p) = bm.page_index {
                        *target_page = Some(p);
                    }
                }
            } else {
                egui::CollapsingHeader::new(rich)
                    .default_open(true)
                    .show(ui, |ui| {
                        if let Some(p) = bm.page_index {
                            if ui.small_button("Jump to section").clicked() {
                                *target_page = Some(p);
                            }
                        }
                        Self::render_bookmark_tree(ui, &bm.children, target_page);
                    });
            }
        }
    }

    /// Renders bottom status bar (EI-01.5).
    fn render_status_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(&self.status_message);
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if let Some(tab) = self.active_tab() {
                        ui.label(format!("Zoom: {:.0}%", tab.zoom * 100.0));
                        ui.separator();
                        if let Some(p) = tab.doc.get_page(tab.current_page) {
                            let w = (p.crop_box[2] - p.crop_box[0]).abs();
                            let h = (p.crop_box[3] - p.crop_box[1]).abs();
                            ui.label(format!("{:.1} x {:.1} pt", w, h));
                            ui.separator();
                        }
                        let viewports = get_page_geospatial_viewports(&tab.doc, tab.current_page)
                            .unwrap_or_default();
                        if !viewports.is_empty() {
                            ui.label(
                                RichText::new("🌐 Geospatial PDF (WGS84)")
                                    .color(Color32::from_rgb(0, 180, 216))
                                    .size(11.0),
                            );
                            ui.separator();
                        }
                        if self.active_tool == ActiveTool::MeasureDistance
                            || self.active_tool == ActiveTool::MeasureArea
                        {
                            ui.label(
                                RichText::new(format!(
                                    "Scale: {}",
                                    self.measure_scale.to_display_string()
                                ))
                                .size(11.0)
                                .color(Color32::from_rgb(100, 220, 255)),
                            );
                            ui.separator();
                        }
                        ui.label(format!("Tool: {:?}", self.active_tool));
                    }
                });
            });
        });
    }

    /// Renders the multi-column Organize Pages grid view conforming to FR-06.1 - FR-06.11.
    fn render_organize_pages_grid(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let total_pages = self.active_tab().map(|t| t.doc.page_count()).unwrap_or(0);
        let mut organize_action: Option<OrganizeAction> = None;

        ui.vertical(|ui| {
            // Organize Pages Toolbar
            ui.horizontal_wrapped(|ui| {
                ui.heading(
                    RichText::new("🗂 Organize Pages")
                        .size(15.0)
                        .strong(),
                );
                ui.separator();

                if ui
                    .button("➕ Insert Blank")
                    .on_hover_text("Insert blank A4 page at end")
                    .clicked()
                {
                    organize_action = Some(OrganizeAction::InsertBlank);
                }
                let has_selection = !self.organize_selected_pages.is_empty();
                if ui
                    .add_enabled(has_selection, Button::new("⟳ Rotate 90° CW"))
                    .on_hover_text("Rotate selected pages 90° clockwise")
                    .clicked()
                {
                    organize_action = Some(OrganizeAction::RotateCW);
                }
                if ui
                    .add_enabled(has_selection, Button::new("⟲ Rotate 90° CCW"))
                    .on_hover_text("Rotate selected pages 90° counter-clockwise")
                    .clicked()
                {
                    organize_action = Some(OrganizeAction::RotateCCW);
                }
                if ui
                    .add_enabled(has_selection, Button::new("📄 Extract"))
                    .on_hover_text("Extract selected pages to a new document tab")
                    .clicked()
                {
                    organize_action = Some(OrganizeAction::Extract);
                }
                if ui
                    .add_enabled(has_selection, Button::new("📋 Duplicate"))
                    .on_hover_text("Duplicate selected pages")
                    .clicked()
                {
                    organize_action = Some(OrganizeAction::Duplicate);
                }
                let can_delete = has_selection && total_pages > self.organize_selected_pages.len();
                if ui
                    .add_enabled(
                        can_delete,
                        Button::new(
                            RichText::new("🗑 Delete").color(Color32::from_rgb(239, 68, 68)),
                        ),
                    )
                    .on_hover_text("Delete selected pages")
                    .clicked()
                {
                    organize_action = Some(OrganizeAction::Delete);
                }

                ui.separator();
                if ui.button("Select All").clicked() {
                    for i in 0..total_pages {
                        self.organize_selected_pages.insert(i);
                    }
                }
                if ui.button("Deselect All").clicked() {
                    self.organize_selected_pages.clear();
                }

                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui
                        .button(RichText::new("✕ Close (Single Page)").strong())
                        .clicked()
                    {
                        self.page_view_mode = PageViewMode::SinglePage;
                    }
                });
            });

            ui.separator();

            let avail_w = ui.available_width();
            let card_w = 160.0_f32;
            let card_spacing = 14.0_f32;
            let num_cols = ((avail_w + card_spacing) / (card_w + card_spacing))
                .max(1.0)
                .floor() as usize;

            ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.add_space(8.0);
                    for chunk_start in (0..total_pages).step_by(num_cols) {
                        ui.horizontal(|ui| {
                            ui.add_space(10.0);
                            for col in 0..num_cols {
                                let page_idx = chunk_start + col;
                                if page_idx >= total_pages {
                                    break;
                                }

                                let is_selected = self.organize_selected_pages.contains(&page_idx);
                                let is_current = self
                                    .active_tab()
                                    .map(|t| t.current_page == page_idx)
                                    .unwrap_or(false);

                                let border_stroke = if is_selected {
                                    Stroke::new(2.5_f32, Color32::from_rgb(66, 153, 225))
                                } else if is_current {
                                    Stroke::new(1.5_f32, Color32::from_rgb(139, 92, 246))
                                } else {
                                    Stroke::new(1.0_f32, Color32::from_gray(60))
                                };

                                let bg_color = if is_selected {
                                    Color32::from_rgb(30, 48, 75)
                                } else {
                                    Color32::from_rgb(22, 27, 34)
                                };

                                egui::Frame::none()
                                    .fill(bg_color)
                                    .stroke(border_stroke)
                                    .rounding(6.0)
                                    .inner_margin(egui::Margin::same(6.0))
                                    .show(ui, |ui| {
                                        ui.set_width(card_w);
                                        ui.vertical_centered(|ui| {
                                            // Header
                                            ui.horizontal(|ui| {
                                                let mut sel_val = is_selected;
                                                if ui.checkbox(&mut sel_val, "").changed() {
                                                    if sel_val {
                                                        self.organize_selected_pages
                                                            .insert(page_idx);
                                                    } else {
                                                        self.organize_selected_pages
                                                            .remove(&page_idx);
                                                    }
                                                }
                                                let page_lbl = if let Some(tab) = self.active_tab()
                                                {
                                                    get_page_label(&tab.doc, page_idx)
                                                } else {
                                                    (page_idx + 1).to_string()
                                                };
                                                ui.label(
                                                    RichText::new(format!("Page {}", page_lbl))
                                                        .strong()
                                                        .size(11.0),
                                                );
                                            });

                                            ui.add_space(3.0);

                                            // Thumbnail
                                            let tex_opt = if let Some(tab) = self.active_tab_mut() {
                                                tab.get_or_render_thumbnail(ctx, page_idx)
                                            } else {
                                                None
                                            };

                                            let target_w = card_w - 12.0;
                                            let target_h = target_w * 1.35;

                                            if let Some(tex) = tex_opt {
                                                let img_resp = ui.add(egui::ImageButton::new(
                                                    egui::Image::new(&tex).fit_to_exact_size(
                                                        Vec2::new(target_w, target_h),
                                                    ),
                                                ));
                                                if img_resp.clicked() {
                                                    if self
                                                        .organize_selected_pages
                                                        .contains(&page_idx)
                                                    {
                                                        self.organize_selected_pages
                                                            .remove(&page_idx);
                                                    } else {
                                                        self.organize_selected_pages
                                                            .insert(page_idx);
                                                    }
                                                }
                                                if img_resp.double_clicked() {
                                                    if let Some(tab) = self.active_tab_mut() {
                                                        tab.current_page = page_idx;
                                                        tab.page_input_buffer =
                                                            (page_idx + 1).to_string();
                                                        tab.invalidate_render();
                                                    }
                                                    self.page_view_mode = PageViewMode::SinglePage;
                                                }
                                            } else {
                                                let (r, resp) = ui.allocate_exact_size(
                                                    Vec2::new(target_w, target_h),
                                                    egui::Sense::click(),
                                                );
                                                ui.painter().rect_filled(
                                                    r,
                                                    4.0,
                                                    Color32::from_gray(35),
                                                );
                                                ui.painter().text(
                                                    r.center(),
                                                    Align2::CENTER_CENTER,
                                                    format!("Page {}", page_idx + 1),
                                                    FontId::proportional(12.0),
                                                    Color32::WHITE,
                                                );
                                                if resp.clicked() {
                                                    if self
                                                        .organize_selected_pages
                                                        .contains(&page_idx)
                                                    {
                                                        self.organize_selected_pages
                                                            .remove(&page_idx);
                                                    } else {
                                                        self.organize_selected_pages
                                                            .insert(page_idx);
                                                    }
                                                }
                                            }

                                            ui.add_space(4.0);

                                            // Action buttons
                                            ui.horizontal(|ui| {
                                                if ui
                                                    .small_button("⟳")
                                                    .on_hover_text("Rotate 90° CW")
                                                    .clicked()
                                                {
                                                    organize_action = Some(
                                                        OrganizeAction::SingleRotate(page_idx, 90),
                                                    );
                                                }
                                                if ui
                                                    .small_button("⟲")
                                                    .on_hover_text("Rotate 90° CCW")
                                                    .clicked()
                                                {
                                                    organize_action = Some(
                                                        OrganizeAction::SingleRotate(page_idx, 270),
                                                    );
                                                }
                                                if page_idx > 0
                                                    && ui
                                                        .small_button("◀")
                                                        .on_hover_text("Move page left")
                                                        .clicked()
                                                {
                                                    organize_action =
                                                        Some(OrganizeAction::MoveLeft(page_idx));
                                                }
                                                if page_idx + 1 < total_pages
                                                    && ui
                                                        .small_button("▶")
                                                        .on_hover_text("Move page right")
                                                        .clicked()
                                                {
                                                    organize_action =
                                                        Some(OrganizeAction::MoveRight(page_idx));
                                                }
                                                if total_pages > 1
                                                    && ui
                                                        .small_button("🗑")
                                                        .on_hover_text("Delete page")
                                                        .clicked()
                                                {
                                                    organize_action = Some(
                                                        OrganizeAction::SingleDelete(page_idx),
                                                    );
                                                }
                                            });
                                        });
                                    });

                                ui.add_space(card_spacing);
                            }
                        });
                        ui.add_space(card_spacing);
                    }
                });
        });

        if let Some(act) = organize_action {
            self.execute_organize_action(act);
        }
    }

    fn execute_organize_action(&mut self, action: OrganizeAction) {
        match action {
            OrganizeAction::InsertBlank => {
                if let Some(tab) = self.active_tab_mut() {
                    let total = tab.doc.page_count();
                    let _ = insert_blank_page(
                        &mut tab.doc,
                        total,
                        paper_sizes::A4.0,
                        paper_sizes::A4.1,
                    );
                    tab.current_page = total;
                    tab.page_input_buffer = (total + 1).to_string();
                    tab.is_modified = true;
                    tab.invalidate_render();
                    self.status_message = format!("Inserted blank page {}", total + 1);
                }
            }
            OrganizeAction::RotateCW => {
                let pages: Vec<usize> = self.organize_selected_pages.iter().copied().collect();
                if let Some(tab) = self.active_tab_mut() {
                    for p in pages {
                        let _ = rotate_page(&mut tab.doc, p, 90);
                    }
                    tab.is_modified = true;
                    tab.invalidate_render();
                    self.status_message = "Rotated selected pages 90° CW".into();
                }
            }
            OrganizeAction::RotateCCW => {
                let pages: Vec<usize> = self.organize_selected_pages.iter().copied().collect();
                if let Some(tab) = self.active_tab_mut() {
                    for p in pages {
                        let _ = rotate_page(&mut tab.doc, p, 270);
                    }
                    tab.is_modified = true;
                    tab.invalidate_render();
                    self.status_message = "Rotated selected pages 90° CCW".into();
                }
            }
            OrganizeAction::Extract => {
                let mut pages: Vec<usize> = self.organize_selected_pages.iter().copied().collect();
                pages.sort_unstable();
                if !pages.is_empty() {
                    let tab_title = self.active_tab().map(|t| t.title.clone());
                    let extracted = self
                        .active_tab()
                        .and_then(|t| extract_pages(&t.doc, &pages).ok());
                    if let (Some(title), Some(new_doc)) = (tab_title, extracted) {
                        let new_tab_id = self.next_tab_id;
                        self.next_tab_id += 1;
                        let title = format!("{}-extracted.pdf", title.trim_end_matches(".pdf"));
                        self.tabs
                            .push(DocumentTab::new(new_tab_id, title, new_doc, None));
                        self.active_tab_idx = Some(self.tabs.len() - 1);
                        self.page_view_mode = PageViewMode::SinglePage;
                        self.status_message =
                            format!("Extracted {} page(s) to new tab", pages.len());
                    }
                }
            }
            OrganizeAction::Duplicate => {
                let mut pages: Vec<usize> = self.organize_selected_pages.iter().copied().collect();
                pages.sort_by(|a, b| b.cmp(a));
                if let Some(tab) = self.active_tab_mut() {
                    let mut count = 0;
                    for p in pages {
                        if duplicate_page(&mut tab.doc, p).is_ok() {
                            count += 1;
                        }
                    }
                    if count > 0 {
                        tab.is_modified = true;
                        tab.invalidate_render();
                        self.organize_selected_pages.clear();
                        self.status_message = format!("Duplicated {} page(s)", count);
                    }
                }
            }
            OrganizeAction::Delete => {
                let mut pages: Vec<usize> = self.organize_selected_pages.iter().copied().collect();
                pages.sort_by(|a, b| b.cmp(a));
                if let Some(tab) = self.active_tab_mut() {
                    let total = tab.doc.page_count();
                    let mut deleted = 0;
                    for p in pages {
                        if total - deleted > 1
                            && p < tab.doc.page_count()
                            && delete_page(&mut tab.doc, p).is_ok()
                        {
                            deleted += 1;
                        }
                    }
                    if deleted > 0 {
                        tab.current_page =
                            tab.current_page.min(tab.doc.page_count().saturating_sub(1));
                        tab.page_input_buffer = (tab.current_page + 1).to_string();
                        tab.is_modified = true;
                        tab.invalidate_render();
                        self.organize_selected_pages.clear();
                        self.status_message = format!("Deleted {} page(s)", deleted);
                    }
                }
            }
            OrganizeAction::SingleRotate(idx, angle) => {
                if let Some(tab) = self.active_tab_mut() {
                    let _ = rotate_page(&mut tab.doc, idx, angle);
                    tab.is_modified = true;
                    tab.invalidate_render();
                    self.status_message = format!("Rotated page {} by {}°", idx + 1, angle);
                }
            }
            OrganizeAction::SingleDelete(idx) => {
                if let Some(tab) = self.active_tab_mut() {
                    if tab.doc.page_count() > 1 {
                        let _ = delete_page(&mut tab.doc, idx);
                        tab.current_page =
                            tab.current_page.min(tab.doc.page_count().saturating_sub(1));
                        tab.page_input_buffer = (tab.current_page + 1).to_string();
                        tab.is_modified = true;
                        tab.invalidate_render();
                        self.organize_selected_pages.remove(&idx);
                        self.status_message = format!("Deleted page {}", idx + 1);
                    }
                }
            }
            OrganizeAction::MoveLeft(idx) => {
                if idx > 0 {
                    if let Some(tab) = self.active_tab_mut() {
                        let total = tab.doc.page_count();
                        let mut order: Vec<usize> = (0..total).collect();
                        order.swap(idx, idx - 1);
                        let _ = reorder_pages(&mut tab.doc, &order);
                        tab.is_modified = true;
                        tab.invalidate_render();
                        self.status_message = format!("Moved page {} left", idx + 1);
                    }
                }
            }
            OrganizeAction::MoveRight(idx) => {
                if let Some(tab) = self.active_tab_mut() {
                    let total = tab.doc.page_count();
                    if idx + 1 < total {
                        let mut order: Vec<usize> = (0..total).collect();
                        order.swap(idx, idx + 1);
                        let _ = reorder_pages(&mut tab.doc, &order);
                        tab.is_modified = true;
                        tab.invalidate_render();
                        self.status_message = format!("Moved page {} right", idx + 1);
                    }
                }
            }
        }
    }

    /// Renders dual-pane split-view mode allowing inspection of two separate pages simultaneously (FR-01.3.5).
    fn render_split_view(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let total_pages = self.active_tab().map(|t| t.doc.page_count()).unwrap_or(0);
        if total_pages == 0 {
            return;
        }

        let left_page = self.active_tab().map(|t| t.current_page).unwrap_or(0);
        let right_page = self
            .split_view_second_page
            .min(total_pages.saturating_sub(1));

        let mut new_left = None;
        let mut new_right = None;
        let mut close_split = false;

        ui.horizontal(|ui| {
            ui.label(RichText::new("Split View Mode").strong());
            ui.separator();
            if ui.button("✖ Close Split View").clicked() {
                close_split = true;
            }
        });
        ui.separator();

        ui.columns(2, |cols| {
            // Left Pane (Primary page)
            let col0 = &mut cols[0];
            col0.horizontal(|ui| {
                ui.label(
                    RichText::new(format!("Left: Page {} of {}", left_page + 1, total_pages))
                        .strong(),
                );
                if ui.button("◀").clicked() && left_page > 0 {
                    new_left = Some(left_page - 1);
                }
                if ui.button("▶").clicked() && left_page + 1 < total_pages {
                    new_left = Some(left_page + 1);
                }
            });
            ScrollArea::both()
                .id_salt("split_left_scroll")
                .auto_shrink([false, false])
                .show(col0, |ui| {
                    let tex_opt = if let Some(tab) = self.active_tab_mut() {
                        tab.get_or_render_continuous_page(ctx, left_page)
                    } else {
                        None
                    };
                    if let Some(tex) = tex_opt {
                        let size = tex.size_vec2() / ctx.pixels_per_point();
                        let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
                        ui.painter().rect_filled(
                            rect.expand(2.0),
                            0.0,
                            Color32::from_black_alpha(40),
                        );
                        ui.painter().rect_filled(rect, 0.0, Color32::WHITE);
                        ui.painter().image(
                            tex.id(),
                            rect,
                            Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                            Color32::WHITE,
                        );
                    }
                });

            // Right Pane (Secondary page)
            let col1 = &mut cols[1];
            col1.horizontal(|ui| {
                ui.label(
                    RichText::new(format!("Right: Page {} of {}", right_page + 1, total_pages))
                        .strong(),
                );
                if ui.button("◀").clicked() && right_page > 0 {
                    new_right = Some(right_page - 1);
                }
                if ui.button("▶").clicked() && right_page + 1 < total_pages {
                    new_right = Some(right_page + 1);
                }
            });
            ScrollArea::both()
                .id_salt("split_right_scroll")
                .auto_shrink([false, false])
                .show(col1, |ui| {
                    let tex_opt = if let Some(tab) = self.active_tab_mut() {
                        tab.get_or_render_continuous_page(ctx, right_page)
                    } else {
                        None
                    };
                    if let Some(tex) = tex_opt {
                        let size = tex.size_vec2() / ctx.pixels_per_point();
                        let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
                        ui.painter().rect_filled(
                            rect.expand(2.0),
                            0.0,
                            Color32::from_black_alpha(40),
                        );
                        ui.painter().rect_filled(rect, 0.0, Color32::WHITE);
                        ui.painter().image(
                            tex.id(),
                            rect,
                            Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                            Color32::WHITE,
                        );
                    }
                });
        });

        if close_split {
            self.split_view_enabled = false;
        }
        if let Some(nl) = new_left {
            self.record_page_navigation(nl);
        }
        if let Some(nr) = new_right {
            self.split_view_second_page = nr;
        }
    }

    /// Helper to get dimensions of a page in PDF points.
    fn get_page_dim(&self, page_idx: usize) -> (f32, f32) {
        if let Some(tab) = self.active_tab() {
            if let Some(p) = tab.doc.get_page(page_idx) {
                let w = (p.crop_box[2] - p.crop_box[0]).abs() as f32;
                let h = (p.crop_box[3] - p.crop_box[1]).abs() as f32;
                return (w, h);
            }
        }
        (612.0_f32, 792.0_f32)
    }

    /// Renders Two-Page Facing View (spreads side-by-side) with optional cover page (FR-01.2.2).
    fn render_two_page_facing(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let total_pages = self.active_tab().map(|t| t.doc.page_count()).unwrap_or(0);
        if total_pages == 0 {
            return;
        }

        let curr_page = self.active_tab().map(|t| t.current_page).unwrap_or(0);
        let zoom = self.active_tab().map(|t| t.zoom).unwrap_or(1.0_f32);

        // Calculate left and right pages for facing spread
        let (left_idx, right_idx) = if self.two_page_cover_page {
            if curr_page == 0 {
                (0, None)
            } else {
                let left = 1 + ((curr_page - 1) / 2) * 2;
                let right = if left + 1 < total_pages {
                    Some(left + 1)
                } else {
                    None
                };
                (left, right)
            }
        } else {
            let left = (curr_page / 2) * 2;
            let right = if left + 1 < total_pages {
                Some(left + 1)
            } else {
                None
            };
            (left, right)
        };

        let gutter = 20.0_f32;
        let (pw_l, ph_l) = self.get_page_dim(left_idx);
        let size_l = Vec2::new(pw_l * zoom, ph_l * zoom);

        let (size_r, has_right) = if let Some(r_idx) = right_idx {
            let (pw_r, ph_r) = self.get_page_dim(r_idx);
            (Vec2::new(pw_r * zoom, ph_r * zoom), true)
        } else {
            (Vec2::ZERO, false)
        };

        let total_spread_w = size_l.x
            + if has_right {
                size_r.x + gutter
            } else {
                0.0_f32
            };

        ScrollArea::both()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let avail_w = ui.available_width();
                let offset_x = ((avail_w - total_spread_w) * 0.5_f32).max(16.0_f32);
                ui.add_space(16.0_f32);

                ui.horizontal(|ui| {
                    ui.add_space(offset_x);

                    // Left Page
                    ui.vertical(|ui| {
                        ui.label(
                            RichText::new(format!("Page {}", left_idx + 1))
                                .size(11.0_f32)
                                .weak(),
                        );
                        ui.add_space(4.0_f32);
                        let (rect_l, resp_l) =
                            ui.allocate_exact_size(size_l, egui::Sense::click_and_drag());

                        ui.painter().rect_filled(
                            rect_l.translate(Vec2::new(0.0_f32, 4.0_f32)).expand(6.0_f32),
                            4.0_f32,
                            Color32::from_black_alpha(20),
                        );
                        ui.painter().rect_filled(
                            rect_l.translate(Vec2::new(0.0_f32, 2.0_f32)).expand(3.0_f32),
                            2.0_f32,
                            Color32::from_black_alpha(35),
                        );
                        ui.painter().rect_filled(rect_l, 0.0_f32, Color32::WHITE);

                        let tex_l = if let Some(tab) = self.active_tab_mut() {
                            tab.get_or_render_continuous_page(ctx, left_idx)
                        } else {
                            None
                        };
                        if let Some(tex) = tex_l {
                            ui.painter().image(
                                tex.id(),
                                rect_l,
                                Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0_f32, 1.0_f32)),
                                Color32::WHITE,
                            );
                        }

                        if resp_l.clicked() {
                            if let Some(tab) = self.active_tab_mut() {
                                tab.current_page = left_idx;
                                tab.page_input_buffer = (left_idx + 1).to_string();
                            }
                        }
                    });

                    // Right Page
                    if let Some(r_idx) = right_idx {
                        ui.add_space(gutter);
                        ui.vertical(|ui| {
                            ui.label(
                                RichText::new(format!("Page {}", r_idx + 1))
                                    .size(11.0_f32)
                                    .weak(),
                            );
                            ui.add_space(4.0_f32);
                            let (rect_r, resp_r) =
                                ui.allocate_exact_size(size_r, egui::Sense::click_and_drag());

                            ui.painter().rect_filled(
                                rect_r.translate(Vec2::new(0.0_f32, 4.0_f32)).expand(6.0_f32),
                                4.0_f32,
                                Color32::from_black_alpha(20),
                            );
                            ui.painter().rect_filled(
                                rect_r.translate(Vec2::new(0.0_f32, 2.0_f32)).expand(3.0_f32),
                                2.0_f32,
                                Color32::from_black_alpha(35),
                            );
                            ui.painter().rect_filled(rect_r, 0.0_f32, Color32::WHITE);

                            let tex_r = if let Some(tab) = self.active_tab_mut() {
                                tab.get_or_render_continuous_page(ctx, r_idx)
                            } else {
                                None
                            };
                            if let Some(tex) = tex_r {
                                ui.painter().image(
                                    tex.id(),
                                    rect_r,
                                    Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0_f32, 1.0_f32)),
                                    Color32::WHITE,
                                );
                            }

                            if resp_r.clicked() {
                                if let Some(tab) = self.active_tab_mut() {
                                    tab.current_page = r_idx;
                                    tab.page_input_buffer = (r_idx + 1).to_string();
                                }
                            }
                        });
                    }
                });

                ui.add_space(20.0_f32);
            });
    }

    /// Renders Two-Page Continuous Scroll View (FR-01.2.2).
    fn render_two_page_continuous(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let total_pages = self.active_tab().map(|t| t.doc.page_count()).unwrap_or(0);
        if total_pages == 0 {
            return;
        }

        let zoom = self.active_tab().map(|t| t.zoom).unwrap_or(1.0_f32);
        let gutter = 20.0_f32;

        let mut spreads: Vec<(usize, Option<usize>)> = Vec::new();
        if self.two_page_cover_page {
            spreads.push((0, None));
            let mut i = 1;
            while i < total_pages {
                let right = if i + 1 < total_pages {
                    Some(i + 1)
                } else {
                    None
                };
                spreads.push((i, right));
                i += 2;
            }
        } else {
            let mut i = 0;
            while i < total_pages {
                let right = if i + 1 < total_pages {
                    Some(i + 1)
                } else {
                    None
                };
                spreads.push((i, right));
                i += 2;
            }
        }

        let mut visible_in_focus = None;

        ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.add_space(12.0_f32);
                for (left_idx, right_opt) in spreads {
                    let (pw_l, ph_l) = self.get_page_dim(left_idx);
                    let size_l = Vec2::new(pw_l * zoom, ph_l * zoom);

                    let (size_r, has_right) = if let Some(r_idx) = right_opt {
                        let (pw_r, ph_r) = self.get_page_dim(r_idx);
                        (Vec2::new(pw_r * zoom, ph_r * zoom), true)
                    } else {
                        (Vec2::ZERO, false)
                    };

                    let total_spread_w = size_l.x
                        + if has_right {
                            size_r.x + gutter
                        } else {
                            0.0_f32
                        };
                    let avail_w = ui.available_width();
                    let offset_x = ((avail_w - total_spread_w) * 0.5_f32).max(16.0_f32);

                    ui.horizontal(|ui| {
                        ui.add_space(offset_x);

                        // Left Page
                        ui.vertical(|ui| {
                            ui.label(
                                RichText::new(format!("Page {}", left_idx + 1))
                                    .size(11.0_f32)
                                    .weak(),
                            );
                            ui.add_space(2.0_f32);

                            let (rect_l, resp_l) =
                                ui.allocate_exact_size(size_l, egui::Sense::click_and_drag());

                            ui.painter().rect_filled(
                                rect_l.translate(Vec2::new(0.0_f32, 4.0_f32)).expand(6.0_f32),
                                4.0_f32,
                                Color32::from_black_alpha(20),
                            );
                            ui.painter().rect_filled(
                                rect_l.translate(Vec2::new(0.0_f32, 2.0_f32)).expand(3.0_f32),
                                2.0_f32,
                                Color32::from_black_alpha(35),
                            );
                            ui.painter().rect_filled(rect_l, 0.0_f32, Color32::WHITE);

                            let tex_l = if let Some(tab) = self.active_tab_mut() {
                                tab.get_or_render_continuous_page(ctx, left_idx)
                            } else {
                                None
                            };
                            if let Some(tex) = tex_l {
                                ui.painter().image(
                                    tex.id(),
                                    rect_l,
                                    Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0_f32, 1.0_f32)),
                                    Color32::WHITE,
                                );
                            }

                            if ui.is_rect_visible(rect_l) && visible_in_focus.is_none() {
                                visible_in_focus = Some(left_idx);
                            }

                            if resp_l.clicked() {
                                if let Some(tab) = self.active_tab_mut() {
                                    tab.current_page = left_idx;
                                    tab.page_input_buffer = (left_idx + 1).to_string();
                                }
                            }
                        });

                        // Right Page
                        if let Some(r_idx) = right_opt {
                            ui.add_space(gutter);
                            ui.vertical(|ui| {
                                ui.label(
                                    RichText::new(format!("Page {}", r_idx + 1))
                                        .size(11.0_f32)
                                        .weak(),
                                );
                                ui.add_space(2.0_f32);

                                let (rect_r, resp_r) =
                                    ui.allocate_exact_size(size_r, egui::Sense::click_and_drag());

                                ui.painter().rect_filled(
                                    rect_r.translate(Vec2::new(0.0_f32, 4.0_f32)).expand(6.0_f32),
                                    4.0_f32,
                                    Color32::from_black_alpha(20),
                                );
                                ui.painter().rect_filled(
                                    rect_r.translate(Vec2::new(0.0_f32, 2.0_f32)).expand(3.0_f32),
                                    2.0_f32,
                                    Color32::from_black_alpha(35),
                                );
                                ui.painter().rect_filled(rect_r, 0.0_f32, Color32::WHITE);

                                let tex_r = if let Some(tab) = self.active_tab_mut() {
                                    tab.get_or_render_continuous_page(ctx, r_idx)
                                } else {
                                    None
                                };
                                if let Some(tex) = tex_r {
                                    ui.painter().image(
                                        tex.id(),
                                        rect_r,
                                        Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0_f32, 1.0_f32)),
                                        Color32::WHITE,
                                    );
                                }

                                if resp_r.clicked() {
                                    if let Some(tab) = self.active_tab_mut() {
                                        tab.current_page = r_idx;
                                        tab.page_input_buffer = (r_idx + 1).to_string();
                                    }
                                }
                            });
                        }
                    });

                    ui.add_space(18.0_f32);
                }
            });

        if let Some(vis) = visible_in_focus {
            if let Some(tab) = self.active_tab_mut() {
                if tab.current_page != vis {
                    tab.current_page = vis;
                    tab.page_input_buffer = (vis + 1).to_string();
                }
            }
        }
    }

    /// Renders central canvas area with PDF rendering, interactive annotations, and form widgets (EI-01.3).
    fn render_canvas(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            if self.tabs.is_empty() || self.active_tab().is_none() {
                self.render_welcome_screen(ui);
                return;
            }

            // Ensure current active tab page texture is rendered
            let soft_proof = if self.soft_proof_active {
                Some(self.soft_proof_config.clone())
            } else {
                None
            };

            if self.show_output_preview_dialog {
                let plate_cfg = self.plate_config.clone();
                let analysis = if let Some(tab) = self.active_tab_mut() {
                    tab.ensure_page_rendered_with_plate(ctx, Some(&plate_cfg), soft_proof.as_ref())
                } else {
                    None
                };
                if analysis.is_some() {
                    self.separation_analysis = analysis;
                }
            } else if let Some(tab) = self.active_tab_mut() {
                let _ = tab.ensure_page_rendered_with_plate(ctx, None, soft_proof.as_ref());
            }

            // macOS Trackpad Pinch-to-Zoom & Cmd/Ctrl+Scroll Zoom (FR-01.3.1, NFR-05, NFR-06)
            let zoom_delta = ctx.input(|i| i.zoom_delta());
            let cmd_or_ctrl = ctx.input(|i| i.modifiers.command || i.modifiers.ctrl);
            let scroll_delta_y = ctx.input(|i| i.raw_scroll_delta.y);

            let mut applied_zoom_factor = 1.0_f32;
            if (zoom_delta - 1.0).abs() > 0.002 {
                applied_zoom_factor = zoom_delta;
            } else if cmd_or_ctrl && scroll_delta_y.abs() > 0.0 {
                applied_zoom_factor = 1.0 + (scroll_delta_y * 0.005).clamp(-0.4, 0.4);
            }

            if (applied_zoom_factor - 1.0).abs() > 0.002
                && self.page_view_mode != PageViewMode::OrganizePages
            {
                if let Some(tab) = self.active_tab_mut() {
                    tab.target_zoom = (tab.target_zoom * applied_zoom_factor).clamp(0.1, 64.0);
                    ctx.request_repaint();
                }
            }

            // Smooth animated pinch zoom interpolation (critically damped exponential spring)
            if let Some(tab) = self.active_tab_mut() {
                let diff = tab.target_zoom - tab.zoom;
                if diff.abs() > 0.0005 {
                    let dt = ctx.input(|i| i.stable_dt).min(0.1);
                    let lerp_factor = 1.0 - (-18.0 * dt).exp();
                    tab.zoom += diff * lerp_factor;
                    if (tab.target_zoom - tab.zoom).abs() < 0.001 {
                        tab.zoom = tab.target_zoom;
                        tab.invalidate_render();
                    }
                    ctx.request_repaint();
                }
            }

            // View Mode check: Organize Pages Visual Grid Mode (FR-06.1 - FR-06.11)
            if self.page_view_mode == PageViewMode::OrganizePages {
                self.render_organize_pages_grid(ui, ctx);
                return;
            }

            // View Mode check: Split View Mode (FR-01.3.5)
            if self.split_view_enabled {
                self.render_split_view(ui, ctx);
                return;
            }

            // View Mode check: Two-Page Facing View (FR-01.2.2)
            if self.page_view_mode == PageViewMode::TwoPageFacing {
                self.render_two_page_facing(ui, ctx);
                return;
            }

            // View Mode check: Two-Page Continuous Scroll Mode (FR-01.2.2)
            if self.page_view_mode == PageViewMode::TwoPageContinuous {
                self.render_two_page_continuous(ui, ctx);
                return;
            }

            // View Mode check: Continuous Scroll Mode (FR-01.2.2)
            if self.page_view_mode == PageViewMode::ContinuousScroll {
                let total_pages = self.active_tab().map(|t| t.doc.page_count()).unwrap_or(0);
                let zoom = self.active_tab().map(|t| t.zoom).unwrap_or(1.0);
                let mut visible_in_focus = None;

                ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        for page_idx in 0..total_pages {
                            let (pw, ph) = if let Some(tab) = self.active_tab() {
                                if let Some(p) = tab.doc.get_page(page_idx) {
                                    let w = (p.crop_box[2] - p.crop_box[0]).abs() as f32;
                                    let h = (p.crop_box[3] - p.crop_box[1]).abs() as f32;
                                    (w, h)
                                } else {
                                    (612.0, 792.0)
                                }
                            } else {
                                (612.0, 792.0)
                            };

                            let page_pixel_size = Vec2::new(pw * zoom, ph * zoom);

                            ui.vertical_centered(|ui| {
                                ui.add_space(8.0);
                                ui.label(RichText::new(format!("Page {} of {}", page_idx + 1, total_pages)).size(11.0).weak());
                                ui.add_space(2.0);

                                let (rect, response) = ui.allocate_exact_size(page_pixel_size, egui::Sense::click_and_drag());

                                ui.painter().rect_filled(rect.expand(2.0), 0.0, Color32::from_black_alpha(40));
                                ui.painter().rect_filled(rect, 0.0, Color32::WHITE);

                                let tex_opt = if let Some(tab) = self.active_tab_mut() {
                                    tab.get_or_render_continuous_page(ctx, page_idx)
                                } else {
                                    None
                                };

                                if let Some(tex) = tex_opt {
                                    ui.painter().image(
                                        tex.id(),
                                        rect,
                                        Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                                        Color32::WHITE,
                                    );
                                }

                                if ui.is_rect_visible(rect) && visible_in_focus.is_none() {
                                    visible_in_focus = Some(page_idx);
                                }

                                if response.clicked() {
                                    if let Some(pos) = response.interact_pointer_pos() {
                                        let local_x = ((pos.x - rect.min.x) / zoom) as f64;
                                        let local_y = ((rect.max.y - pos.y) / zoom) as f64;
                                        match self.active_tool {
                                            ActiveTool::StickyNote => {
                                                if let Some(tab) = self.active_tab_mut() {
                                                    let annot = Annotation::new(
                                                        AnnotationType::Text,
                                                        [local_x, local_y, local_x + 30.0, local_y + 30.0],
                                                    );
                                                    let _ = add_annotation_to_page(&mut tab.doc, page_idx, &annot);
                                                    tab.is_modified = true;
                                                    tab.invalidate_render();
                                                    self.status_message = format!("Added sticky note to page {}", page_idx + 1);
                                                }
                                            }
                                            ActiveTool::Text => {
                                                self.free_text_coords = Some([local_x, local_y, local_x + 120.0, local_y + 24.0]);
                                                self.free_text_content.clear();
                                                self.show_free_text_dialog = true;
                                            }
                                            ActiveTool::Typewriter => {
                                                self.typewriter_pos_x = local_x;
                                                self.typewriter_pos_y = local_y;
                                                self.show_typewriter_dialog = true;
                                            }
                                            ActiveTool::Callout => {
                                                self.callout_box_x = local_x;
                                                self.callout_box_y = local_y;
                                                self.callout_box_w = 160.0;
                                                self.callout_box_h = 40.0;
                                                self.callout_leader_x = local_x - 50.0;
                                                self.callout_leader_y = local_y - 40.0;
                                                self.callout_knee_x = local_x - 15.0;
                                                self.callout_knee_y = local_y - 10.0;
                                                self.show_callout_dialog = true;
                                            }
                                            _ => {}
                                        }
                                    }
                                }

                                ui.add_space(14.0);
                            });
                        }
                    });

                if let Some(vis) = visible_in_focus {
                    if let Some(tab) = self.active_tab_mut() {
                        if tab.current_page != vis {
                            tab.current_page = vis;
                            tab.page_input_buffer = (vis + 1).to_string();
                        }
                    }
                }
                return;
            }

            // Read properties of current active page
            let (page_size, page_h, zoom) = {
                let tab = self.active_tab().unwrap();
                let page = tab.doc.get_page(tab.current_page);
                let page_h = page
                    .as_ref()
                    .map(|p| (p.crop_box[3] - p.crop_box[1]).abs() as f32)
                    .unwrap_or(792.0);
                let page_w = page
                    .as_ref()
                    .map(|p| (p.crop_box[2] - p.crop_box[0]).abs() as f32)
                    .unwrap_or(612.0);
                let tex_size = tab
                    .page_texture
                    .as_ref()
                    .map(|t| t.size_vec2() / ctx.pixels_per_point())
                    .unwrap_or_else(|| Vec2::new(page_w * tab.zoom, page_h * tab.zoom));
                (tex_size, page_h, tab.zoom)
            };

            ScrollArea::both()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    if page_size.x <= 0.0 || page_size.y <= 0.0 {
                        return;
                    }

                    let avail_w = ui.available_width();
                    let offset_x = ((avail_w - page_size.x) * 0.5_f32).max(16.0_f32);
                    ui.add_space(16.0_f32);

                    let (rect, response) = ui
                        .horizontal(|ui| {
                            ui.add_space(offset_x);
                            ui.allocate_exact_size(page_size, egui::Sense::click_and_drag())
                        })
                        .inner;

                    // 1. Draw page background and soft drop shadow
                    ui.painter().rect_filled(
                        rect.translate(Vec2::new(0.0_f32, 4.0_f32)).expand(6.0_f32),
                        4.0_f32,
                        Color32::from_black_alpha(20),
                    );
                    ui.painter().rect_filled(
                        rect.translate(Vec2::new(0.0_f32, 2.0_f32)).expand(3.0_f32),
                        2.0_f32,
                        Color32::from_black_alpha(35),
                    );
                    ui.painter().rect_filled(rect, 0.0_f32, Color32::WHITE);

                    // 2. Paint raster page texture
                    if let Some(tab) = self.active_tab() {
                        if let Some(ref tex) = tab.page_texture {
                            ui.painter().image(
                                tex.id(),
                                rect,
                                Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                                Color32::WHITE,
                            );
                        }
                    }

                    // 3. Interactive Form Fields Overlay (FR-11.2)
                    let current_page = self.active_tab().map(|t| t.current_page).unwrap_or(0);
                    let mut field_updates: Vec<(ravpdf_core::object::ObjectId, String)> = Vec::new();

                    if let Some(tab) = self.active_tab() {
                        if let Ok(fields) = get_page_form_fields(&tab.doc, current_page) {
                            for field in fields {
                                if let Some(field_id) = field.id {
                                    // Convert PDF coordinates (origin bottom-left) to canvas screen space
                                    let x_min = rect.min.x + (field.rect[0] as f32) * zoom;
                                    let x_max = rect.min.x + (field.rect[2] as f32) * zoom;
                                    let y_min = rect.min.y + (page_h - field.rect[3] as f32) * zoom;
                                    let y_max = rect.min.y + (page_h - field.rect[1] as f32) * zoom;

                                    let field_rect = Rect::from_min_max(
                                        Pos2::new(x_min, y_min),
                                        Pos2::new(x_max, y_max),
                                    );

                                    match field.field_type {
                                        FormFieldType::Text => {
                                            let current_text = self
                                                .form_field_text_buffers
                                                .entry(field_id)
                                                .or_insert_with(|| {
                                                    field.value.clone().unwrap_or_default()
                                                });

                                            ui.allocate_new_ui(
                                                egui::UiBuilder::new().max_rect(field_rect),
                                                |ui| {
                                                    egui::Frame::none()
                                                        .fill(Color32::from_rgba_unmultiplied(225, 235, 255, 180))
                                                        .stroke(Stroke::new(1.0_f32, Color32::from_rgb(66, 153, 225)))
                                                        .show(ui, |ui| {
                                                            let edit = ui.add_sized(
                                                                field_rect.size(),
                                                                egui::TextEdit::singleline(current_text),
                                                            );
                                                            if edit.changed() {
                                                                field_updates.push((field_id, current_text.clone()));
                                                            }
                                                        });
                                                },
                                            );
                                        }
                                        FormFieldType::Checkbox => {
                                            let is_checked = field.value.as_deref() == Some("Yes");
                                            let mut val = is_checked;
                                            ui.allocate_new_ui(
                                                egui::UiBuilder::new().max_rect(field_rect),
                                                |ui| {
                                                    egui::Frame::none()
                                                        .fill(Color32::from_rgba_unmultiplied(240, 245, 255, 200))
                                                        .stroke(Stroke::new(1.0_f32, Color32::from_rgb(66, 153, 225)))
                                                        .show(ui, |ui| {
                                                            if ui.checkbox(&mut val, "").changed() {
                                                                let new_val = if val { "Yes" } else { "Off" };
                                                                field_updates.push((field_id, new_val.into()));
                                                            }
                                                        });
                                                },
                                            );
                                        }
                                        FormFieldType::Choice => {
                                            let mut selected = field.value.clone().unwrap_or_default();
                                            ui.allocate_new_ui(
                                                egui::UiBuilder::new().max_rect(field_rect),
                                                |ui| {
                                                    egui::Frame::none()
                                                        .fill(Color32::from_rgba_unmultiplied(240, 245, 255, 220))
                                                        .stroke(Stroke::new(1.0_f32, Color32::from_rgb(66, 153, 225)))
                                                        .show(ui, |ui| {
                                                            egui::ComboBox::from_id_salt(format!("choice_{}_{}", field_id.number, field_id.generation))
                                                                .selected_text(&selected)
                                                                .show_ui(ui, |ui| {
                                                                    for opt in &field.options {
                                                                        if ui.selectable_value(&mut selected, opt.clone(), opt).clicked() {
                                                                            field_updates.push((field_id, opt.clone()));
                                                                        }
                                                                    }
                                                                });
                                                        });
                                                },
                                            );
                                        }
                                        FormFieldType::PushButton => {
                                            let label = field.value.as_deref().unwrap_or(&field.name);
                                            ui.allocate_new_ui(
                                                egui::UiBuilder::new().max_rect(field_rect),
                                                |ui| {
                                                    if ui.add_sized(field_rect.size(), egui::Button::new(label)).clicked() {
                                                        // Push button clicked feedback
                                                    }
                                                },
                                            );
                                        }
                                        FormFieldType::Radio => {
                                            let export_val = field.radio_export_value.clone().unwrap_or_else(|| "Yes".into());
                                            let is_selected = field.value.as_deref() == Some(&export_val);
                                            let val = is_selected;
                                            ui.allocate_new_ui(
                                                egui::UiBuilder::new().max_rect(field_rect),
                                                |ui| {
                                                    egui::Frame::none()
                                                        .fill(Color32::from_rgba_unmultiplied(240, 245, 255, 200))
                                                        .stroke(Stroke::new(1.0_f32, Color32::from_rgb(66, 153, 225)))
                                                        .show(ui, |ui| {
                                                            if ui.radio(val, "").clicked() {
                                                                let new_val = if !val { export_val } else { "Off".into() };
                                                                field_updates.push((field_id, new_val));
                                                            }
                                                        });
                                                },
                                            );
                                        }
                                        FormFieldType::ListBox => {
                                            let mut selected = field.value.clone().unwrap_or_default();
                                            ui.allocate_new_ui(
                                                egui::UiBuilder::new().max_rect(field_rect),
                                                |ui| {
                                                    egui::Frame::none()
                                                        .fill(Color32::from_rgba_unmultiplied(240, 245, 255, 220))
                                                        .stroke(Stroke::new(1.0_f32, Color32::from_rgb(66, 153, 225)))
                                                        .show(ui, |ui| {
                                                            ScrollArea::vertical().show(ui, |ui| {
                                                                for opt in &field.options {
                                                                    if ui.selectable_label(selected == *opt, opt).clicked() {
                                                                        selected = opt.clone();
                                                                        field_updates.push((field_id, opt.clone()));
                                                                    }
                                                                }
                                                            });
                                                        });
                                                },
                                            );
                                        }
                                        FormFieldType::Barcode => {
                                            let barcode_str = field.value.as_deref().unwrap_or("[Barcode]");
                                            ui.allocate_new_ui(
                                                egui::UiBuilder::new().max_rect(field_rect),
                                                |ui| {
                                                    egui::Frame::none()
                                                        .fill(Color32::from_rgba_unmultiplied(255, 255, 255, 230))
                                                        .stroke(Stroke::new(1.0_f32, Color32::from_rgb(40, 40, 40)))
                                                        .show(ui, |ui| {
                                                            ui.vertical_centered(|ui| {
                                                                ui.label(RichText::new("📊 [QR-Code]").size(11.0).strong());
                                                                ui.label(RichText::new(barcode_str).size(9.0).weak());
                                                            });
                                                        });
                                                },
                                            );
                                        }
                                        FormFieldType::Signature => {
                                            let mut trigger_sign = false;
                                            ui.allocate_new_ui(
                                                egui::UiBuilder::new().max_rect(field_rect),
                                                |ui| {
                                                    let resp = egui::Frame::none()
                                                        .fill(Color32::from_rgba_unmultiplied(255, 248, 220, 200))
                                                        .stroke(Stroke::new(1.0_f32, Color32::from_rgb(218, 165, 32)))
                                                        .show(ui, |ui| {
                                                            ui.horizontal(|ui| {
                                                                ui.label(RichText::new("🔏").size(12.0));
                                                                ui.label(RichText::new("Click to Digitally Sign").size(10.0).italics());
                                                            });
                                                        }).response.interact(egui::Sense::click());
                                                    if resp.clicked() {
                                                        trigger_sign = true;
                                                    }
                                                },
                                            );
                                            if trigger_sign {
                                                self.sig_target_page = current_page;
                                                self.sig_field_name = field.name.clone();
                                                self.sig_mode_idx = 0;
                                                self.show_add_signature_dialog = true;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    // 3b. Interactive Hyperlinks Overlay (FR-20.1, FR-20.2)
                    let mut jump_target_page: Option<usize> = None;
                    if let Some(tab) = self.active_tab() {
                        if let Ok(annots) = get_page_annotations(&tab.doc, current_page) {
                            for annot in annots {
                                if let AnnotationType::Link { ref action, border_width, .. } = annot.annot_type {
                                    let x_min = rect.min.x + (annot.rect[0] as f32) * zoom;
                                    let x_max = rect.min.x + (annot.rect[2] as f32) * zoom;
                                    let y_min = rect.min.y + (page_h - annot.rect[3] as f32) * zoom;
                                    let y_max = rect.min.y + (page_h - annot.rect[1] as f32) * zoom;
                                    let link_rect = Rect::from_min_max(Pos2::new(x_min, y_min), Pos2::new(x_max, y_max));

                                    if border_width > 0.0 {
                                        ui.painter().rect_stroke(
                                            link_rect,
                                            0.0,
                                            Stroke::new((border_width as f32 * zoom).max(1.0), Color32::from_rgb(0, 102, 204)),
                                        );
                                    }

                                    let mut resp = ui.allocate_rect(link_rect, egui::Sense::click());
                                    match action {
                                        LinkAction::Uri(ref u) => {
                                            resp = resp.on_hover_ui(|ui| {
                                                ui.label(format!("🔗 Open: {}", u));
                                            });
                                            if resp.clicked() {
                                                ctx.open_url(egui::OpenUrl::same_tab(u));
                                            }
                                        }
                                        LinkAction::GoToPage(tp) => {
                                            let target = *tp;
                                            resp = resp.on_hover_ui(|ui| {
                                                ui.label(format!("➔ Jump to Page {}", target + 1));
                                            });
                                            if resp.clicked() {
                                                jump_target_page = Some(target);
                                            }
                                        }
                                        LinkAction::Named(ref n) => {
                                            resp = resp.on_hover_ui(|ui| {
                                                ui.label(format!("➔ Destination: {}", n));
                                            });
                                            if resp.clicked() {
                                                if let Some(tab) = self.active_tab() {
                                                    if let Ok(dests) = get_named_destinations(&tab.doc) {
                                                        if let Some(d) = dests.iter().find(|d| d.name == *n) {
                                                            jump_target_page = Some(d.page_index);
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                        LinkAction::OpenFile(ref path) => {
                                            resp = resp.on_hover_ui(|ui| {
                                                ui.label(format!("📂 Open File: {}", path));
                                            });
                                            if resp.clicked() {
                                                self.status_message = format!("Launch action: '{}'", path);
                                            }
                                        }
                                        LinkAction::JavaScript(ref code) => {
                                            resp = resp.on_hover_ui(|ui| {
                                                ui.label("⚡ Execute JavaScript");
                                            });
                                            if resp.clicked() {
                                                let mut ctx_exec = JsExecutionContext::default();
                                                let _ = execute_script(&mut ctx_exec, code);
                                                self.status_message = "Executed link JavaScript".into();
                                            }
                                        }
                                        LinkAction::SubmitForm { ref url, .. } => {
                                            resp = resp.on_hover_ui(|ui| {
                                                ui.label(format!("📨 Submit Form to: {}", url));
                                            });
                                            if resp.clicked() {
                                                self.status_message = format!("Submit form to: {}", url);
                                            }
                                        }
                                        LinkAction::ResetForm { .. } => {
                                            resp = resp.on_hover_ui(|ui| {
                                                ui.label("↺ Reset Form Fields");
                                            });
                                            if resp.clicked() {
                                                self.status_message = "Reset form fields".into();
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    if let Some(target) = jump_target_page {
                        if let Some(tab) = self.active_tab_mut() {
                            if target < tab.doc.page_count() {
                                tab.current_page = target;
                                tab.page_input_buffer = (target + 1).to_string();
                                tab.invalidate_render();
                            }
                        }
                    }

                    // Apply any interactive form updates back to document (FR-11.2)
                    if !field_updates.is_empty() {
                        if let Some(tab) = self.active_tab_mut() {
                            for (fid, val) in field_updates {
                                let _ = set_form_field_value(&mut tab.doc, fid, &val);
                            }
                            tab.is_modified = true;
                            tab.invalidate_render();
                        }
                    }

                    // Selected Annotation Bounding Box & 8 Handles (FR-08.3, FR-09.12)
                    if let Some(ref sel) = self.selected_annotation {
                        if sel.page_index == current_page {
                            let ax1 = rect.min.x + (sel.rect[0] as f32) * zoom;
                            let ax2 = rect.min.x + (sel.rect[2] as f32) * zoom;
                            let ay1 = rect.min.y + (page_h - sel.rect[3] as f32) * zoom;
                            let ay2 = rect.min.y + (page_h - sel.rect[1] as f32) * zoom;
                            let box_rect = Rect::from_min_max(Pos2::new(ax1.min(ax2), ay1.min(ay2)), Pos2::new(ax1.max(ax2), ay1.max(ay2))).expand(2.0);
                            ui.painter().rect_stroke(box_rect, 2.0, Stroke::new(1.8_f32, Color32::from_rgb(59, 130, 246)));

                            for pt in [
                                box_rect.left_top(), box_rect.center_top(), box_rect.right_top(),
                                box_rect.left_center(), box_rect.right_center(),
                                box_rect.left_bottom(), box_rect.center_bottom(), box_rect.right_bottom(),
                            ] {
                                ui.painter().rect_filled(Rect::from_center_size(pt, Vec2::splat(6.0)), 1.0, Color32::WHITE);
                                ui.painter().rect_stroke(Rect::from_center_size(pt, Vec2::splat(6.0)), 1.0, Stroke::new(1.0_f32, Color32::from_rgb(59, 130, 246)));
                            }
                        }
                    }

                    // Selected Form Field Bounding Box & 8 Handles (FR-11.1 - FR-11.3)
                    if let Some(ref sel_field) = self.selected_form_field {
                        if !self.form_test_mode && sel_field.page_index == current_page {
                            let fx1 = rect.min.x + (sel_field.rect[0] as f32) * zoom;
                            let fx2 = rect.min.x + (sel_field.rect[2] as f32) * zoom;
                            let fy1 = rect.min.y + (page_h - sel_field.rect[3] as f32) * zoom;
                            let fy2 = rect.min.y + (page_h - sel_field.rect[1] as f32) * zoom;
                            let box_rect = Rect::from_min_max(Pos2::new(fx1.min(fx2), fy1.min(fy2)), Pos2::new(fx1.max(fx2), fy1.max(fy2))).expand(2.0);
                            ui.painter().rect_stroke(box_rect, 2.0, Stroke::new(2.0_f32, Color32::from_rgb(139, 92, 246)));

                            // Badge
                            let badge_w = (sel_field.name.len() as f32 * 7.5 + 24.0).clamp(60.0, 160.0);
                            let badge_rect = Rect::from_min_size(Pos2::new(box_rect.min.x, (box_rect.min.y - 18.0).max(rect.min.y)), Vec2::new(badge_w, 16.0));
                            ui.painter().rect_filled(badge_rect, 2.0, Color32::from_rgb(109, 40, 217));
                            ui.painter().text(badge_rect.center(), Align2::CENTER_CENTER, format!("📝 {}", sel_field.name), FontId::proportional(10.0), Color32::WHITE);

                            for pt in [
                                box_rect.left_top(), box_rect.center_top(), box_rect.right_top(),
                                box_rect.left_center(), box_rect.right_center(),
                                box_rect.left_bottom(), box_rect.center_bottom(), box_rect.right_bottom(),
                            ] {
                                ui.painter().rect_filled(Rect::from_center_size(pt, Vec2::splat(6.0)), 1.0, Color32::WHITE);
                                ui.painter().rect_stroke(Rect::from_center_size(pt, Vec2::splat(6.0)), 1.0, Stroke::new(1.2_f32, Color32::from_rgb(139, 92, 246)));
                            }
                        }
                    }

                    // Marked Redactions Visual Overlay (FR-15.1, FR-15.6)
                    let mut remove_redaction_id = None;
                    for mr in self.marked_redactions.iter().filter(|r| r.page_index == current_page) {
                        let rx1 = rect.min.x + (mr.rect[0] as f32) * zoom;
                        let rx2 = rect.min.x + (mr.rect[2] as f32) * zoom;
                        let ry1 = rect.min.y + (page_h - mr.rect[3] as f32) * zoom;
                        let ry2 = rect.min.y + (page_h - mr.rect[1] as f32) * zoom;
                        let r_box = Rect::from_min_max(Pos2::new(rx1.min(rx2), ry1.min(ry2)), Pos2::new(rx1.max(rx2), ry1.max(ry2)));

                        ui.painter().rect_filled(r_box, 0.0, Color32::from_rgba_unmultiplied(220, 38, 38, 90));
                        ui.painter().rect_stroke(r_box, 0.0, Stroke::new(2.0_f32, Color32::from_rgb(220, 38, 38)));
                        let text = mr.appearance.overlay_text.as_deref().unwrap_or("[REDACTED]");
                        ui.painter().text(r_box.center(), Align2::CENTER_CENTER, text, FontId::proportional(12.0), Color32::WHITE);

                        // Small close badge
                        let btn_rect = Rect::from_center_size(r_box.right_top() + Vec2::new(-8.0, 8.0), Vec2::splat(14.0));
                        ui.painter().rect_filled(btn_rect, 7.0, Color32::from_rgb(185, 28, 28));
                        ui.painter().text(btn_rect.center(), Align2::CENTER_CENTER, "✕", FontId::proportional(9.0), Color32::WHITE);
                        if ui.allocate_rect(btn_rect, egui::Sense::click()).clicked() {
                            remove_redaction_id = Some(mr.id);
                        }
                    }
                    if let Some(rm_id) = remove_redaction_id {
                        self.marked_redactions.retain(|r| r.id != rm_id);
                    }

                    // Text Selection Overlay & Floating Copy Pill (FR-01.1.2, FR-04.6, FR-04.7)
                    for span in &self.text_selection.selected_spans {
                        let span_w = (span.text.len() as f64 * span.font_size * 0.55 * zoom as f64) as f32;
                        let span_h = (span.font_size * 1.2 * zoom as f64) as f32;
                        let span_min = Pos2::new(rect.min.x + span.x as f32 * zoom, rect.min.y + (page_h - span.y as f32 - span.font_size as f32) * zoom);
                        let span_rect = Rect::from_min_size(span_min, Vec2::new(span_w, span_h));
                        ui.painter().rect_filled(span_rect, 1.0, Color32::from_rgba_unmultiplied(66, 133, 244, 90));
                    }
                    if !self.text_selection.selected_text.is_empty() {
                        if let Some(pos) = self.text_selection.current_pos {
                            let pill_w = 360.0_f32;
                            let pill_h = 28.0_f32;
                            let pill_x = pos.x.clamp(rect.min.x, (rect.max.x - pill_w).max(rect.min.x));
                            let pill_y = (pos.y - 34.0).max(rect.min.y);
                            let pill_rect = Rect::from_min_size(Pos2::new(pill_x, pill_y), Vec2::new(pill_w, pill_h));

                            ui.allocate_new_ui(egui::UiBuilder::new().max_rect(pill_rect), |ui| {
                                egui::Frame::none()
                                    .fill(Color32::from_rgba_unmultiplied(24, 24, 27, 245))
                                    .rounding(6.0_f32)
                                    .stroke(Stroke::new(1.0_f32, Color32::from_rgb(63, 63, 70)))
                                    .inner_margin(egui::Margin::symmetric(6.0, 4.0))
                                    .show(ui, |ui| {
                                        ui.horizontal(|ui| {
                                            ui.spacing_mut().item_spacing = Vec2::new(4.0, 0.0);
                                            if ui.small_button("📋 Copy").on_hover_text("Copy selected text (Ctrl+C)").clicked() {
                                                ctx.output_mut(|o| o.copied_text = self.text_selection.selected_text.clone());
                                                self.status_message = format!("Copied {} chars to clipboard", self.text_selection.selected_text.len());
                                            }
                                            if ui.small_button("🖍 Highlight").on_hover_text("Highlight selected text").clicked() {
                                                let mut min_x = f64::MAX;
                                                let mut min_y = f64::MAX;
                                                let mut max_x = f64::MIN;
                                                let mut max_y = f64::MIN;
                                                for span in &self.text_selection.selected_spans {
                                                    let span_w = span.text.len() as f64 * span.font_size * 0.55;
                                                    let span_h = span.font_size * 1.2;
                                                    min_x = min_x.min(span.x);
                                                    min_y = min_y.min(span.y);
                                                    max_x = max_x.max(span.x + span_w);
                                                    max_y = max_y.max(span.y + span_h);
                                                }
                                                if min_x < max_x && min_y < max_y {
                                                    if let Some(tab) = self.active_tab_mut() {
                                                        let annot = Annotation::new(AnnotationType::Highlight, [min_x, min_y, max_x, max_y]);
                                                        let curr = tab.current_page;
                                                        let _ = add_annotation_to_page(&mut tab.doc, curr, &annot);
                                                        tab.is_modified = true;
                                                        tab.invalidate_render();
                                                        self.status_message = "Highlighted selected text".into();
                                                    }
                                                }
                                            }
                                            if ui.small_button("U̲ Underline").on_hover_text("Underline selected text").clicked() {
                                                let mut min_x = f64::MAX;
                                                let mut min_y = f64::MAX;
                                                let mut max_x = f64::MIN;
                                                let mut max_y = f64::MIN;
                                                for span in &self.text_selection.selected_spans {
                                                    let span_w = span.text.len() as f64 * span.font_size * 0.55;
                                                    let span_h = span.font_size * 1.2;
                                                    min_x = min_x.min(span.x);
                                                    min_y = min_y.min(span.y);
                                                    max_x = max_x.max(span.x + span_w);
                                                    max_y = max_y.max(span.y + span_h);
                                                }
                                                if min_x < max_x && min_y < max_y {
                                                    if let Some(tab) = self.active_tab_mut() {
                                                        let annot = Annotation::new(AnnotationType::Underline, [min_x, min_y, max_x, max_y]);
                                                        let curr = tab.current_page;
                                                        let _ = add_annotation_to_page(&mut tab.doc, curr, &annot);
                                                        tab.is_modified = true;
                                                        tab.invalidate_render();
                                                        self.status_message = "Underlined selected text".into();
                                                    }
                                                }
                                            }
                                            if ui.small_button("█ Redact").on_hover_text("Mark selected text for redaction").clicked() {
                                                let mut min_x = f64::MAX;
                                                let mut min_y = f64::MAX;
                                                let mut max_x = f64::MIN;
                                                let mut max_y = f64::MIN;
                                                for span in &self.text_selection.selected_spans {
                                                    let span_w = span.text.len() as f64 * span.font_size * 0.55;
                                                    let span_h = span.font_size * 1.2;
                                                    min_x = min_x.min(span.x);
                                                    min_y = min_y.min(span.y);
                                                    max_x = max_x.max(span.x + span_w);
                                                    max_y = max_y.max(span.y + span_h);
                                                }
                                                if min_x < max_x && min_y < max_y {
                                                    let r_id = self.next_redaction_id;
                                                    self.next_redaction_id += 1;
                                                    self.marked_redactions.push(MarkedRedaction {
                                                        id: r_id,
                                                        page_index: current_page,
                                                        rect: [min_x, min_y, max_x, max_y],
                                                        appearance: RedactionAppearance {
                                                            fill_color: [0.0, 0.0, 0.0],
                                                            overlay_text: Some("[REDACTED]".into()),
                                                            text_color: [1.0, 1.0, 1.0],
                                                            font_size: None,
                                                        },
                                                    });
                                                    self.status_message = format!("Marked selection for redaction (Total marked: {})", self.marked_redactions.len());
                                                }
                                            }
                                            if ui.small_button("✏ Edit").on_hover_text("Edit text in-place").clicked() {
                                                self.text_editor_target_text = self.text_selection.selected_text.clone();
                                                self.text_editor_new_text = self.text_selection.selected_text.clone();
                                                self.show_text_editor_dialog = true;
                                            }
                                            if ui.small_button("✖").on_hover_text("Clear selection").clicked() {
                                                self.text_selection.selected_text.clear();
                                                self.text_selection.selected_spans.clear();
                                            }
                                        });
                                    });
                            });
                        }
                    }

                    // Canvas Keyboard Shortcuts (Ctrl+C, Ctrl+A, Delete/Backspace)
                    if ctx.input(|i| i.modifiers.command && i.key_pressed(Key::C)) && !self.text_selection.selected_text.is_empty() {
                        ctx.output_mut(|o| o.copied_text = self.text_selection.selected_text.clone());
                        self.status_message = format!("Copied {} chars to clipboard", self.text_selection.selected_text.len());
                    }

                    if self.active_tool == ActiveTool::Select && ctx.input(|i| i.modifiers.command && i.key_pressed(Key::A)) {
                        if let Some(tab) = self.active_tab() {
                            let spans = extract_page_text_spans(&tab.doc, current_page).unwrap_or_default();
                            self.text_selection.selected_text = spans.iter().map(|s| s.text.as_str()).collect::<Vec<_>>().join(" ");
                            self.text_selection.selected_spans = spans;
                            self.status_message = format!("Selected all text ({} chars)", self.text_selection.selected_text.len());
                        }
                    }

                    if self.active_tool == ActiveTool::Select && (ctx.input(|i| i.key_pressed(Key::Delete)) || ctx.input(|i| i.key_pressed(Key::Backspace))) {
                        if let Some(sel) = self.selected_annotation.take() {
                            if let Some(tab) = self.active_tab_mut() {
                                let _ = delete_annotation(&mut tab.doc, sel.page_index, sel.id);
                                tab.is_modified = true;
                                tab.invalidate_render();
                                self.status_message = "Deleted annotation".into();
                            }
                        } else if let Some(sel_field) = self.selected_form_field.take() {
                            if let Some(tab) = self.active_tab_mut() {
                                let _ = delete_form_field(&mut tab.doc, sel_field.id);
                                tab.is_modified = true;
                                tab.invalidate_render();
                                self.status_message = format!("Deleted form field '{}'", sel_field.name);
                            }
                            self.show_form_field_inspector = false;
                        }
                    }

                    // 4. Annotation Drag-Drawing and Click Handlers (FR-08, FR-09)
                    if response.drag_started() {
                        if let Some(pos) = response.interact_pointer_pos() {
                            self.drag_start_pos = Some(pos);
                            self.drag_current_pos = Some(pos);
                            if self.active_tool == ActiveTool::MarqueeZoom {
                                self.marquee_zoom_start = Some(pos);
                            }
                            if self.active_tool == ActiveTool::Pencil {
                                self.active_pencil_points.clear();
                                self.active_pencil_points.push(pos);
                            } else if self.active_tool == ActiveTool::Select {
                                let mut hit_annot = None;
                                if let Some(tab) = self.active_tab() {
                                    if let Ok(annots) = get_page_annotations(&tab.doc, current_page) {
                                        for a in annots {
                                            let ax1 = rect.min.x + (a.rect[0] as f32) * zoom;
                                            let ax2 = rect.min.x + (a.rect[2] as f32) * zoom;
                                            let ay1 = rect.min.y + (page_h - a.rect[3] as f32) * zoom;
                                            let ay2 = rect.min.y + (page_h - a.rect[1] as f32) * zoom;
                                            let a_rect = Rect::from_min_max(Pos2::new(ax1.min(ax2), ay1.min(ay2)), Pos2::new(ax1.max(ax2), ay1.max(ay2))).expand(4.0);
                                            if a_rect.contains(pos) {
                                                hit_annot = Some(a);
                                                break;
                                            }
                                        }
                                    }
                                }

                                let mut hit_field = None;
                                if !self.form_test_mode && hit_annot.is_none() {
                                    if let Some(tab) = self.active_tab() {
                                        if let Ok(fields) = get_page_form_fields(&tab.doc, current_page) {
                                            for f in fields {
                                                let fx1 = rect.min.x + (f.rect[0] as f32) * zoom;
                                                let fx2 = rect.min.x + (f.rect[2] as f32) * zoom;
                                                let fy1 = rect.min.y + (page_h - f.rect[3] as f32) * zoom;
                                                let fy2 = rect.min.y + (page_h - f.rect[1] as f32) * zoom;
                                                let f_rect = Rect::from_min_max(Pos2::new(fx1.min(fx2), fy1.min(fy2)), Pos2::new(fx1.max(fx2), fy1.max(fy2))).expand(4.0);
                                                if f_rect.contains(pos) {
                                                    hit_field = Some(f);
                                                    break;
                                                }
                                            }
                                        }
                                    }
                                }

                                if let Some(a) = hit_annot {
                                    let a_id = a.id.unwrap_or(ObjectId { number: 0, generation: 0 });
                                    self.selected_annotation = Some(SelectedAnnotation {
                                        id: a_id,
                                        page_index: current_page,
                                        annot_type: a.annot_type.clone(),
                                        rect: a.rect,
                                        contents: a.contents.clone(),
                                        author: a.author.clone(),
                                        color: a.color,
                                        opacity: a.opacity,
                                        is_dragging: true,
                                        drag_start_rect: a.rect,
                                        drag_start_pointer: pos,
                                    });
                                    self.annot_inspector_contents = a.contents.unwrap_or_default();
                                    self.annot_inspector_author = a.author.unwrap_or_else(|| "User".into());
                                    if let Some(c) = a.color {
                                        self.annot_inspector_color = [c.0, c.1, c.2];
                                    }
                                    self.annot_inspector_opacity = a.opacity;
                                    self.show_annot_inspector = true;
                                    self.selected_form_field = None;
                                    self.show_form_field_inspector = false;
                                    self.text_selection.selected_text.clear();
                                    self.text_selection.selected_spans.clear();
                                } else if let Some(f) = hit_field {
                                    let fid = f.id.unwrap_or(ObjectId { number: 0, generation: 0 });
                                    self.selected_form_field = Some(SelectedFormField {
                                        id: fid,
                                        page_index: current_page,
                                        field_type: f.field_type,
                                        name: f.name.clone(),
                                        value: f.value.clone(),
                                        tooltip: f.tooltip.clone(),
                                        default_value: f.default_value.clone(),
                                        rect: f.rect,
                                        read_only: f.read_only,
                                        required: f.required,
                                        font_size: f.font_size,
                                        text_color: f.text_color,
                                        is_dragging: true,
                                        drag_start_rect: f.rect,
                                        drag_start_pointer: pos,
                                    });
                                    self.form_field_inspector_name = f.name.clone();
                                    self.form_field_inspector_tooltip = f.tooltip.unwrap_or_default();
                                    self.form_field_inspector_default_val = f.default_value.unwrap_or_default();
                                    self.form_field_inspector_required = f.required;
                                    self.form_field_inspector_readonly = f.read_only;
                                    self.form_field_inspector_font_size = f.font_size.unwrap_or(12.0);
                                    self.form_field_inspector_color = f.text_color.unwrap_or([0.0, 0.0, 0.0]);
                                    self.selected_annotation = None;
                                    self.show_annot_inspector = false;
                                    self.show_form_field_inspector = true;
                                    self.text_selection.selected_text.clear();
                                    self.text_selection.selected_spans.clear();
                                } else {
                                    self.selected_annotation = None;
                                    self.show_annot_inspector = false;
                                    self.selected_form_field = None;
                                    self.show_form_field_inspector = false;
                                    self.text_selection.is_selecting = true;
                                    self.text_selection.start_pos = Some(pos);
                                    self.text_selection.current_pos = Some(pos);
                                    self.text_selection.selected_spans.clear();
                                    self.text_selection.selected_text.clear();
                                }
                            }
                        }
                    }

                    if response.dragged() {
                        if let Some(pos) = response.interact_pointer_pos() {
                            self.drag_current_pos = Some(pos);
                            if self.active_tool == ActiveTool::Pencil {
                                self.active_pencil_points.push(pos);
                            } else if self.active_tool == ActiveTool::Select {
                                if let Some(ref mut sel) = self.selected_annotation {
                                    if sel.is_dragging {
                                        let dx = ((pos.x - sel.drag_start_pointer.x) / zoom) as f64;
                                        let dy = -((pos.y - sel.drag_start_pointer.y) / zoom) as f64;
                                        sel.rect = [
                                            sel.drag_start_rect[0] + dx,
                                            sel.drag_start_rect[1] + dy,
                                            sel.drag_start_rect[2] + dx,
                                            sel.drag_start_rect[3] + dy,
                                        ];
                                    }
                                } else if let Some(ref mut sf) = self.selected_form_field {
                                    if sf.is_dragging {
                                        let dx = ((pos.x - sf.drag_start_pointer.x) / zoom) as f64;
                                        let dy = -((pos.y - sf.drag_start_pointer.y) / zoom) as f64;
                                        sf.rect = [
                                            sf.drag_start_rect[0] + dx,
                                            sf.drag_start_rect[1] + dy,
                                            sf.drag_start_rect[2] + dx,
                                            sf.drag_start_rect[3] + dy,
                                        ];
                                    }
                                } else if self.text_selection.is_selecting {
                                    self.text_selection.current_pos = Some(pos);
                                    if let Some(start) = self.text_selection.start_pos {
                                        let sel_screen_rect = Rect::from_two_pos(start, pos);
                                        ui.painter().rect_filled(sel_screen_rect, 0.0, Color32::from_rgba_unmultiplied(66, 133, 244, 40));
                                        ui.painter().rect_stroke(sel_screen_rect, 0.0, Stroke::new(1.0_f32, Color32::from_rgb(66, 133, 244)));

                                        if let Some(tab) = self.active_tab() {
                                            let spans = extract_page_text_spans(&tab.doc, current_page).unwrap_or_default();
                                            let mut matched = Vec::new();
                                            for span in spans {
                                                let span_w = (span.text.len() as f64 * span.font_size * 0.55 * zoom as f64) as f32;
                                                let span_h = (span.font_size * 1.2 * zoom as f64) as f32;
                                                let span_min = Pos2::new(rect.min.x + span.x as f32 * zoom, rect.min.y + (page_h - span.y as f32 - span.font_size as f32) * zoom);
                                                let span_rect = Rect::from_min_size(span_min, Vec2::new(span_w, span_h));
                                                if span_rect.intersects(sel_screen_rect) {
                                                    matched.push(span);
                                                }
                                            }
                                            matched.sort_by(|a, b| {
                                                if (a.y - b.y).abs() > 4.0 {
                                                    b.y.partial_cmp(&a.y).unwrap_or(std::cmp::Ordering::Equal)
                                                } else {
                                                    a.x.partial_cmp(&b.x).unwrap_or(std::cmp::Ordering::Equal)
                                                }
                                            });
                                            self.text_selection.selected_text = matched.iter().map(|s| s.text.as_str()).collect::<Vec<_>>().join(" ");
                                            self.text_selection.selected_spans = matched;
                                        }
                                    }
                                }
                            }
                        }
                    }

                    // Live preview for Pencil (Freehand Ink)
                    if self.active_tool == ActiveTool::Pencil && self.active_pencil_points.len() >= 2 {
                        let ink_color = Color32::from_rgb(30, 100, 220);
                        for window in self.active_pencil_points.windows(2) {
                            ui.painter().line_segment([window[0], window[1]], Stroke::new(2.5_f32, ink_color));
                        }
                    }

                    // Live preview for Eraser
                    if self.active_tool == ActiveTool::Eraser {
                        if let Some(pos) = response.interact_pointer_pos() {
                            ui.painter().circle_stroke(pos, 15.0, Stroke::new(1.5_f32, Color32::from_rgb(239, 68, 68)));
                        }
                    }

                    // Live preview and vertex rendering for Polygon & Polyline (FR-09.2, FR-09.3)
                    if (self.active_tool == ActiveTool::Polygon || self.active_tool == ActiveTool::Polyline) && !self.polygon_vertices.is_empty() {
                        let shape_color = Color32::from_rgb(220, 38, 38);
                        let seg_stroke = Stroke::new(2.0_f32, shape_color);

                        // Draw lines between existing vertices
                        for i in 0..self.polygon_vertices.len().saturating_sub(1) {
                            let p1 = self.polygon_vertices[i];
                            let p2 = self.polygon_vertices[i + 1];
                            let s1 = Pos2::new(rect.min.x + (p1[0] as f32) * zoom, rect.min.y + (page_h - p1[1] as f32) * zoom);
                            let s2 = Pos2::new(rect.min.x + (p2[0] as f32) * zoom, rect.min.y + (page_h - p2[1] as f32) * zoom);
                            ui.painter().line_segment([s1, s2], seg_stroke);
                        }

                        // Draw anchor dots at all vertices
                        for pt in &self.polygon_vertices {
                            let s = Pos2::new(rect.min.x + (pt[0] as f32) * zoom, rect.min.y + (page_h - pt[1] as f32) * zoom);
                            ui.painter().circle_filled(s, 3.5, shape_color);
                            ui.painter().circle_stroke(s, 4.5, Stroke::new(1.0_f32, Color32::WHITE));
                        }

                        // Rubber-band line to cursor
                        if let Some(cursor_pos) = response.hover_pos() {
                            if let Some(&last_pt) = self.polygon_vertices.last() {
                                let last_s = Pos2::new(rect.min.x + (last_pt[0] as f32) * zoom, rect.min.y + (page_h - last_pt[1] as f32) * zoom);
                                ui.painter().line_segment([last_s, cursor_pos], Stroke::new(1.5_f32, Color32::from_rgba_unmultiplied(220, 38, 38, 160)));

                                if self.active_tool == ActiveTool::Polygon && self.polygon_vertices.len() >= 2 {
                                    let first_pt = self.polygon_vertices[0];
                                    let first_s = Pos2::new(rect.min.x + (first_pt[0] as f32) * zoom, rect.min.y + (page_h - first_pt[1] as f32) * zoom);
                                    ui.painter().line_segment([cursor_pos, first_s], Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(100, 116, 139, 120)));
                                }
                            }
                        }
                    }

                    // Magnifying Glass (Loupe) Reticle (FR-01.3.3)
                    if self.active_tool == ActiveTool::Loupe {
                        let scroll_y = ui.input(|i| i.raw_scroll_delta.y);
                        if scroll_y.abs() > 0.0 {
                            self.loupe_magnification = (self.loupe_magnification + scroll_y * 0.02).clamp(1.5, 8.0);
                        }

                        if let Some(ptr_pos) = ui.input(|i| i.pointer.hover_pos()) {
                            if rect.contains(ptr_pos) {
                                let radius = 75.0_f32;
                                let mag = self.loupe_magnification.clamp(1.5, 8.0);
                                ui.painter().circle_filled(ptr_pos, radius + 3.0, Color32::from_black_alpha(80));

                                let uv_center_x = (ptr_pos.x - rect.min.x) / rect.width();
                                let uv_center_y = (ptr_pos.y - rect.min.y) / rect.height();
                                let uv_half_w = radius / (rect.width() * mag);
                                let uv_half_h = radius / (rect.height() * mag);
                                let uv_rect = Rect::from_min_max(
                                    Pos2::new((uv_center_x - uv_half_w).clamp(0.0, 1.0), (uv_center_y - uv_half_h).clamp(0.0, 1.0)),
                                    Pos2::new((uv_center_x + uv_half_w).clamp(0.0, 1.0), (uv_center_y + uv_half_h).clamp(0.0, 1.0)),
                                );

                                if let Some(tab) = self.active_tab() {
                                    if let Some(ref tex) = tab.page_texture {
                                        let loupe_rect = Rect::from_center_size(ptr_pos, Vec2::splat(radius * 2.0));
                                        ui.painter().image(
                                            tex.id(),
                                            loupe_rect,
                                            uv_rect,
                                            Color32::WHITE,
                                        );
                                    }
                                }

                                ui.painter().circle_stroke(ptr_pos, radius, Stroke::new(2.5_f32, Color32::from_rgb(59, 130, 246)));
                                ui.painter().line_segment(
                                    [Pos2::new(ptr_pos.x - 12.0, ptr_pos.y), Pos2::new(ptr_pos.x + 12.0, ptr_pos.y)],
                                    Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(59, 130, 246, 200)),
                                );
                                ui.painter().line_segment(
                                    [Pos2::new(ptr_pos.x, ptr_pos.y - 12.0), Pos2::new(ptr_pos.x, ptr_pos.y + 12.0)],
                                    Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(59, 130, 246, 200)),
                                );

                                let badge_pos = Pos2::new(ptr_pos.x, ptr_pos.y + radius + 14.0);
                                ui.painter().rect_filled(
                                    Rect::from_center_size(badge_pos, Vec2::new(46.0, 18.0)),
                                    3.0,
                                    Color32::from_black_alpha(180),
                                );
                                ui.painter().text(
                                    badge_pos,
                                    Align2::CENTER_CENTER,
                                    format!("{:.1}x", mag),
                                    FontId::proportional(11.0),
                                    Color32::WHITE,
                                );
                            }
                        }
                    }

                    // Live bounding box preview while dragging
                    if let (Some(start), Some(curr)) = (self.drag_start_pos, self.drag_current_pos) {
                        let drag_rect = Rect::from_two_pos(start, curr);
                        match self.active_tool {
                            ActiveTool::Highlight => {
                                ui.painter().rect_filled(
                                    drag_rect,
                                    0.0,
                                    Color32::from_rgba_unmultiplied(255, 255, 0, 80),
                                );
                                ui.painter().rect_stroke(
                                    drag_rect,
                                    0.0,
                                    Stroke::new(1.0_f32, Color32::from_rgb(200, 180, 0)),
                                );
                            }
                            ActiveTool::Underline => {
                                ui.painter().rect_filled(
                                    drag_rect,
                                    0.0,
                                    Color32::from_rgba_unmultiplied(0, 120, 230, 30),
                                );
                                ui.painter().line_segment(
                                    [Pos2::new(drag_rect.min.x, drag_rect.max.y), Pos2::new(drag_rect.max.x, drag_rect.max.y)],
                                    Stroke::new(2.0_f32, Color32::from_rgb(0, 120, 230)),
                                );
                            }
                            ActiveTool::Squiggly => {
                                ui.painter().rect_filled(
                                    drag_rect,
                                    0.0_f32,
                                    Color32::from_rgba_unmultiplied(16, 185, 129, 30),
                                );
                                let start_x = drag_rect.min.x;
                                let end_x = drag_rect.max.x;
                                let base_y = drag_rect.max.y;
                                let step = 4.0_f32;
                                let mut px = start_x;
                                let mut points = Vec::new();
                                let mut up = true;
                                while px <= end_x {
                                    let py = if up { base_y - 2.0_f32 } else { base_y + 2.0_f32 };
                                    points.push(Pos2::new(px, py));
                                    px += step;
                                    up = !up;
                                }
                                for pair in points.windows(2) {
                                    ui.painter().line_segment(
                                        [pair[0], pair[1]],
                                        Stroke::new(1.5_f32, Color32::from_rgb(16, 185, 129)),
                                    );
                                }
                            }
                            ActiveTool::MarqueeZoom => {
                                ui.painter().rect_filled(
                                    drag_rect,
                                    0.0_f32,
                                    Color32::from_rgba_unmultiplied(59, 130, 246, 35),
                                );
                                ui.painter().rect_stroke(
                                    drag_rect,
                                    0.0_f32,
                                    Stroke::new(1.5_f32, Color32::from_rgb(59, 130, 246)),
                                );
                                let text = format!(
                                    "🔍 Zoom to Box ({:.0} × {:.0} pt)",
                                    drag_rect.width() / zoom,
                                    drag_rect.height() / zoom,
                                );
                                let center = drag_rect.center();
                                ui.painter().rect_filled(
                                    Rect::from_center_size(center, Vec2::new(170.0_f32, 22.0_f32)),
                                    4.0_f32,
                                    Color32::from_rgba_unmultiplied(15, 23, 42, 220),
                                );
                                ui.painter().text(
                                    center,
                                    Align2::CENTER_CENTER,
                                    text,
                                    FontId::proportional(11.0_f32),
                                    Color32::WHITE,
                                );
                            }
                            ActiveTool::StrikeOut => {
                                ui.painter().rect_filled(
                                    drag_rect,
                                    0.0,
                                    Color32::from_rgba_unmultiplied(220, 38, 38, 30),
                                );
                                let mid_y = (drag_rect.min.y + drag_rect.max.y) * 0.5;
                                ui.painter().line_segment(
                                    [Pos2::new(drag_rect.min.x, mid_y), Pos2::new(drag_rect.max.x, mid_y)],
                                    Stroke::new(2.0_f32, Color32::from_rgb(220, 38, 38)),
                                );
                            }
                            ActiveTool::Rectangle => {
                                ui.painter().rect_stroke(
                                    drag_rect,
                                    0.0,
                                    Stroke::new(2.0_f32, Color32::from_rgb(220, 38, 38)),
                                );
                            }
                            ActiveTool::Circle => {
                                ui.painter().rect_stroke(
                                    drag_rect,
                                    drag_rect.width().min(drag_rect.height()) * 0.5,
                                    Stroke::new(2.0_f32, Color32::from_rgb(220, 38, 38)),
                                );
                            }
                            ActiveTool::Line => {
                                let stroke_color = Color32::from_rgb(220, 38, 38);
                                ui.painter().line_segment([start, curr], Stroke::new(2.0_f32, stroke_color));
                                let dx = curr.x - start.x;
                                let dy = curr.y - start.y;
                                let len = (dx * dx + dy * dy).sqrt();
                                if len > 5.0 {
                                    let ux = dx / len;
                                    let uy = dy / len;
                                    let p1 = Pos2::new(curr.x - ux * 10.0 - uy * 5.0, curr.y - uy * 10.0 + ux * 5.0);
                                    let p2 = Pos2::new(curr.x - ux * 10.0 + uy * 5.0, curr.y - uy * 10.0 - ux * 5.0);
                                    ui.painter().line_segment([curr, p1], Stroke::new(2.0_f32, stroke_color));
                                    ui.painter().line_segment([curr, p2], Stroke::new(2.0_f32, stroke_color));
                                }
                            }
                            ActiveTool::Callout => {
                                ui.painter().rect_stroke(drag_rect, 2.0, Stroke::new(1.5_f32, Color32::from_rgb(0, 102, 204)));
                                ui.painter().line_segment([start, drag_rect.left_top()], Stroke::new(1.5_f32, Color32::from_rgb(0, 102, 204)));
                            }
                            ActiveTool::Redact => {
                                ui.painter().rect_filled(
                                    drag_rect,
                                    0.0,
                                    Color32::from_rgba_unmultiplied(220, 38, 38, 80),
                                );
                                ui.painter().rect_stroke(
                                    drag_rect,
                                    0.0,
                                    Stroke::new(2.0_f32, Color32::from_rgb(220, 38, 38)),
                                );
                                ui.painter().text(
                                    drag_rect.center(),
                                    Align2::CENTER_CENTER,
                                    "REDACT",
                                    FontId::proportional(12.0),
                                    Color32::WHITE,
                                );
                            }
                            ActiveTool::MeasureDistance => {
                                let cyan = Color32::from_rgb(0, 180, 216);
                                ui.painter().line_segment([start, curr], Stroke::new(2.5_f32, cyan));

                                let local_x1 = ((start.x - rect.min.x) / zoom) as f64;
                                let local_y1 = ((rect.max.y - start.y) / zoom) as f64;
                                let local_x2 = ((curr.x - rect.min.x) / zoom) as f64;
                                let local_y2 = ((rect.max.y - curr.y) / zoom) as f64;

                                let dist_res = calculate_distance([local_x1, local_y1], [local_x2, local_y2], &self.measure_scale, self.measure_unit);

                                let dx = curr.x - start.x;
                                let dy = curr.y - start.y;
                                let len = (dx * dx + dy * dy).sqrt();
                                if len > 2.0 {
                                    let nx = -dy / len * 7.0;
                                    let ny = dx / len * 7.0;
                                    ui.painter().line_segment([Pos2::new(start.x + nx, start.y + ny), Pos2::new(start.x - nx, start.y - ny)], Stroke::new(2.0_f32, cyan));
                                    ui.painter().line_segment([Pos2::new(curr.x + nx, curr.y + ny), Pos2::new(curr.x - nx, curr.y - ny)], Stroke::new(2.0_f32, cyan));
                                }

                                let text = format!("📏 {} ({:.1} pt, {:.1}°)", dist_res.formatted, dist_res.points_distance, dist_res.angle_degrees);
                                let mid = Pos2::new((start.x + curr.x) * 0.5, (start.y + curr.y) * 0.5 - 14.0);
                                ui.painter().rect_filled(Rect::from_center_size(mid, Vec2::new(170.0, 22.0)), 4.0, Color32::from_rgba_unmultiplied(10, 25, 47, 230));
                                ui.painter().text(mid, Align2::CENTER_CENTER, text, FontId::proportional(11.5), Color32::from_rgb(0, 220, 255));
                            }
                            ActiveTool::MeasureArea => {
                                let cyan = Color32::from_rgb(0, 180, 216);
                                ui.painter().rect_filled(drag_rect, 0.0, Color32::from_rgba_unmultiplied(0, 180, 216, 40));
                                ui.painter().rect_stroke(drag_rect, 0.0, Stroke::new(2.0_f32, cyan));

                                let local_x1 = ((drag_rect.min.x - rect.min.x) / zoom) as f64;
                                let local_x2 = ((drag_rect.max.x - rect.min.x) / zoom) as f64;
                                let local_y1 = ((rect.max.y - drag_rect.max.y) / zoom) as f64;
                                let local_y2 = ((rect.max.y - drag_rect.min.y) / zoom) as f64;

                                let area_res = calculate_area(&[
                                    [local_x1, local_y1],
                                    [local_x2, local_y1],
                                    [local_x2, local_y2],
                                    [local_x1, local_y2],
                                ], &self.measure_scale, self.measure_area_unit, self.measure_unit);

                                let text = format!("📐 {} (Perim: {})", area_res.formatted, area_res.perimeter_result.formatted);
                                let center = drag_rect.center();
                                ui.painter().rect_filled(Rect::from_center_size(center, Vec2::new(170.0, 22.0)), 4.0, Color32::from_rgba_unmultiplied(10, 25, 47, 230));
                                ui.painter().text(center, Align2::CENTER_CENTER, text, FontId::proportional(11.5), Color32::from_rgb(0, 220, 255));
                            }
                            ActiveTool::Link => {
                                let blue = Color32::from_rgb(0, 102, 204);
                                ui.painter().rect_filled(drag_rect, 0.0, Color32::from_rgba_unmultiplied(0, 102, 204, 40));
                                ui.painter().rect_stroke(drag_rect, 0.0, Stroke::new(1.5_f32, blue));
                                let center = drag_rect.center();
                                ui.painter().text(center, Align2::CENTER_CENTER, "🔗 Link Hotspot", FontId::proportional(12.0), blue);
                            }
                            ActiveTool::FormField(ft) => {
                                ui.painter().rect_filled(
                                    drag_rect,
                                    2.0,
                                    Color32::from_rgba_unmultiplied(139, 92, 246, 50),
                                );
                                ui.painter().rect_stroke(
                                    drag_rect,
                                    2.0,
                                    Stroke::new(1.8_f32, Color32::from_rgb(139, 92, 246)),
                                );
                                let label = match ft {
                                    FormFieldType::Text => "📝 Text Field",
                                    FormFieldType::Checkbox => "☑ Checkbox",
                                    FormFieldType::Radio => "🔘 Radio Button",
                                    FormFieldType::Choice => "▾ Dropdown",
                                    FormFieldType::PushButton => "🔘 Push Button",
                                    FormFieldType::Signature => "🔏 Signature",
                                    _ => "📝 Form Field",
                                };
                                ui.painter().text(
                                    drag_rect.center(),
                                    Align2::CENTER_CENTER,
                                    label,
                                    FontId::proportional(12.0),
                                    Color32::WHITE,
                                );
                            }
                            _ => {}
                        }
                    }

                    if response.drag_stopped() {
                        if let (Some(start), Some(curr)) = (self.drag_start_pos.take(), self.drag_current_pos.take()) {
                            let drag_rect = Rect::from_two_pos(start, curr);
                            if drag_rect.width() > 4.0 && drag_rect.height() > 4.0 {
                                // Convert to PDF coordinate space
                                let local_x1 = ((drag_rect.min.x - rect.min.x) / zoom) as f64;
                                let local_x2 = ((drag_rect.max.x - rect.min.x) / zoom) as f64;
                                let local_y1 = ((rect.max.y - drag_rect.max.y) / zoom) as f64;
                                let local_y2 = ((rect.max.y - drag_rect.min.y) / zoom) as f64;

                                let pdf_rect = [local_x1, local_y1, local_x2, local_y2];

                                match self.active_tool {
                                    ActiveTool::Highlight => {
                                        if let Some(tab) = self.active_tab_mut() {
                                            let mut annot = Annotation::new(AnnotationType::Highlight, pdf_rect);
                                            annot.color = Some((1.0, 1.0, 0.0));
                                            annot.opacity = 0.5;
                                            let curr_page = tab.current_page;
                                            let _ = add_annotation_to_page(&mut tab.doc, curr_page, &annot);
                                            tab.is_modified = true;
                                            tab.invalidate_render();
                                            self.status_message = "Added highlight annotation".into();
                                        }
                                    }
                                    ActiveTool::Underline => {
                                        if let Some(tab) = self.active_tab_mut() {
                                            let mut annot = Annotation::new(AnnotationType::Underline, pdf_rect);
                                            annot.color = Some((0.0, 0.45, 0.85));
                                            let curr_page = tab.current_page;
                                            let _ = add_annotation_to_page(&mut tab.doc, curr_page, &annot);
                                            tab.is_modified = true;
                                            tab.invalidate_render();
                                            self.status_message = "Added underline annotation".into();
                                        }
                                    }
                                    ActiveTool::Squiggly => {
                                        if let Some(tab) = self.active_tab_mut() {
                                            let mut annot = Annotation::new(
                                                AnnotationType::Squiggly,
                                                pdf_rect,
                                            );
                                            annot.color = Some((0.06, 0.72, 0.50));
                                            let curr_page = tab.current_page;
                                            let _ = add_annotation_to_page(
                                                &mut tab.doc,
                                                curr_page,
                                                &annot,
                                            );
                                            tab.is_modified = true;
                                            tab.invalidate_render();
                                            self.status_message =
                                                "Added squiggly underline annotation".into();
                                        }
                                    }
                                    ActiveTool::MarqueeZoom => {
                                        let avail_w = ui.available_width().max(100.0_f32);
                                        let avail_h = ui.available_height().max(100.0_f32);
                                        let dw = drag_rect.width().max(8.0_f32);
                                        let dh = drag_rect.height().max(8.0_f32);
                                        let factor = ((avail_w - 32.0_f32) / dw)
                                            .min((avail_h - 32.0_f32) / dh);
                                        if let Some(tab) = self.active_tab_mut() {
                                            let target_zoom = (tab.zoom * factor).clamp(0.1_f32, 64.0_f32);
                                            tab.zoom = target_zoom;
                                            tab.invalidate_render();
                                            self.status_message = format!(
                                                "Marquee zoom: {:.0}%",
                                                target_zoom * 100.0_f32
                                            );
                                        }
                                    }
                                    ActiveTool::StrikeOut => {
                                        if let Some(tab) = self.active_tab_mut() {
                                            let mut annot = Annotation::new(AnnotationType::StrikeOut, pdf_rect);
                                            annot.color = Some((0.85, 0.15, 0.15));
                                            let curr_page = tab.current_page;
                                            let _ = add_annotation_to_page(&mut tab.doc, curr_page, &annot);
                                            tab.is_modified = true;
                                            tab.invalidate_render();
                                            self.status_message = "Added strikeout annotation".into();
                                        }
                                    }
                                    ActiveTool::Rectangle => {
                                        if let Some(tab) = self.active_tab_mut() {
                                            let mut annot = Annotation::new(AnnotationType::Square, pdf_rect);
                                            annot.color = Some((0.85, 0.15, 0.15));
                                            let curr_page = tab.current_page;
                                            let _ = add_annotation_to_page(&mut tab.doc, curr_page, &annot);
                                            tab.is_modified = true;
                                            tab.invalidate_render();
                                            self.status_message = "Added rectangle shape".into();
                                        }
                                    }
                                    ActiveTool::Circle => {
                                        if let Some(tab) = self.active_tab_mut() {
                                            let mut annot = Annotation::new(AnnotationType::Circle, pdf_rect);
                                            annot.color = Some((0.85, 0.15, 0.15));
                                            let curr_page = tab.current_page;
                                            let _ = add_annotation_to_page(&mut tab.doc, curr_page, &annot);
                                            tab.is_modified = true;
                                            tab.invalidate_render();
                                            self.status_message = "Added circle shape".into();
                                        }
                                    }
                                    ActiveTool::Line => {
                                        if let Some(tab) = self.active_tab_mut() {
                                            let local_start_x = ((start.x - rect.min.x) / zoom) as f64;
                                            let local_start_y = ((rect.max.y - start.y) / zoom) as f64;
                                            let local_end_x = ((curr.x - rect.min.x) / zoom) as f64;
                                            let local_end_y = ((rect.max.y - curr.y) / zoom) as f64;

                                            let config = ShapeAnnotationConfig {
                                                shape_type: ShapeType::Line {
                                                    start: [local_start_x, local_start_y],
                                                    end: [local_end_x, local_end_y],
                                                    start_style: LineEndingStyle::None,
                                                    end_style: LineEndingStyle::OpenArrow,
                                                },
                                                rect: [local_start_x.min(local_end_x), local_start_y.min(local_end_y), local_start_x.max(local_end_x), local_start_y.max(local_end_y)],
                                                stroke_color: Some((0.85, 0.15, 0.15)),
                                                fill_color: None,
                                                stroke_width: 2.0,
                                                border_style: BorderStyle::Solid,
                                                border_effect: BorderEffect::None,
                                                opacity: 1.0,
                                                author: Some("User".into()),
                                                locked: false,
                                                popup_note: None,
                                            };
                                            let curr_page = tab.current_page;
                                            let _ = add_shape_annotation(&mut tab.doc, curr_page, &config);
                                            tab.is_modified = true;
                                            tab.invalidate_render();
                                            self.status_message = "Added arrow line".into();
                                        }
                                    }
                                    ActiveTool::Callout => {
                                        let local_start_x = ((start.x - rect.min.x) / zoom) as f64;
                                        let local_start_y = ((rect.max.y - start.y) / zoom) as f64;
                                        let local_end_x = ((curr.x - rect.min.x) / zoom) as f64;
                                        let local_end_y = ((rect.max.y - curr.y) / zoom) as f64;

                                        self.callout_leader_x = local_start_x;
                                        self.callout_leader_y = local_start_y;
                                        self.callout_knee_x = (local_start_x + local_end_x) * 0.5;
                                        self.callout_knee_y = (local_start_y + local_end_y) * 0.5;
                                        self.callout_box_x = local_end_x;
                                        self.callout_box_y = local_end_y;
                                        self.callout_box_w = 160.0;
                                        self.callout_box_h = 40.0;
                                        self.show_callout_dialog = true;
                                    }
                                    ActiveTool::Link => {
                                        self.link_rect = pdf_rect;
                                        let current_page = self.active_tab().map(|t| t.current_page).unwrap_or(0);
                                        let total_pages = self.active_tab().map(|t| t.doc.page_count()).unwrap_or(1);
                                        self.link_target_page = (current_page + 2).min(total_pages);
                                        self.show_link_dialog = true;
                                    }
                                    ActiveTool::Redact => {
                                        let current_page = self.active_tab().map(|t| t.current_page).unwrap_or(0);
                                        let r_id = self.next_redaction_id;
                                        self.next_redaction_id += 1;
                                        let overlay = if self.redact_replacement_input.trim().is_empty() {
                                            Some("[REDACTED]".to_string())
                                        } else {
                                            Some(self.redact_replacement_input.clone())
                                        };
                                        self.marked_redactions.push(MarkedRedaction {
                                            id: r_id,
                                            page_index: current_page,
                                            rect: pdf_rect,
                                            appearance: RedactionAppearance {
                                                fill_color: [0.0, 0.0, 0.0],
                                                overlay_text: overlay,
                                                text_color: [1.0, 1.0, 1.0],
                                                font_size: None,
                                            },
                                        });
                                        self.status_message = format!("Marked area on page {} for redaction. Total marked: {}", current_page + 1, self.marked_redactions.len());
                                    }
                                    ActiveTool::FormField(ft) => {
                                        let current_page = self.active_tab().map(|t| t.current_page).unwrap_or(0);
                                        let mut created = None;
                                        if let Some(tab) = self.active_tab_mut() {
                                            let count = get_form_fields(&tab.doc).map(|f| f.len()).unwrap_or(0);
                                            let field = match ft {
                                                FormFieldType::Text => FormField::new_text(format!("Text_{}", count + 1), pdf_rect),
                                                FormFieldType::Checkbox => FormField::new_checkbox(format!("Check_{}", count + 1), pdf_rect, false),
                                                FormFieldType::Radio => FormField::new_radio_button(format!("Radio_{}", count + 1), "Choice1", pdf_rect, false),
                                                FormFieldType::Choice => FormField::new_choice(format!("Dropdown_{}", count + 1), pdf_rect, vec!["Option 1".into(), "Option 2".into()], Some("Option 1".into())),
                                                FormFieldType::PushButton => FormField::new_pushbutton(format!("Button_{}", count + 1), pdf_rect, "Button"),
                                                FormFieldType::Signature => FormField::new_signature(format!("Signature_{}", count + 1), pdf_rect),
                                                _ => FormField::new_text(format!("Field_{}", count + 1), pdf_rect),
                                            };
                                            if let Ok(fid) = add_form_field(&mut tab.doc, current_page, &field) {
                                                tab.is_modified = true;
                                                tab.invalidate_render();
                                                created = Some((fid, field));
                                            }
                                        }
                                        if let Some((fid, field)) = created {
                                            self.selected_form_field = Some(SelectedFormField {
                                                id: fid,
                                                page_index: current_page,
                                                field_type: ft,
                                                name: field.name.clone(),
                                                value: field.value.clone(),
                                                tooltip: field.tooltip.clone(),
                                                default_value: field.default_value.clone(),
                                                rect: field.rect,
                                                read_only: field.read_only,
                                                required: field.required,
                                                font_size: field.font_size,
                                                text_color: field.text_color,
                                                is_dragging: false,
                                                drag_start_rect: field.rect,
                                                drag_start_pointer: curr,
                                            });
                                            self.form_field_inspector_name = field.name.clone();
                                            self.form_field_inspector_tooltip = String::new();
                                            self.form_field_inspector_default_val = String::new();
                                            self.form_field_inspector_required = false;
                                            self.form_field_inspector_readonly = false;
                                            self.form_field_inspector_font_size = 12.0;
                                            self.form_field_inspector_color = [0.0, 0.0, 0.0];
                                            self.show_form_field_inspector = true;
                                            self.active_tool = ActiveTool::Select;
                                            self.status_message = format!("Created {} form field '{}'", ft.ft_name(), field.name);
                                        }
                                    }
                                    ActiveTool::MeasureDistance => {
                                        let local_x1 = ((start.x - rect.min.x) / zoom) as f64;
                                        let local_y1 = ((rect.max.y - start.y) / zoom) as f64;
                                        let local_x2 = ((curr.x - rect.min.x) / zoom) as f64;
                                        let local_y2 = ((rect.max.y - curr.y) / zoom) as f64;

                                        let dist_res = calculate_distance([local_x1, local_y1], [local_x2, local_y2], &self.measure_scale, self.measure_unit);
                                        self.measure_calib_real_val = dist_res.world_distance;

                                        if self.measure_bake_as_annot {
                                            if let Some(tab) = self.active_tab_mut() {
                                                let curr_page = tab.current_page;
                                                let _ = add_measurement_line_annotation(&mut tab.doc, curr_page, &dist_res, (0.0, 0.7, 0.9));
                                                tab.is_modified = true;
                                                tab.invalidate_render();
                                                self.status_message = format!("Added measurement annotation: {}", dist_res.formatted);
                                            }
                                        } else {
                                            self.status_message = format!("Measured Distance: {} ({:.1} pt, Angle: {:.1}°)", dist_res.formatted, dist_res.points_distance, dist_res.angle_degrees);
                                        }
                                    }
                                    ActiveTool::MeasureArea => {
                                        let local_x1 = ((drag_rect.min.x - rect.min.x) / zoom) as f64;
                                        let local_x2 = ((drag_rect.max.x - rect.min.x) / zoom) as f64;
                                        let local_y1 = ((rect.max.y - drag_rect.max.y) / zoom) as f64;
                                        let local_y2 = ((rect.max.y - drag_rect.min.y) / zoom) as f64;

                                        let area_res = calculate_area(&[
                                            [local_x1, local_y1],
                                            [local_x2, local_y1],
                                            [local_x2, local_y2],
                                            [local_x1, local_y2],
                                        ], &self.measure_scale, self.measure_area_unit, self.measure_unit);

                                        self.status_message = format!("Measured Area: {} (Perimeter: {})", area_res.formatted, area_res.perimeter_result.formatted);
                                    }
                                    _ => {}
                                }
                            }
                        }
                        self.marquee_zoom_start = None;
                    }

                    if response.drag_stopped() && self.active_tool == ActiveTool::Select {
                        if let Some(ref mut sel) = self.selected_annotation {
                            if sel.is_dragging {
                                sel.is_dragging = false;
                                let final_rect = sel.rect;
                                let a_id = sel.id;
                                if let Some(tab) = self.active_tab_mut() {
                                    let _ = update_annotation_rect(&mut tab.doc, a_id, final_rect);
                                    tab.is_modified = true;
                                    tab.invalidate_render();
                                    self.status_message = format!("Moved annotation to ({:.1}, {:.1})", final_rect[0], final_rect[1]);
                                }
                            }
                        } else if let Some(ref mut sf) = self.selected_form_field {
                            if sf.is_dragging {
                                sf.is_dragging = false;
                                let final_rect = sf.rect;
                                let f_id = sf.id;
                                let f_name = sf.name.clone();
                                if let Some(tab) = self.active_tab_mut() {
                                    let _ = update_form_field_rect(&mut tab.doc, f_id, final_rect);
                                    tab.is_modified = true;
                                    tab.invalidate_render();
                                    self.status_message = format!("Moved form field '{}' to ({:.1}, {:.1})", f_name, final_rect[0], final_rect[1]);
                                }
                            }
                        }
                        self.text_selection.is_selecting = false;
                    }

                    if response.drag_stopped() && self.active_tool == ActiveTool::Pencil {
                        if self.active_pencil_points.len() >= 2 {
                            let pts: Vec<[f64; 2]> = self.active_pencil_points.drain(..)
                                .map(|p| [((p.x - rect.min.x) / zoom) as f64, ((rect.max.y - p.y) / zoom) as f64])
                                .collect();
                            if let Some(tab) = self.active_tab_mut() {
                                let curr_page = tab.current_page;
                                let config = InkAnnotationConfig {
                                    strokes: vec![pts],
                                    stroke_width: 2.5,
                                    color: (0.1, 0.4, 0.9),
                                    opacity: 1.0,
                                    smooth: true,
                                    author: Some("User".into()),
                                    locked: false,
                                    popup_note: None,
                                };
                                let _ = add_ink_annotation(&mut tab.doc, curr_page, &config);
                                tab.is_modified = true;
                                tab.invalidate_render();
                                self.status_message = "Added freehand pencil drawing".into();
                            }
                        } else {
                            self.active_pencil_points.clear();
                        }
                    }

                    if (response.dragged() || response.clicked()) && self.active_tool == ActiveTool::Eraser {
                        if let Some(pos) = response.interact_pointer_pos() {
                            let ex = ((pos.x - rect.min.x) / zoom) as f64;
                            let ey = ((rect.max.y - pos.y) / zoom) as f64;
                            if let Some(tab) = self.active_tab_mut() {
                                let curr_page = tab.current_page;
                                if let Ok(annots) = get_page_annotations(&tab.doc, curr_page) {
                                    let mut erased = false;
                                    for a in annots {
                                        if matches!(a.annot_type, AnnotationType::Ink) {
                                            if let Some(id) = a.id {
                                                if let Ok(true) = erase_ink_strokes(&mut tab.doc, curr_page, id, [ex, ey], 20.0) {
                                                    erased = true;
                                                }
                                            }
                                        }
                                    }
                                    if erased {
                                        tab.is_modified = true;
                                        tab.invalidate_render();
                                        self.status_message = "Erased ink stroke".into();
                                    }
                                }
                            }
                        }
                    }

                    // Click handlers for point-based tools
                    if response.clicked() {
                        if let Some(pos) = response.interact_pointer_pos() {
                            let local_x = ((pos.x - rect.min.x) / zoom) as f64;
                            let local_y = ((rect.max.y - pos.y) / zoom) as f64;

                            match self.active_tool {
                                ActiveTool::Select => {
                                    let mut pill_clicked = false;
                                    if !self.text_selection.selected_text.is_empty() {
                                        if let Some(curr) = self.text_selection.current_pos {
                                            let pill_w = 360.0_f32;
                                            let pill_x = curr.x.clamp(rect.min.x, (rect.max.x - pill_w).max(rect.min.x));
                                            let pill_y = (curr.y - 34.0).max(rect.min.y);
                                            let pill_rect = Rect::from_min_size(Pos2::new(pill_x, pill_y), Vec2::new(pill_w, 28.0));
                                            if pill_rect.contains(pos) {
                                                pill_clicked = true;
                                            }
                                        }
                                    }
                                    if !pill_clicked {
                                        let mut hit_annot = None;
                                        if let Some(tab) = self.active_tab() {
                                            if let Ok(annots) = get_page_annotations(&tab.doc, current_page) {
                                                for a in annots {
                                                    let ax1 = rect.min.x + (a.rect[0] as f32) * zoom;
                                                    let ax2 = rect.min.x + (a.rect[2] as f32) * zoom;
                                                    let ay1 = rect.min.y + (page_h - a.rect[3] as f32) * zoom;
                                                    let ay2 = rect.min.y + (page_h - a.rect[1] as f32) * zoom;
                                                    let a_rect = Rect::from_min_max(Pos2::new(ax1.min(ax2), ay1.min(ay2)), Pos2::new(ax1.max(ax2), ay1.max(ay2))).expand(4.0);
                                                    if a_rect.contains(pos) {
                                                        hit_annot = Some(a);
                                                        break;
                                                    }
                                                }
                                            }
                                        }

                                        let mut hit_field = None;
                                        if hit_annot.is_none() {
                                            if let Some(tab) = self.active_tab() {
                                                if let Ok(fields) = get_page_form_fields(&tab.doc, current_page) {
                                                    for f in fields {
                                                        let fx1 = rect.min.x + (f.rect[0] as f32) * zoom;
                                                        let fx2 = rect.min.x + (f.rect[2] as f32) * zoom;
                                                        let fy1 = rect.min.y + (page_h - f.rect[3] as f32) * zoom;
                                                        let fy2 = rect.min.y + (page_h - f.rect[1] as f32) * zoom;
                                                        let f_rect = Rect::from_min_max(Pos2::new(fx1.min(fx2), fy1.min(fy2)), Pos2::new(fx1.max(fx2), fy1.max(fy2))).expand(4.0);
                                                        if f_rect.contains(pos) {
                                                            hit_field = Some(f);
                                                            break;
                                                        }
                                                    }
                                                }
                                            }
                                        }

                                        if let Some(a) = hit_annot {
                                            let a_id = a.id.unwrap_or(ObjectId { number: 0, generation: 0 });
                                            self.selected_annotation = Some(SelectedAnnotation {
                                                id: a_id,
                                                page_index: current_page,
                                                annot_type: a.annot_type.clone(),
                                                rect: a.rect,
                                                contents: a.contents.clone(),
                                                author: a.author.clone(),
                                                color: a.color,
                                                opacity: a.opacity,
                                                is_dragging: false,
                                                drag_start_rect: a.rect,
                                                drag_start_pointer: pos,
                                            });
                                            self.annot_inspector_contents = a.contents.unwrap_or_default();
                                            self.annot_inspector_author = a.author.unwrap_or_else(|| "User".into());
                                            if let Some(c) = a.color {
                                                self.annot_inspector_color = [c.0, c.1, c.2];
                                            }
                                            self.annot_inspector_opacity = a.opacity;
                                            self.show_annot_inspector = true;
                                            self.selected_form_field = None;
                                            self.show_form_field_inspector = false;
                                        } else if let Some(f) = hit_field {
                                            let fid = f.id.unwrap_or(ObjectId { number: 0, generation: 0 });
                                            self.selected_form_field = Some(SelectedFormField {
                                                id: fid,
                                                page_index: current_page,
                                                field_type: f.field_type,
                                                name: f.name.clone(),
                                                value: f.value.clone(),
                                                tooltip: f.tooltip.clone(),
                                                default_value: f.default_value.clone(),
                                                rect: f.rect,
                                                read_only: f.read_only,
                                                required: f.required,
                                                font_size: f.font_size,
                                                text_color: f.text_color,
                                                is_dragging: false,
                                                drag_start_rect: f.rect,
                                                drag_start_pointer: pos,
                                            });
                                            self.form_field_inspector_name = f.name.clone();
                                            self.form_field_inspector_tooltip = f.tooltip.unwrap_or_default();
                                            self.form_field_inspector_default_val = f.default_value.unwrap_or_default();
                                            self.form_field_inspector_required = f.required;
                                            self.form_field_inspector_readonly = f.read_only;
                                            self.form_field_inspector_font_size = f.font_size.unwrap_or(12.0);
                                            self.form_field_inspector_color = f.text_color.unwrap_or([0.0, 0.0, 0.0]);
                                            self.selected_annotation = None;
                                            self.show_annot_inspector = false;
                                            self.show_form_field_inspector = true;
                                        } else {
                                            self.selected_annotation = None;
                                            self.show_annot_inspector = false;
                                            self.selected_form_field = None;
                                            self.show_form_field_inspector = false;
                                        }
                                    }
                                }
                                ActiveTool::StickyNote => {
                                    if let Some(tab) = self.active_tab_mut() {
                                        let annot = Annotation::new(
                                            AnnotationType::Text,
                                            [local_x, local_y, local_x + 30.0, local_y + 30.0],
                                        );
                                        let curr_page = tab.current_page;
                                        let _ = add_annotation_to_page(&mut tab.doc, curr_page, &annot);
                                        tab.is_modified = true;
                                        tab.invalidate_render();
                                        self.status_message = "Added sticky note".into();
                                    }
                                }
                                ActiveTool::Text => {
                                    self.free_text_coords = Some([local_x, local_y, local_x + 120.0, local_y + 24.0]);
                                    self.free_text_content.clear();
                                    self.show_free_text_dialog = true;
                                }
                                ActiveTool::Typewriter => {
                                    self.typewriter_pos_x = local_x;
                                    self.typewriter_pos_y = local_y;
                                    self.show_typewriter_dialog = true;
                                }
                                ActiveTool::Callout => {
                                    self.callout_box_x = local_x;
                                    self.callout_box_y = local_y;
                                    self.callout_box_w = 160.0;
                                    self.callout_box_h = 40.0;
                                    self.callout_leader_x = local_x - 50.0;
                                    self.callout_leader_y = local_y - 40.0;
                                    self.callout_knee_x = local_x - 15.0;
                                    self.callout_knee_y = local_y - 10.0;
                                    self.show_callout_dialog = true;
                                }
                                ActiveTool::Polygon => {
                                    let mut finish = false;
                                    if self.polygon_vertices.len() >= 3 {
                                        let first = self.polygon_vertices[0];
                                        let dx = (first[0] - local_x).abs();
                                        let dy = (first[1] - local_y).abs();
                                        if (dx * zoom as f64) < 12.0 && (dy * zoom as f64) < 12.0 {
                                            finish = true;
                                        }
                                    }
                                    if finish {
                                        let vertices = std::mem::take(&mut self.polygon_vertices);
                                        let num_verts = vertices.len();
                                        if let Some(tab) = self.active_tab_mut() {
                                            let config = ShapeAnnotationConfig {
                                                shape_type: ShapeType::Polygon { vertices },
                                                rect: [0.0, 0.0, 0.0, 0.0],
                                                stroke_color: Some((0.85, 0.15, 0.15)),
                                                fill_color: None,
                                                stroke_width: 2.0,
                                                border_style: BorderStyle::Solid,
                                                border_effect: BorderEffect::None,
                                                opacity: 1.0,
                                                author: Some("User".into()),
                                                locked: false,
                                                popup_note: None,
                                            };
                                            let curr_page = tab.current_page;
                                            let _ = add_shape_annotation(&mut tab.doc, curr_page, &config);
                                            tab.is_modified = true;
                                            tab.invalidate_render();
                                            self.status_message = format!("Created polygon with {} vertices", num_verts);
                                        }
                                    } else {
                                        self.polygon_vertices.push([local_x, local_y]);
                                        self.status_message = format!("Added vertex {} (Click near start or press Enter to close)", self.polygon_vertices.len());
                                    }
                                }
                                ActiveTool::Polyline => {
                                    self.polygon_vertices.push([local_x, local_y]);
                                    self.status_message = format!("Added polyline vertex {} (Double-click or press Enter to finish)", self.polygon_vertices.len());
                                }
                                _ => {}
                            }
                        }
                    }

                    // Keyboard & double-click shortcuts for Polygon and Polyline
                    if self.active_tool == ActiveTool::Polygon || self.active_tool == ActiveTool::Polyline {
                        if ctx.input(|i| i.key_pressed(Key::Escape)) {
                            self.polygon_vertices.clear();
                            self.status_message = "Cancelled drawing".into();
                        } else if ctx.input(|i| i.key_pressed(Key::Enter)) || response.double_clicked() {
                            if self.active_tool == ActiveTool::Polygon && self.polygon_vertices.len() >= 3 {
                                let vertices = std::mem::take(&mut self.polygon_vertices);
                                let num_verts = vertices.len();
                                if let Some(tab) = self.active_tab_mut() {
                                    let config = ShapeAnnotationConfig {
                                        shape_type: ShapeType::Polygon { vertices },
                                        rect: [0.0, 0.0, 0.0, 0.0],
                                        stroke_color: Some((0.85, 0.15, 0.15)),
                                        fill_color: None,
                                        stroke_width: 2.0,
                                        border_style: BorderStyle::Solid,
                                        border_effect: BorderEffect::None,
                                        opacity: 1.0,
                                        author: Some("User".into()),
                                        locked: false,
                                        popup_note: None,
                                    };
                                    let curr_page = tab.current_page;
                                    let _ = add_shape_annotation(&mut tab.doc, curr_page, &config);
                                    tab.is_modified = true;
                                    tab.invalidate_render();
                                    self.status_message = format!("Created polygon with {} vertices", num_verts);
                                }
                            } else if self.active_tool == ActiveTool::Polyline && self.polygon_vertices.len() >= 2 {
                                let vertices = std::mem::take(&mut self.polygon_vertices);
                                let num_verts = vertices.len();
                                if let Some(tab) = self.active_tab_mut() {
                                    let config = ShapeAnnotationConfig {
                                        shape_type: ShapeType::PolyLine {
                                            vertices,
                                            start_style: LineEndingStyle::None,
                                            end_style: LineEndingStyle::None,
                                        },
                                        rect: [0.0, 0.0, 0.0, 0.0],
                                        stroke_color: Some((0.85, 0.15, 0.15)),
                                        fill_color: None,
                                        stroke_width: 2.0,
                                        border_style: BorderStyle::Solid,
                                        border_effect: BorderEffect::None,
                                        opacity: 1.0,
                                        author: Some("User".into()),
                                        locked: false,
                                        popup_note: None,
                                    };
                                    let curr_page = tab.current_page;
                                    let _ = add_shape_annotation(&mut tab.doc, curr_page, &config);
                                    tab.is_modified = true;
                                    tab.invalidate_render();
                                    self.status_message = format!("Created polyline with {} vertices", num_verts);
                                }
                            }
                        }
                    }

                    if response.double_clicked() && self.active_tool == ActiveTool::Select {
                        if let Some(pos) = response.interact_pointer_pos() {
                            if let Some(tab) = self.active_tab() {
                                let spans = extract_page_text_spans(&tab.doc, current_page).unwrap_or_default();
                                for span in spans {
                                    let span_w = (span.text.len() as f64 * span.font_size * 0.55 * zoom as f64) as f32;
                                    let span_h = (span.font_size * 1.2 * zoom as f64) as f32;
                                    let span_min = Pos2::new(rect.min.x + span.x as f32 * zoom, rect.min.y + (page_h - span.y as f32 - span.font_size as f32) * zoom);
                                    let span_rect = Rect::from_min_size(span_min, Vec2::new(span_w, span_h));
                                    if span_rect.contains(pos) {
                                        self.text_selection.selected_text = span.text.clone();
                                        self.text_selection.selected_spans = vec![span];
                                        self.text_selection.current_pos = Some(pos);
                                        self.status_message = format!("Selected text: \"{}\"", self.text_selection.selected_text);
                                        break;
                                    }
                                }
                            }
                        }
                    }
                });
        });
    }

    /// Renders welcome screen when no documents are loaded (EI-01.7).
    fn render_welcome_screen(&mut self, ui: &mut egui::Ui) {
        ui.vertical_centered(|ui| {
            ui.add_space(80.0);

            // Title
            ui.heading(
                RichText::new("RAV PDF")
                    .size(36.0)
                    .strong()
                    .color(Color32::from_rgb(60, 130, 246)),
            );

            ui.add_space(40.0);

            // Primary Action Buttons
            ui.horizontal(|ui| {
                ui.add_space((ui.available_width() - 580.0).max(0.0) * 0.5);

                let open_btn = Button::new(RichText::new("📂  Open PDF").size(14.0).strong())
                    .min_size(Vec2::new(130.0, 44.0));
                if ui.add(open_btn).clicked() {
                    self.open_file_dialog();
                }

                ui.add_space(10.0);

                let new_btn = Button::new(RichText::new("📄  New Blank").size(14.0))
                    .min_size(Vec2::new(130.0, 44.0));
                if ui.add(new_btn).clicked() {
                    self.create_new_blank_document();
                }

                ui.add_space(10.0);

                let img_btn = Button::new(RichText::new("🖼  From Images").size(14.0))
                    .min_size(Vec2::new(130.0, 44.0));
                if ui.add(img_btn).clicked() {
                    self.create_pdf_from_images_dialog();
                }

                ui.add_space(10.0);

                let comb_btn = Button::new(RichText::new("🔗  Combine Files").size(14.0))
                    .min_size(Vec2::new(130.0, 44.0));
                if ui.add(comb_btn).clicked() {
                    self.show_combine_dialog = true;
                }
            });

            ui.add_space(40.0);

            // Drag and drop target box (EI-01.7)
            let drop_box_size = Vec2::new(420.0, 120.0);
            let (rect, _resp) = ui.allocate_exact_size(drop_box_size, egui::Sense::hover());
            let painter = ui.painter();

            painter.rect_stroke(rect, 8.0, Stroke::new(1.5_f32, Color32::from_gray(100)));

            painter.text(
                rect.center() - Vec2::new(0.0, 10.0),
                Align2::CENTER_CENTER,
                "📥 Drag and drop any PDF file here",
                FontId::proportional(15.0),
                Color32::from_gray(170),
            );
            painter.text(
                rect.center() + Vec2::new(0.0, 16.0),
                Align2::CENTER_CENTER,
                "Supports ISO 32000-1 / ISO 32000-2 (PDF 1.0 - 2.0)",
                FontId::proportional(12.0),
                Color32::from_gray(120),
            );

            ui.add_space(50.0);

            // Feature Badges
            ui.horizontal(|ui| {
                ui.add_space((ui.available_width() - 480.0).max(0.0) * 0.5);
                ui.label(RichText::new("⚡ Sub-second Rendering").size(12.0).weak());
                ui.label("•");
                ui.label(RichText::new("🔒 AES-128 Encryption").size(12.0).weak());
                ui.label("•");
                ui.label(RichText::new("📝 Form Flattening").size(12.0).weak());
                ui.label("•");
                ui.label(RichText::new("🛡 Memory Safe").size(12.0).weak());
            });
        });
    }

    /// Renders modal dialogs for Watermarking, Bates Numbering, and Headers/Footers (FR-18).
    fn render_dialogs(&mut self, ctx: &egui::Context) {
        // 1. Watermark Dialog (FR-18.2)
        if self.show_watermark_dialog {
            let mut close = false;
            egui::Window::new("Add Text Watermark")
                .collapsible(false)
                .resizable(false)
                .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Watermark Text:");
                        ui.text_edit_singleline(&mut self.watermark_text);
                    });
                    ui.horizontal(|ui| {
                        ui.label("Opacity (0.05 - 1.0):");
                        ui.add(egui::Slider::new(&mut self.watermark_opacity, 0.05..=1.0));
                    });
                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        if ui.button("Apply Watermark").clicked() {
                            let text = self.watermark_text.clone();
                            let opacity = self.watermark_opacity;
                            if let Some(tab) = self.active_tab_mut() {
                                let _ =
                                    apply_text_watermark(&mut tab.doc, &text, opacity, 45.0, 54.0);
                                tab.is_modified = true;
                                tab.invalidate_render();
                                self.status_message = "Applied watermark to document".into();
                            }
                            close = true;
                        }
                        if ui.button("Cancel").clicked() {
                            close = true;
                        }
                    });
                });
            if close {
                self.show_watermark_dialog = false;
            }
        }

        // 2. Bates Numbering Dialog (FR-18.3)
        if self.show_bates_dialog {
            let mut close = false;
            egui::Window::new("Apply Bates Numbering")
                .collapsible(false)
                .resizable(false)
                .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Prefix:");
                        ui.text_edit_singleline(&mut self.bates_prefix);
                    });
                    ui.horizontal(|ui| {
                        ui.label("Starting Number:");
                        ui.add(egui::DragValue::new(&mut self.bates_start));
                    });
                    ui.horizontal(|ui| {
                        ui.label("Digit Count:");
                        ui.add(egui::Slider::new(&mut self.bates_digits, 3..=10));
                    });
                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        if ui.button("Apply Bates Numbering").clicked() {
                            let prefix = self.bates_prefix.clone();
                            let start = self.bates_start;
                            let digits = self.bates_digits;
                            if let Some(tab) = self.active_tab_mut() {
                                let _ =
                                    apply_bates_numbering(&mut tab.doc, &prefix, start, digits, "");
                                tab.is_modified = true;
                                tab.invalidate_render();
                                self.status_message = "Applied Bates numbering".into();
                            }
                            close = true;
                        }
                        if ui.button("Cancel").clicked() {
                            close = true;
                        }
                    });
                });
            if close {
                self.show_bates_dialog = false;
            }
        }

        // 3. Headers and Footers Dialog (FR-18.1)
        if self.show_header_footer_dialog {
            let mut close = false;
            egui::Window::new("Add Headers & Footers")
                .collapsible(false)
                .resizable(false)
                .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.heading("Header Positions");
                    ui.horizontal(|ui| {
                        ui.label("Left:");
                        let mut left_h = self.hf_config.left_header.clone().unwrap_or_default();
                        if ui.text_edit_singleline(&mut left_h).changed() {
                            self.hf_config.left_header = if left_h.is_empty() { None } else { Some(left_h) };
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.label("Center:");
                        let mut center_h = self.hf_config.center_header.clone().unwrap_or_default();
                        if ui.text_edit_singleline(&mut center_h).changed() {
                            self.hf_config.center_header = if center_h.is_empty() { None } else { Some(center_h) };
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.label("Right:");
                        let mut right_h = self.hf_config.right_header.clone().unwrap_or_default();
                        if ui.text_edit_singleline(&mut right_h).changed() {
                            self.hf_config.right_header = if right_h.is_empty() { None } else { Some(right_h) };
                        }
                    });

                    ui.separator();
                    ui.heading("Footer Positions");
                    ui.horizontal(|ui| {
                        ui.label("Left:");
                        let mut left_f = self.hf_config.left_footer.clone().unwrap_or_default();
                        if ui.text_edit_singleline(&mut left_f).changed() {
                            self.hf_config.left_footer = if left_f.is_empty() { None } else { Some(left_f) };
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.label("Center:");
                        let mut center_f = self.hf_config.center_footer.clone().unwrap_or_default();
                        if ui.text_edit_singleline(&mut center_f).changed() {
                            self.hf_config.center_footer = if center_f.is_empty() { None } else { Some(center_f) };
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.label("Right:");
                        let mut right_f = self.hf_config.right_footer.clone().unwrap_or_default();
                        if ui.text_edit_singleline(&mut right_f).changed() {
                            self.hf_config.right_footer = if right_f.is_empty() { None } else { Some(right_f) };
                        }
                    });

                    ui.separator();
                    ui.horizontal(|ui| {
                        ui.label("Font Size:");
                        ui.add(egui::Slider::new(&mut self.hf_config.font_size, 6.0..=24.0));
                    });

                    ui.weak("Tokens: <<Page>> = Page #, <<TotalPages>> = Total Pages, <<Date>> = Date, <<Title>> = Title");

                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        if ui.button("Apply Headers & Footers").clicked() {
                            let config = self.hf_config.clone();
                            if let Some(tab) = self.active_tab_mut() {
                                let doc_title = tab.metadata.as_ref().and_then(|m| m.title.as_deref());
                                let _ = apply_headers_and_footers(
                                    &mut tab.doc,
                                    &config,
                                    doc_title,
                                    None,
                                );
                                tab.is_modified = true;
                                tab.invalidate_render();
                                self.status_message = "Applied headers and footers".into();
                            }
                            close = true;
                        }
                        if ui.button("Cancel").clicked() {
                            close = true;
                        }
                    });
                });
            if close {
                self.show_header_footer_dialog = false;
            }
        }

        // 4. FreeText Annotation Dialog (FR-08.1.4)
        if self.show_free_text_dialog {
            let mut close = false;
            egui::Window::new("Add Text Box Annotation")
                .collapsible(false)
                .resizable(false)
                .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.label("Text Content:");
                    ui.text_edit_multiline(&mut self.free_text_content);
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui.button("Insert Text").clicked() {
                            let text = self.free_text_content.clone();
                            let coords = self.free_text_coords;
                            if let (Some(c), Some(tab)) = (coords, self.active_tab_mut()) {
                                let mut annot = Annotation::new(AnnotationType::FreeText, c);
                                annot.contents = Some(text);
                                annot.color = Some((0.1, 0.4, 0.8));
                                let curr_page = tab.current_page;
                                let _ = add_annotation_to_page(&mut tab.doc, curr_page, &annot);
                                tab.is_modified = true;
                                tab.invalidate_render();
                                self.status_message = "Added text box annotation".into();
                            }
                            close = true;
                        }
                        if ui.button("Cancel").clicked() {
                            close = true;
                        }
                    });
                });
            if close {
                self.show_free_text_dialog = false;
            }
        }

        // 5. Combine / Merge Files Dialog (FR-07.1)
        if self.show_combine_dialog {
            let mut close = false;
            egui::Window::new("Combine / Merge PDF Files")
                .collapsible(false)
                .resizable(true)
                .default_size([480.0, 360.0])
                .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.label(
                        "Add PDF files in the desired order to merge them into a single document:",
                    );
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        if ui.button("➕ Add PDF Files...").clicked() {
                            if let Some(files) = rfd::FileDialog::new()
                                .add_filter("PDF files", &["pdf"])
                                .pick_files()
                            {
                                self.combine_file_paths.extend(files);
                            }
                        }
                        if ui.button("🗑 Clear List").clicked() {
                            self.combine_file_paths.clear();
                        }
                    });
                    ui.separator();

                    let mut remove_idx = None;
                    let mut move_up = None;
                    let mut move_down = None;

                    ScrollArea::vertical().max_height(200.0).show(ui, |ui| {
                        if self.combine_file_paths.is_empty() {
                            ui.weak(
                                "(No files selected yet — click 'Add PDF Files' to select files)",
                            );
                        }
                        for (i, p) in self.combine_file_paths.iter().enumerate() {
                            ui.horizontal(|ui| {
                                let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("?");
                                ui.label(format!("{}. {}", i + 1, name));
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    if ui.small_button("✕").on_hover_text("Remove").clicked() {
                                        remove_idx = Some(i);
                                    }
                                    if i + 1 < self.combine_file_paths.len()
                                        && ui.small_button("▼").on_hover_text("Move down").clicked()
                                    {
                                        move_down = Some(i);
                                    }
                                    if i > 0
                                        && ui.small_button("▲").on_hover_text("Move up").clicked()
                                    {
                                        move_up = Some(i);
                                    }
                                });
                            });
                        }
                    });

                    if let Some(i) = remove_idx {
                        self.combine_file_paths.remove(i);
                    }
                    if let Some(i) = move_up {
                        self.combine_file_paths.swap(i, i - 1);
                    }
                    if let Some(i) = move_down {
                        self.combine_file_paths.swap(i, i + 1);
                    }

                    ui.separator();
                    ui.horizontal(|ui| {
                        let can_combine = self.combine_file_paths.len() >= 2;
                        if ui
                            .add_enabled(can_combine, Button::new("Combine Files"))
                            .clicked()
                        {
                            let mut docs = Vec::new();
                            let mut load_err = None;
                            for p in &self.combine_file_paths {
                                match PdfDocument::load_from_file(p) {
                                    Ok(d) => docs.push(d),
                                    Err(e) => {
                                        load_err =
                                            Some(format!("Failed to load {}: {}", p.display(), e));
                                        break;
                                    }
                                }
                            }
                            if let Some(err) = load_err {
                                self.status_message = err;
                            } else {
                                match merge_documents(&docs) {
                                    Ok(merged) => {
                                        let tab_id = self.next_tab_id;
                                        self.next_tab_id += 1;
                                        let title =
                                            format!("Merged Document ({} files).pdf", docs.len());
                                        let mut tab = DocumentTab::new(tab_id, title, merged, None);
                                        tab.is_modified = true;
                                        self.tabs.push(tab);
                                        self.active_tab_idx = Some(self.tabs.len() - 1);
                                        self.status_message = format!(
                                            "Combined {} documents successfully",
                                            docs.len()
                                        );
                                        close = true;
                                    }
                                    Err(e) => {
                                        self.status_message = format!("Merge error: {}", e);
                                    }
                                }
                            }
                        }
                        if ui.button("Cancel").clicked() {
                            close = true;
                        }
                    });
                });
            if close {
                self.show_combine_dialog = false;
            }
        }

        // 6. Split Document Dialog (FR-07.3)
        if self.show_split_dialog {
            let mut close = false;
            egui::Window::new("Split Document")
                .collapsible(false)
                .resizable(false)
                .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.label("Split the active document into separate PDF files by page ranges:");
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        ui.label("Page ranges:");
                        ui.text_edit_singleline(&mut self.split_ranges_input);
                    });
                    ui.weak("Example: '1-2, 3, 4-5' creates separate PDFs for each range.");
                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        if ui.button("Choose Folder & Split...").clicked() {
                            if let Some(tab) = self.active_tab() {
                                let total_pages = tab.doc.page_count();
                                let ranges =
                                    parse_page_ranges(&self.split_ranges_input, total_pages);
                                if ranges.is_empty() {
                                    self.status_message =
                                        "Invalid or empty page ranges specified".into();
                                } else if let Some(folder) = rfd::FileDialog::new().pick_folder() {
                                    match split_document(&tab.doc, &ranges) {
                                        Ok(docs) => {
                                            let mut count = 0;
                                            for (idx, doc) in docs.iter().enumerate() {
                                                let (s, e) = ranges[idx];
                                                let out_name = format!(
                                                    "split_pages_{}_to_{}.pdf",
                                                    s + 1,
                                                    e + 1
                                                );
                                                let out_path = folder.join(out_name);
                                                if doc.save_to_file(&out_path).is_ok() {
                                                    count += 1;
                                                }
                                            }
                                            self.status_message = format!(
                                                "Created {} split PDF files in '{}'",
                                                count,
                                                folder.display()
                                            );
                                            close = true;
                                        }
                                        Err(e) => {
                                            self.status_message = format!("Split error: {}", e);
                                        }
                                    }
                                }
                            }
                        }
                        if ui.button("Cancel").clicked() {
                            close = true;
                        }
                    });
                });
            if close {
                self.show_split_dialog = false;
            }
        }

        // 7. Document Properties Dialog (FR-33)
        if self.show_properties_dialog {
            let mut close = false;
            let mut apply = false;
            egui::Window::new("Document Properties")
                .collapsible(false)
                .resizable(true)
                .default_size([500.0, 420.0])
                .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.heading("General Metadata");
                    ui.add_space(4.0);
                    egui::Grid::new("prop_grid")
                        .num_columns(2)
                        .spacing([12.0, 8.0])
                        .show(ui, |ui| {
                            ui.label("Title:");
                            ui.text_edit_singleline(&mut self.prop_title);
                            ui.end_row();

                            ui.label("Author:");
                            ui.text_edit_singleline(&mut self.prop_author);
                            ui.end_row();

                            ui.label("Subject:");
                            ui.text_edit_singleline(&mut self.prop_subject);
                            ui.end_row();

                            ui.label("Keywords:");
                            ui.text_edit_singleline(&mut self.prop_keywords);
                            ui.end_row();
                        });

                    ui.separator();
                    ui.heading("File Metrics & Security");
                    ui.add_space(4.0);

                    if let Some(tab) = self.active_tab() {
                        let path_str = tab
                            .file_path
                            .as_ref()
                            .map(|p| p.display().to_string())
                            .unwrap_or_else(|| "<Unsaved Document>".into());
                        ui.label(format!("File Path: {}", path_str));
                        ui.label(format!("Page Count: {}", tab.doc.page_count()));
                        ui.label(format!(
                            "PDF Version: %PDF-{}.{}",
                            tab.doc.version.0, tab.doc.version.1
                        ));
                        if let Some(page) = tab.doc.get_page(tab.current_page) {
                            let w_pt = (page.crop_box[2] - page.crop_box[0]).abs();
                            let h_pt = (page.crop_box[3] - page.crop_box[1]).abs();
                            ui.label(format!(
                                "Page {} Size: {:.1} x {:.1} pt ({:.2}\" x {:.2}\")",
                                tab.current_page + 1,
                                w_pt,
                                h_pt,
                                w_pt / 72.0,
                                h_pt / 72.0
                            ));
                        }
                    }

                    ui.add_space(12.0);
                    ui.horizontal(|ui| {
                        if ui.button("💾 Save Metadata").clicked() {
                            apply = true;
                            close = true;
                        }
                        if ui.button("Cancel").clicked() {
                            close = true;
                        }
                    });
                });

            if apply {
                let info = ravpdf_tools::DocumentInfo {
                    title: if self.prop_title.trim().is_empty() {
                        None
                    } else {
                        Some(self.prop_title.clone())
                    },
                    author: if self.prop_author.trim().is_empty() {
                        None
                    } else {
                        Some(self.prop_author.clone())
                    },
                    subject: if self.prop_subject.trim().is_empty() {
                        None
                    } else {
                        Some(self.prop_subject.clone())
                    },
                    keywords: if self.prop_keywords.trim().is_empty() {
                        None
                    } else {
                        Some(self.prop_keywords.clone())
                    },
                    creator: Some("RAV PDF (ISO 32000 Conforming)".into()),
                    producer: Some("RAV PDF Engine".into()),
                    creation_date: None,
                    mod_date: None,
                };
                if let Some(tab) = self.active_tab_mut() {
                    let _ = set_document_info(&mut tab.doc, &info);
                    tab.metadata = Some(info);
                    tab.is_modified = true;
                    self.status_message = "Document properties updated successfully".into();
                }
            }
            if close {
                self.show_properties_dialog = false;
            }
        }

        // 8. Sanitize & Redact Patterns Dialog (FR-15.4)
        if self.show_redact_pattern_dialog {
            let mut close = false;
            let mut apply = false;
            egui::Window::new("Sanitize & Redact Patterns")
                .collapsible(false)
                .resizable(false)
                .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.label(
                        "Permanently search and sanitize sensitive patterns across the document:",
                    );
                    ui.add_space(6.0);

                    ui.label(RichText::new("Quick Presets:").strong());
                    ui.horizontal(|ui| {
                        if ui
                            .button("SSN")
                            .on_hover_text(r"\d{3}-\d{2}-\d{4}")
                            .clicked()
                        {
                            self.redact_pattern_input = r"\d{3}-\d{2}-\d{4}".into();
                        }
                        if ui
                            .button("Email")
                            .on_hover_text(r"[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}")
                            .clicked()
                        {
                            self.redact_pattern_input =
                                r"[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}".into();
                        }
                        if ui
                            .button("Phone")
                            .on_hover_text(r"\d{3}[-.]?\d{3}[-.]?\d{4}")
                            .clicked()
                        {
                            self.redact_pattern_input = r"\d{3}[-.]?\d{3}[-.]?\d{4}".into();
                        }
                        if ui
                            .button("Credit Card")
                            .on_hover_text(r"\b(?:\d{4}[ -]?){3}\d{4}\b")
                            .clicked()
                        {
                            self.redact_pattern_input = r"\b(?:\d{4}[ -]?){3}\d{4}\b".into();
                        }
                    });

                    ui.add_space(8.0);
                    egui::Grid::new("redact_grid")
                        .num_columns(2)
                        .spacing([12.0, 8.0])
                        .show(ui, |ui| {
                            ui.label("Pattern / Regex:");
                            ui.text_edit_singleline(&mut self.redact_pattern_input);
                            ui.end_row();

                            ui.label("Replacement Text:");
                            ui.text_edit_singleline(&mut self.redact_replacement_input);
                            ui.end_row();
                        });

                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        let can_redact = !self.redact_pattern_input.trim().is_empty();
                        if ui
                            .add_enabled(can_redact, Button::new("✂ Redact All Occurrences"))
                            .clicked()
                        {
                            apply = true;
                            close = true;
                        }
                        if ui.button("Cancel").clicked() {
                            close = true;
                        }
                    });
                });

            if apply {
                let pattern = self.redact_pattern_input.clone();
                let replacement = self.redact_replacement_input.clone();
                if let Some(tab) = self.active_tab_mut() {
                    match redact_pattern(&mut tab.doc, &pattern, &replacement) {
                        Ok(count) => {
                            tab.is_modified = true;
                            tab.invalidate_render();
                            self.status_message =
                                format!("Redacted {} occurrences of pattern '{}'", count, pattern);
                        }
                        Err(e) => {
                            self.status_message = format!("Redaction failed: {}", e);
                        }
                    }
                }
            }
            if close {
                self.show_redact_pattern_dialog = false;
            }
        }

        // 9. Password Protect & Encrypt Dialog (FR-14)
        if self.show_encrypt_dialog {
            let mut close = false;
            egui::Window::new("Password Protect & Encrypt")
                .collapsible(false)
                .resizable(false)
                .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.label("Encrypts the document using standard security handler (ISO 32000-1 / ISO 32000-2).");
                    ui.add_space(8.0);

                    ui.horizontal(|ui| {
                        ui.label("Algorithm:");
                        egui::ComboBox::from_id_salt("enc_algo_select")
                            .selected_text(match self.encrypt_algorithm_idx {
                                0 => "AES-256 (PDF 2.0 / Revision 6 Standard)",
                                _ => "AES-128 (PDF 1.6+ / Revision 4)",
                            })
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut self.encrypt_algorithm_idx, 0, "AES-256 (PDF 2.0 / Revision 6 Standard)");
                                ui.selectable_value(&mut self.encrypt_algorithm_idx, 1, "AES-128 (PDF 1.6+ / Revision 4)");
                            });
                    });
                    ui.add_space(6.0);

                    egui::Grid::new("enc_grid").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
                        ui.label("User Password (to open):");
                        ui.add(egui::TextEdit::singleline(&mut self.encrypt_user_pass).password(true));
                        ui.end_row();

                        ui.label("Owner Password (permissions):");
                        ui.add(egui::TextEdit::singleline(&mut self.encrypt_owner_pass).password(true));
                        ui.end_row();
                    });

                    ui.add_space(8.0);
                    ui.heading("Permissions");
                    ui.checkbox(&mut self.encrypt_allow_print, "Allow Printing");
                    ui.checkbox(&mut self.encrypt_allow_copy, "Allow Copying Text & Graphics");
                    ui.checkbox(&mut self.encrypt_allow_modify, "Allow Modifying Content");

                    ui.add_space(12.0);
                    ui.horizontal(|ui| {
                        if ui.button("🔒 Encrypt & Save As...").clicked() {
                            if let Some(tab) = self.active_tab() {
                                if let Some(path) = rfd::FileDialog::new()
                                    .add_filter("PDF files", &["pdf"])
                                    .save_file()
                                {
                                    let mut flags = -4;
                                    if !self.encrypt_allow_print {
                                        flags &= !Permissions::PRINT;
                                    }
                                    if !self.encrypt_allow_modify {
                                        flags &= !Permissions::MODIFY;
                                    }
                                    if !self.encrypt_allow_copy {
                                        flags &= !Permissions::COPY;
                                    }
                                    let perms = Permissions::new(flags);
                                    let mut enc_doc = tab.doc.clone();
                                    let algo = if self.encrypt_algorithm_idx == 0 {
                                        EncryptionAlgorithm::Aes256
                                    } else {
                                        EncryptionAlgorithm::Aes128
                                    };
                                    match encrypt_document_with_algorithm(&mut enc_doc, &self.encrypt_user_pass, &self.encrypt_owner_pass, perms, algo) {
                                        Ok(()) => {
                                            if let Err(e) = enc_doc.save_to_file(&path) {
                                                self.status_message = format!("Save error: {}", e);
                                            } else {
                                                self.status_message = format!("Encrypted PDF ({}) saved to '{}'", algo.display_name(), path.display());
                                                close = true;
                                            }
                                        }
                                        Err(e) => {
                                            self.status_message = format!("Encryption error: {}", e);
                                        }
                                    }
                                }
                            }
                        }
                        if ui.button("Cancel").clicked() {
                            close = true;
                        }
                    });
                });
            if close {
                self.show_encrypt_dialog = false;
            }
        }

        // 10. PDF Optimizer / Reduce File Size Dialog (FR-25.1)
        if self.show_optimize_dialog {
            let mut close = false;
            let mut apply = false;
            egui::Window::new("Reduce File Size & Optimize")
                .collapsible(false)
                .resizable(false)
                .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.label("Optimizes document streams using Flate compression and removes orphaned objects.");
                    ui.add_space(6.0);

                    if let Some(tab) = self.active_tab() {
                        let cur_len = tab.doc.to_bytes().map(|b| b.len()).unwrap_or(0);
                        ui.label(format!("Current File Size: {:.1} KB ({} bytes)", cur_len as f64 / 1024.0, cur_len));
                    }

                    if let Some(ref stats) = self.optimization_stats {
                        ui.separator();
                        ui.heading("Optimization Results:");
                        ui.label(format!("Original Size: {:.1} KB", stats.original_bytes as f64 / 1024.0));
                        ui.label(format!("Optimized Size: {:.1} KB", stats.optimized_bytes as f64 / 1024.0));
                        ui.label(RichText::new(format!(
                            "Saved: {:.1} KB ({:.1}% reduction)",
                            stats.saved_bytes() as f64 / 1024.0,
                            stats.reduction_percent()
                        )).strong().color(Color32::from_rgb(46, 160, 67)));
                        ui.label(format!("Streams Compressed: {}", stats.streams_compressed));
                        ui.label(format!("Orphaned Objects Discarded: {}", stats.objects_removed));
                    }

                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        if ui.button("⚡ Optimize Document").clicked() {
                            apply = true;
                        }
                        if ui.button("Close").clicked() {
                            close = true;
                        }
                    });
                });

            if apply {
                if let Some(tab) = self.active_tab_mut() {
                    match optimize_document(&mut tab.doc) {
                        Ok(stats) => {
                            tab.is_modified = true;
                            tab.invalidate_render();
                            self.status_message = format!(
                                "Optimization complete: {:.1}% reduction",
                                stats.reduction_percent()
                            );
                            self.optimization_stats = Some(stats);
                        }
                        Err(e) => {
                            self.status_message = format!("Optimization error: {}", e);
                        }
                    }
                }
            }
            if close {
                self.show_optimize_dialog = false;
                self.optimization_stats = None;
            }
        }

        // 11. Audit Space Usage Dialog (FR-25.9)
        if self.show_space_audit_dialog {
            let mut close = false;
            egui::Window::new("Audit Space Usage")
                .collapsible(false)
                .resizable(false)
                .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    if let Some(ref audit) = self.space_audit {
                        ui.heading("Space Breakdown by Component");
                        ui.add_space(6.0);

                        egui::Grid::new("audit_grid")
                            .num_columns(3)
                            .spacing([16.0, 8.0])
                            .show(ui, |ui| {
                                ui.label(RichText::new("Category").strong());
                                ui.label(RichText::new("Bytes").strong());
                                ui.label(RichText::new("% of Total").strong());
                                ui.end_row();

                                ui.label("Content Streams:");
                                ui.label(format!("{} bytes", audit.content_stream_bytes));
                                ui.label(format!("{:.1}%", audit.content_percent()));
                                ui.end_row();

                                ui.label("Images & Rasters:");
                                ui.label(format!("{} bytes", audit.image_bytes));
                                ui.label(format!("{:.1}%", audit.image_percent()));
                                ui.end_row();

                                ui.label("Fonts:");
                                ui.label(format!("{} bytes", audit.font_bytes));
                                ui.label(format!("{:.1}%", audit.font_percent()));
                                ui.end_row();

                                ui.label("Metadata & Info:");
                                ui.label(format!("{} bytes", audit.metadata_bytes));
                                ui.label(format!("{:.1}%", audit.metadata_percent()));
                                ui.end_row();

                                ui.label("Structure & Cross-Ref:");
                                ui.label(format!("{} bytes", audit.structure_bytes));
                                ui.label(format!("{:.1}%", audit.structure_percent()));
                                ui.end_row();

                                ui.label(RichText::new("Total Document Size:").strong());
                                ui.label(
                                    RichText::new(format!("{} bytes", audit.total_bytes)).strong(),
                                );
                                ui.label(RichText::new("100.0%").strong());
                                ui.end_row();
                            });
                    } else {
                        ui.label("No space audit data available.");
                    }

                    ui.add_space(10.0);
                    if ui.button("Close").clicked() {
                        close = true;
                    }
                });

            if close {
                self.show_space_audit_dialog = false;
            }
        }

        // 12. Document Comparison Dialog (FR-17)
        if self.show_compare_dialog {
            let mut close = false;
            let mut run_compare = false;
            egui::Window::new("Compare Two Documents")
                .collapsible(false)
                .resizable(true)
                .default_size([580.0, 460.0])
                .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.label("Compare two PDF files and generate a detailed change and text difference report.");
                    ui.add_space(8.0);

                    // Document A
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Document A (Original):").strong());
                        let name_a = self.compare_file_a.as_ref()
                            .and_then(|p| p.file_name())
                            .and_then(|n| n.to_str())
                            .unwrap_or("<Select File...>");
                        ui.label(name_a);
                        if ui.button("Browse...").clicked() {
                            if let Some(file) = rfd::FileDialog::new().add_filter("PDF files", &["pdf"]).pick_file() {
                                self.compare_file_a = Some(file);
                            }
                        }
                    });

                    // Document B
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Document B (Revised):").strong());
                        let name_b = self.compare_file_b.as_ref()
                            .and_then(|p| p.file_name())
                            .and_then(|n| n.to_str())
                            .unwrap_or("<Select File...>");
                        ui.label(name_b);
                        if ui.button("Browse...").clicked() {
                            if let Some(file) = rfd::FileDialog::new().add_filter("PDF files", &["pdf"]).pick_file() {
                                self.compare_file_b = Some(file);
                            }
                        }
                    });

                    ui.add_space(8.0);
                    let can_run = self.compare_file_a.is_some() && self.compare_file_b.is_some();
                    if ui.add_enabled(can_run, Button::new("🔍 Compare Documents")).clicked() {
                        run_compare = true;
                    }

                    if let Some(ref rep) = self.comparison_report {
                        ui.separator();
                        ui.heading("Comparison Summary");
                        if rep.is_identical() {
                            ui.colored_label(Color32::from_rgb(46, 160, 67), "✓ Documents are identical in text and metadata.");
                        } else {
                            ui.colored_label(
                                Color32::from_rgb(220, 100, 30),
                                format!("Found {} total difference(s). Document A has {} pages, Document B has {} pages.", rep.total_differences, rep.doc_a_pages, rep.doc_b_pages),
                            );

                            if !rep.metadata_diffs.is_empty() {
                                ui.label(RichText::new("Metadata Changes:").strong());
                                for md in &rep.metadata_diffs {
                                    ui.label(format!("• {}", md));
                                }
                            }

                            ui.add_space(6.0);
                            ui.label(RichText::new("Page-by-Page Content Differences:").strong());
                            ScrollArea::vertical().max_height(200.0).show(ui, |ui| {
                                for p in &rep.page_diffs {
                                    if p.has_differences {
                                        ui.horizontal(|ui| {
                                            ui.label(RichText::new(format!("Page {}:", p.page_number)).strong());
                                            ui.weak(&p.summary);
                                        });
                                        for add in &p.additions {
                                            ui.colored_label(Color32::from_rgb(46, 160, 67), format!("  + {}", add));
                                        }
                                        for del in &p.deletions {
                                            ui.colored_label(Color32::from_rgb(200, 50, 50), format!("  - {}", del));
                                        }
                                        ui.separator();
                                    }
                                }
                            });
                        }
                    }

                    ui.add_space(10.0);
                    if ui.button("Close").clicked() {
                        close = true;
                    }
                });

            if run_compare {
                if let (Some(ref pa), Some(ref pb)) = (&self.compare_file_a, &self.compare_file_b) {
                    match (
                        PdfDocument::load_from_file(pa),
                        PdfDocument::load_from_file(pb),
                    ) {
                        (Ok(da), Ok(db)) => {
                            self.comparison_report = Some(compare_documents(&da, &db));
                            self.status_message = "Comparison generated successfully".into();
                        }
                        (Err(e), _) => {
                            self.status_message = format!("Failed to load Document A: {}", e)
                        }
                        (_, Err(e)) => {
                            self.status_message = format!("Failed to load Document B: {}", e)
                        }
                    }
                }
            }
            if close {
                self.show_compare_dialog = false;
            }
        }

        // 13. Preflight & Standards Validation Dialog (FR-22, FR-23)
        if self.show_preflight_dialog {
            let mut close = false;
            let mut run_check = false;
            let mut execute_fixup = false;
            egui::Window::new("Preflight & Standards Validation")
                .collapsible(false)
                .resizable(true)
                .default_size([620.0, 500.0])
                .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.label("Audits the document against ISO standards and print production profiles.");
                    ui.add_space(8.0);

                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Validation Profile:").strong());
                        ui.selectable_value(&mut self.preflight_profile, PreflightProfile::PdfA1b, "PDF/A-1b Archival");
                        ui.selectable_value(&mut self.preflight_profile, PreflightProfile::PrintProduction, "Print Prepress");
                        ui.selectable_value(&mut self.preflight_profile, PreflightProfile::GeneralQuality, "General Quality");
                    });

                    ui.add_space(8.0);
                    if ui.button("⚡ Run Preflight Audit").clicked() {
                        run_check = true;
                    }

                    if let Some(ref rep) = self.preflight_report {
                        ui.separator();
                        ui.horizontal(|ui| {
                            if rep.is_compliant {
                                ui.colored_label(Color32::from_rgb(46, 160, 67), RichText::new("✓ COMPLIANT").strong().size(14.0));
                            } else {
                                ui.colored_label(Color32::from_rgb(220, 38, 38), RichText::new("✗ NON-COMPLIANT").strong().size(14.0));
                            }
                            ui.label(format!("(Passed: {}, Warnings: {}, Errors: {})", rep.total_passed, rep.total_warnings, rep.total_errors));
                        });

                        ui.add_space(6.0);
                        ScrollArea::vertical().max_height(280.0).show(ui, |ui| {
                            for check in &rep.checks {
                                ui.group(|ui| {
                                    ui.horizontal(|ui| {
                                        let (icon, color) = if check.passed {
                                            ("✓", Color32::from_rgb(46, 160, 67))
                                        } else {
                                            match check.severity {
                                                PreflightSeverity::Error => ("✗", Color32::from_rgb(220, 38, 38)),
                                                PreflightSeverity::Warning => ("⚠", Color32::from_rgb(230, 160, 0)),
                                                PreflightSeverity::Info => ("ℹ", Color32::from_rgb(60, 130, 246)),
                                            }
                                        };
                                        ui.colored_label(color, RichText::new(icon).strong().size(14.0));
                                        ui.label(RichText::new(&check.title).strong());
                                        ui.weak(format!("[{}]", check.category));
                                    });
                                    ui.label(&check.details);
                                });
                                ui.add_space(4.0);
                            }
                        });
                        if !rep.is_compliant {
                            ui.add_space(6.0);
                            if ui.button("🛠 Execute Automated Preflight Fixups").on_hover_text("Embeds standard fonts, normalizes color spaces, sets OutputIntent, and compresses uncompressed streams").clicked() {
                                execute_fixup = true;
                            }
                        }
                    }

                    ui.add_space(10.0);
                    if ui.button("Close").clicked() {
                        close = true;
                    }
                });

            if run_check {
                if let Some(tab) = self.active_tab() {
                    self.preflight_report = Some(run_preflight(&tab.doc, self.preflight_profile));
                    self.status_message = format!(
                        "Preflight audit completed for {}",
                        self.preflight_profile.display_name()
                    );
                }
            }
            if execute_fixup {
                let profile = self.preflight_profile;
                let mut status = None;
                let mut rep = None;
                if let Some(tab) = self.active_tab_mut() {
                    match execute_preflight_fixup(&mut tab.doc, PreflightFixup::FixAll) {
                        Ok(summary) => {
                            tab.is_modified = true;
                            tab.invalidate_render();
                            rep = Some(run_preflight(&tab.doc, profile));
                            status = Some(format!("Preflight Fixup completed: {}", summary));
                        }
                        Err(e) => {
                            status = Some(format!("Preflight Fixup error: {}", e));
                        }
                    }
                }
                if let Some(r) = rep {
                    self.preflight_report = Some(r);
                }
                if let Some(s) = status {
                    self.status_message = s;
                }
            }
            if close {
                self.show_preflight_dialog = false;
            }
        }

        // 14. Action Wizard / Batch Processing Dialog (FR-30)
        if self.show_batch_dialog {
            let mut close = false;
            let mut run_batch = false;
            egui::Window::new("Action Wizard & Batch Processing")
                .collapsible(false)
                .resizable(true)
                .default_size([640.0, 520.0])
                .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.label("Automate multiple document operations across batches of PDF files.");
                    ui.add_space(8.0);

                    // Input files
                    ui.horizontal(|ui| {
                        if ui.button("➕ Add PDF Files...").clicked() {
                            if let Some(files) = rfd::FileDialog::new()
                                .add_filter("PDF files", &["pdf"])
                                .pick_files()
                            {
                                self.batch_input_files.extend(files);
                            }
                        }
                        if ui.button("🗑 Clear Files").clicked() {
                            self.batch_input_files.clear();
                        }
                        ui.label(format!("({} files selected)", self.batch_input_files.len()));
                    });

                    // Output directory
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Output Folder:").strong());
                        let out_name = self
                            .batch_output_dir
                            .as_ref()
                            .map(|p| p.display().to_string())
                            .unwrap_or_else(|| "<Not Selected>".into());
                        ui.label(out_name);
                        if ui.button("Browse...").clicked() {
                            if let Some(folder) = rfd::FileDialog::new().pick_folder() {
                                self.batch_output_dir = Some(folder);
                            }
                        }
                    });

                    ui.separator();
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Templates:").strong());
                        if ui
                            .button("Distribution")
                            .on_hover_text("Sanitize metadata, flatten forms, optimize")
                            .clicked()
                        {
                            self.batch_action_watermark = false;
                            self.batch_action_bates = false;
                            self.batch_action_flatten = true;
                            self.batch_action_optimize = true;
                            self.batch_action_sanitize = true;
                            self.batch_action_linearize = false;
                        }
                        if ui
                            .button("Archiving")
                            .on_hover_text("Flatten forms, optimize")
                            .clicked()
                        {
                            self.batch_action_watermark = false;
                            self.batch_action_bates = false;
                            self.batch_action_flatten = true;
                            self.batch_action_optimize = true;
                            self.batch_action_sanitize = false;
                            self.batch_action_linearize = false;
                        }
                        if ui
                            .button("Web Optimize")
                            .on_hover_text("Optimize and linearize for fast web streaming")
                            .clicked()
                        {
                            self.batch_action_watermark = false;
                            self.batch_action_bates = false;
                            self.batch_action_flatten = false;
                            self.batch_action_optimize = true;
                            self.batch_action_sanitize = false;
                            self.batch_action_linearize = true;
                        }
                    });

                    ui.heading("Action Sequence to Apply:");
                    ui.checkbox(
                        &mut self.batch_action_optimize,
                        "Optimize & Flate Compress",
                    );
                    ui.checkbox(
                        &mut self.batch_action_linearize,
                        "Linearize for Fast Web Streaming",
                    );
                    ui.checkbox(
                        &mut self.batch_action_sanitize,
                        "Sanitize Document Metadata",
                    );
                    ui.checkbox(
                        &mut self.batch_action_flatten,
                        "Flatten All Interactive Forms",
                    );

                    ui.horizontal(|ui| {
                        ui.checkbox(&mut self.batch_action_watermark, "Add Watermark:");
                        if self.batch_action_watermark {
                            ui.text_edit_singleline(&mut self.batch_watermark_text);
                        }
                    });

                    ui.horizontal(|ui| {
                        ui.checkbox(
                            &mut self.batch_action_bates,
                            "Apply Bates Numbering:",
                        );
                        if self.batch_action_bates {
                            ui.label("Prefix:");
                            ui.text_edit_singleline(&mut self.batch_bates_prefix);
                        }
                    });

                    ui.add_space(8.0);
                    let can_run =
                        !self.batch_input_files.is_empty() && self.batch_output_dir.is_some();
                    if ui
                        .add_enabled(can_run, Button::new("▶ Execute Batch Sequence"))
                        .clicked()
                    {
                        run_batch = true;
                    }

                    if let Some(ref res) = self.batch_result {
                        ui.separator();
                        ui.heading("Execution Results:");
                        ui.label(format!(
                            "Total: {}, Succeeded: {}, Failed: {}",
                            res.total_files, res.successful_files, res.failed_files
                        ));

                        ScrollArea::vertical().max_height(160.0).show(ui, |ui| {
                            for f in &res.details {
                                ui.horizontal(|ui| {
                                    let fname =
                                        f.path.file_name().and_then(|n| n.to_str()).unwrap_or("?");
                                    if f.success {
                                        ui.colored_label(Color32::from_rgb(46, 160, 67), "✓");
                                        ui.label(fname);
                                    } else {
                                        ui.colored_label(Color32::from_rgb(220, 38, 38), "✗");
                                        ui.label(fname);
                                        if let Some(ref err) = f.error_message {
                                            ui.weak(format!("({})", err));
                                        }
                                    }
                                });
                            }
                        });
                    }

                    ui.add_space(10.0);
                    if ui.button("Close").clicked() {
                        close = true;
                    }
                });

            if run_batch {
                if let Some(ref out_dir) = self.batch_output_dir {
                    let mut actions = Vec::new();
                    if self.batch_action_sanitize {
                        actions.push(BatchAction::SanitizeMetadata);
                    }
                    if self.batch_action_watermark {
                        actions.push(BatchAction::Watermark {
                            text: self.batch_watermark_text.clone(),
                            opacity: 0.25,
                        });
                    }
                    if self.batch_action_bates {
                        actions.push(BatchAction::BatesNumbering {
                            prefix: self.batch_bates_prefix.clone(),
                            start_number: 1,
                            digits: 6,
                        });
                    }
                    if self.batch_action_flatten {
                        actions.push(BatchAction::FlattenForms);
                    }
                    if self.batch_action_optimize {
                        actions.push(BatchAction::Optimize);
                    }
                    if self.batch_action_linearize {
                        actions.push(BatchAction::Linearize);
                    }

                    let job = BatchJob {
                        input_files: self.batch_input_files.clone(),
                        output_directory: out_dir.clone(),
                        actions,
                    };
                    let result = execute_batch(&job);
                    self.status_message = format!(
                        "Batch execution finished: {}/{} succeeded",
                        result.successful_files, result.total_files
                    );
                    self.batch_result = Some(result);
                }
            }
            if close {
                self.show_batch_dialog = false;
            }
        }

        // Page Boundary Boxes & Crop Dialog (FR-06.8)
        if self.show_crop_dialog {
            let mut close = false;
            let mut apply = false;
            egui::Window::new(format!("Page Boundary Boxes - Page {}", self.crop_target_page + 1))
                .collapsible(false)
                .resizable(false)
                .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.label("Configure ISO 32000-1 page boundary box coordinates (points):");
                    ui.add_space(4.0);

                    ui.horizontal(|ui| {
                        ui.label("Box Type:");
                        egui::ComboBox::from_id_salt("page_box_type_combo")
                            .selected_text(match self.crop_box_type {
                                PageBoxType::CropBox => "CropBox (Visible Page Area)",
                                PageBoxType::MediaBox => "MediaBox (Physical Medium)",
                                PageBoxType::BleedBox => "BleedBox (Production Bleed)",
                                PageBoxType::TrimBox => "TrimBox (Finished Page Trim)",
                                PageBoxType::ArtBox => "ArtBox (Meaningful Content)",
                            })
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut self.crop_box_type, PageBoxType::CropBox, "CropBox (Visible Page Area)");
                                ui.selectable_value(&mut self.crop_box_type, PageBoxType::MediaBox, "MediaBox (Physical Medium)");
                                ui.selectable_value(&mut self.crop_box_type, PageBoxType::BleedBox, "BleedBox (Production Bleed)");
                                ui.selectable_value(&mut self.crop_box_type, PageBoxType::TrimBox, "TrimBox (Finished Page Trim)");
                                ui.selectable_value(&mut self.crop_box_type, PageBoxType::ArtBox, "ArtBox (Meaningful Content)");
                            });
                    });

                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        ui.label("Left (X1):");
                        ui.add(egui::DragValue::new(&mut self.crop_left).speed(1.0));
                        ui.label("Right (X2):");
                        ui.add(egui::DragValue::new(&mut self.crop_right).speed(1.0));
                    });

                    ui.horizontal(|ui| {
                        ui.label("Bottom (Y1):");
                        ui.add(egui::DragValue::new(&mut self.crop_bottom).speed(1.0));
                        ui.label("Top (Y2):");
                        ui.add(egui::DragValue::new(&mut self.crop_top).speed(1.0));
                    });

                    let width = (self.crop_right - self.crop_left).abs();
                    let height = (self.crop_top - self.crop_bottom).abs();
                    ui.weak(format!(
                        "Resulting dimensions: {:.1} × {:.1} pt ({:.2} × {:.2} in)",
                        width,
                        height,
                        width / 72.0,
                        height / 72.0
                    ));

                    ui.checkbox(
                        &mut self.crop_all_pages,
                        "Apply box settings to all pages in document",
                    );

                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        if ui.button("✂ Apply Box").clicked() {
                            apply = true;
                            close = true;
                        }
                        if ui.button("Cancel").clicked() {
                            close = true;
                        }
                    });
                });

            if apply {
                let rect = [
                    self.crop_left,
                    self.crop_bottom,
                    self.crop_right,
                    self.crop_top,
                ];
                let box_type = self.crop_box_type;
                let all_pages = self.crop_all_pages;
                let target_page = self.crop_target_page;
                let mut msg = String::new();

                if let Some(tab) = self.active_tab_mut() {
                    if all_pages {
                        let total = tab.doc.page_count();
                        for i in 0..total {
                            let _ = set_page_box(&mut tab.doc, i, box_type, rect);
                        }
                        msg = format!("Set {:?} for all {} pages", box_type, total);
                    } else {
                        let _ = set_page_box(&mut tab.doc, target_page, box_type, rect);
                        msg = format!("Set {:?} for page {}", box_type, target_page + 1);
                    }
                    tab.is_modified = true;
                    tab.invalidate_render();
                }

                if !msg.is_empty() {
                    self.status_message = msg;
                }
            }

            if close {
                self.show_crop_dialog = false;
            }
        }

        // Digital Signature Placement Dialog (FR-13.1, FR-13.10)
        if self.show_add_signature_dialog {
            let mut close = false;
            let mut execute_sig = false;
            egui::Window::new("Digital Signatures & Certificates")
                .collapsible(false)
                .resizable(false)
                .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.selectable_value(&mut self.sig_mode_idx, 0, "🔏 Cryptographic Digital ID (ISO 32000-1 Clause 12.8)");
                        ui.selectable_value(&mut self.sig_mode_idx, 1, "📋 Prepare Signature Field");
                    });
                    ui.separator();

                    if self.sig_mode_idx == 0 {
                        ui.label(RichText::new("Sign document with PKCS#7 / CMS detached cryptographic digital signature:").italics());
                        ui.horizontal(|ui| {
                            ui.label("Signer Full Name:");
                            ui.text_edit_singleline(&mut self.sig_signer_name);
                        });
                        ui.horizontal(|ui| {
                            ui.label("Organization:");
                            ui.text_edit_singleline(&mut self.sig_org);
                        });
                        ui.horizontal(|ui| {
                            ui.label("Signer Email:");
                            ui.text_edit_singleline(&mut self.sig_email);
                        });
                        ui.horizontal(|ui| {
                            ui.label("Reason:");
                            ui.text_edit_singleline(&mut self.sig_reason);
                        });
                        ui.horizontal(|ui| {
                            ui.label("Location:");
                            ui.text_edit_singleline(&mut self.sig_location);
                        });
                        ui.checkbox(&mut self.sig_is_visible, "Render Visible Signature Appearance");
                        ui.add_space(4.0);
                        ui.checkbox(&mut self.sig_is_certify, "Author Certification Signature (DocMDP)");
                        if self.sig_is_certify {
                            ui.horizontal(|ui| {
                                ui.label("Permitted Actions:");
                                egui::ComboBox::from_id_salt("sig_cert_perm")
                                    .selected_text(match self.sig_cert_perm_idx {
                                        0 => "No changes allowed (P=1)",
                                        1 => "Form filling and signing only (P=2)",
                                        _ => "Annotations, form filling, and signing allowed (P=3)",
                                    })
                                    .show_ui(ui, |ui| {
                                        ui.selectable_value(&mut self.sig_cert_perm_idx, 0, "No changes allowed (P=1)");
                                        ui.selectable_value(&mut self.sig_cert_perm_idx, 1, "Form filling and signing only (P=2)");
                                        ui.selectable_value(&mut self.sig_cert_perm_idx, 2, "Annotations, form filling, and signing allowed (P=3)");
                                    });
                            });
                        }
                        ui.checkbox(&mut self.sig_embed_ltv, "Embed Document Security Store (/DSS) for Long-Term Validation (LTV)");
                    } else {
                        ui.label(RichText::new("Place an interactive form signature field to be signed later:").italics());
                        ui.horizontal(|ui| {
                            ui.label("Field Name:");
                            ui.text_edit_singleline(&mut self.sig_field_name);
                        });
                        ui.horizontal(|ui| {
                            ui.label("Signer Name:");
                            ui.text_edit_singleline(&mut self.sig_signer_name);
                        });
                        ui.horizontal(|ui| {
                            ui.label("Reason:");
                            ui.text_edit_singleline(&mut self.sig_reason);
                        });
                        ui.horizontal(|ui| {
                            ui.label("Location:");
                            ui.text_edit_singleline(&mut self.sig_location);
                        });
                        ui.checkbox(&mut self.sig_is_visible, "Visible Signature Appearance Box");
                    }

                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        let btn_label = if self.sig_mode_idx == 0 {
                            "🔏 Sign Document with Digital ID"
                        } else {
                            "➕ Place Signature Field"
                        };
                        if ui.button(btn_label).clicked() {
                            execute_sig = true;
                            close = true;
                        }
                        if ui.button("Cancel").clicked() {
                            close = true;
                        }
                    });
                });

            if execute_sig {
                let target_page = self.sig_target_page;
                let mode = self.sig_mode_idx;
                let field_name = self.sig_field_name.clone();
                let signer = self.sig_signer_name.clone();
                let org = self.sig_org.clone();
                let email = self.sig_email.clone();
                let reason = self.sig_reason.clone();
                let location = self.sig_location.clone();
                let is_visible = self.sig_is_visible;
                let is_certify = self.sig_is_certify;
                let cert_perm_idx = self.sig_cert_perm_idx;
                let embed_ltv = self.sig_embed_ltv;
                let mut status = None;

                if let Some(tab) = self.active_tab_mut() {
                    let curr = target_page.min(tab.doc.page_count().saturating_sub(1));
                    let rect = if is_visible {
                        Some([72.0, 72.0, 260.0, 130.0])
                    } else {
                        None
                    };

                    if mode == 0 {
                        match DigitalId::generate(&signer, &org, &email, 365) {
                            Ok(digital_id) => {
                                let doc_mdp_perm = match cert_perm_idx {
                                    1 => DocMdpPermission::FormFilling,
                                    2 => DocMdpPermission::AnnotationsAndFormFilling,
                                    _ => DocMdpPermission::NoChanges,
                                };
                                let opts = SigningOptions {
                                    field_name: field_name.clone(),
                                    reason: reason.clone(),
                                    location: location.clone(),
                                    contact_info: Some(email.clone()),
                                    rect,
                                    is_certification: is_certify,
                                    certification_permission: if is_certify {
                                        Some(doc_mdp_perm)
                                    } else {
                                        None
                                    },
                                    embed_ltv,
                                };
                                match sign_pdf_document_advanced(
                                    tab.doc.clone(),
                                    curr,
                                    &digital_id,
                                    &opts,
                                ) {
                                    Ok(signed_bytes) => {
                                        match PdfDocument::load_from_bytes(&signed_bytes) {
                                            Ok(new_doc) => {
                                                tab.doc = new_doc;
                                                tab.raw_file_bytes = Some(signed_bytes);
                                                tab.is_modified = true;
                                                tab.invalidate_render();
                                                let cert_msg = if is_certify {
                                                    format!(
                                                        " [Certified: {}]",
                                                        doc_mdp_perm.description()
                                                    )
                                                } else {
                                                    String::new()
                                                };
                                                let ltv_msg =
                                                    if embed_ltv { " [LTV Enabled]" } else { "" };
                                                status = Some((
                                                    format!("Document digitally signed by '{}'{}{} (ISO 32000-1 Clause 12.8)", signer, cert_msg, ltv_msg),
                                                    true,
                                                ));
                                            }
                                            Err(e) => {
                                                status = Some((format!("Signature generated but re-reading PDF failed: {}", e), false));
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        status = Some((
                                            format!("Cryptographic signing failed: {}", e),
                                            false,
                                        ));
                                    }
                                }
                            }
                            Err(e) => {
                                status = Some((
                                    format!("Digital ID certificate generation failed: {}", e),
                                    false,
                                ));
                            }
                        }
                    } else {
                        match create_signature_field(
                            &mut tab.doc,
                            curr,
                            &field_name,
                            &signer,
                            &reason,
                            &location,
                            rect,
                        ) {
                            Ok(_) => {
                                tab.is_modified = true;
                                tab.invalidate_render();
                                status = Some((
                                    format!(
                                        "Created digital signature field '{}' on page {}",
                                        field_name,
                                        curr + 1
                                    ),
                                    true,
                                ));
                            }
                            Err(e) => {
                                status =
                                    Some((format!("Failed to create signature: {}", e), false));
                            }
                        }
                    }
                }

                if let Some((msg, success)) = status {
                    self.status_message = msg;
                    if success {
                        self.sidebar_tab = SidebarTab::Signatures;
                        self.sidebar_open = true;
                    }
                }
            }

            if close {
                self.show_add_signature_dialog = false;
            }
        }

        // Page Numbering & Labels Dialog (FR-06.9)
        if self.show_page_labels_dialog {
            let mut close = false;
            let mut apply = false;
            let mut clear = false;
            egui::Window::new("Page Numbering & Labels")
                .collapsible(false)
                .resizable(false)
                .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.label("Configure ISO 32000-1 Section 8.3.1 page numbering schemes:");
                    ui.add_space(4.0);

                    ui.horizontal(|ui| {
                        ui.label("Starting at page:");
                        ui.add(
                            egui::DragValue::new(&mut self.page_labels_start_page).range(1..=9999),
                        );
                    });

                    ui.horizontal(|ui| {
                        ui.label("Numbering Style:");
                        egui::ComboBox::from_id_salt("page_label_style_combo")
                            .selected_text(match self.page_labels_style_idx {
                                0 => "1, 2, 3, ... (Decimal)",
                                1 => "i, ii, iii, ... (Lowercase Roman)",
                                2 => "I, II, III, ... (Uppercase Roman)",
                                3 => "a, b, c, ... (Lowercase Letters)",
                                4 => "A, B, C, ... (Uppercase Letters)",
                                _ => "None (Prefix only)",
                            })
                            .show_ui(ui, |ui| {
                                ui.selectable_value(
                                    &mut self.page_labels_style_idx,
                                    0,
                                    "1, 2, 3, ... (Decimal)",
                                );
                                ui.selectable_value(
                                    &mut self.page_labels_style_idx,
                                    1,
                                    "i, ii, iii, ... (Lowercase Roman)",
                                );
                                ui.selectable_value(
                                    &mut self.page_labels_style_idx,
                                    2,
                                    "I, II, III, ... (Uppercase Roman)",
                                );
                                ui.selectable_value(
                                    &mut self.page_labels_style_idx,
                                    3,
                                    "a, b, c, ... (Lowercase Letters)",
                                );
                                ui.selectable_value(
                                    &mut self.page_labels_style_idx,
                                    4,
                                    "A, B, C, ... (Uppercase Letters)",
                                );
                                ui.selectable_value(
                                    &mut self.page_labels_style_idx,
                                    5,
                                    "None (Prefix only)",
                                );
                            });
                    });

                    ui.horizontal(|ui| {
                        ui.label("Prefix:");
                        ui.text_edit_singleline(&mut self.page_labels_prefix);
                    });

                    ui.horizontal(|ui| {
                        ui.label("Start numbering at:");
                        ui.add(
                            egui::DragValue::new(&mut self.page_labels_start_num).range(1..=9999),
                        );
                    });

                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        if ui.button("✓ Apply Labels").clicked() {
                            apply = true;
                            close = true;
                        }
                        if ui.button("🗑 Reset to Default").clicked() {
                            clear = true;
                            close = true;
                        }
                        if ui.button("Cancel").clicked() {
                            close = true;
                        }
                    });
                });

            if apply {
                let start_idx = self.page_labels_start_page.saturating_sub(1);
                let style = match self.page_labels_style_idx {
                    0 => Some(PageLabelStyle::Decimal),
                    1 => Some(PageLabelStyle::LowercaseRoman),
                    2 => Some(PageLabelStyle::UppercaseRoman),
                    3 => Some(PageLabelStyle::LowercaseLetters),
                    4 => Some(PageLabelStyle::UppercaseLetters),
                    _ => None,
                };
                let prefix = if self.page_labels_prefix.trim().is_empty() {
                    None
                } else {
                    Some(self.page_labels_prefix.trim().to_string())
                };
                let start_num = self.page_labels_start_num.max(1);

                let mut range = PageLabelRange::new(start_idx, style).with_start_number(start_num);
                if let Some(p) = prefix {
                    range = range.with_prefix(p);
                }

                let mut msg = None;
                if let Some(tab) = self.active_tab_mut() {
                    let mut existing = get_page_labels(&tab.doc).unwrap_or_default();
                    existing.retain(|r| r.start_page_index != start_idx);
                    existing.push(range);
                    existing.sort_by_key(|r| r.start_page_index);

                    match set_page_labels(&mut tab.doc, &existing) {
                        Ok(_) => {
                            tab.is_modified = true;
                            tab.invalidate_render();
                            msg = Some("Page labeling scheme applied".into());
                        }
                        Err(e) => {
                            msg = Some(format!("Failed to set page labels: {}", e));
                        }
                    }
                }
                if let Some(m) = msg {
                    self.status_message = m;
                }
            }

            if clear {
                if let Some(tab) = self.active_tab_mut() {
                    let _ = set_page_labels(&mut tab.doc, &[]);
                    tab.is_modified = true;
                    tab.invalidate_render();
                    self.status_message = "Page labels reset to standard numbers".into();
                }
            }

            if close {
                self.show_page_labels_dialog = false;
            }
        }

        // Add Hyperlink Dialog (FR-20.1)
        if self.show_link_dialog {
            let mut close = false;
            let mut insert = false;
            egui::Window::new("Add Hyperlink")
                .collapsible(false)
                .resizable(false)
                .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Link Action:");
                        ui.selectable_value(&mut self.link_type_idx, 0, "External Web URL");
                        ui.selectable_value(&mut self.link_type_idx, 1, "Go to Page");
                    });

                    if self.link_type_idx == 0 {
                        ui.horizontal(|ui| {
                            ui.label("Web URL:");
                            ui.text_edit_singleline(&mut self.link_url);
                        });
                    } else {
                        ui.horizontal(|ui| {
                            ui.label("Target Page:");
                            ui.add(
                                egui::DragValue::new(&mut self.link_target_page).range(1..=9999),
                            );
                        });
                    }

                    ui.horizontal(|ui| {
                        ui.label("Border Width (0 = invisible hotspot):");
                        ui.add(
                            egui::DragValue::new(&mut self.link_border_width)
                                .range(0.0..=5.0)
                                .speed(0.5),
                        );
                    });

                    ui.weak(format!(
                        "Region: [{:.1}, {:.1}, {:.1}, {:.1}] pt",
                        self.link_rect[0], self.link_rect[1], self.link_rect[2], self.link_rect[3]
                    ));

                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        if ui.button("🔗 Insert Link").clicked() {
                            insert = true;
                            close = true;
                        }
                        if ui.button("Cancel").clicked() {
                            close = true;
                        }
                    });
                });

            if insert {
                let rect = self.link_rect;
                let border_width = self.link_border_width;
                let is_url = self.link_type_idx == 0;
                let url = self.link_url.trim().to_string();
                let target_p = self.link_target_page.saturating_sub(1);

                let annot = if is_url {
                    let mut a = Annotation::new_link_uri(rect, url);
                    if let AnnotationType::Link {
                        border_width: ref mut bw,
                        ..
                    } = a.annot_type
                    {
                        *bw = border_width;
                    }
                    a
                } else {
                    let mut a = Annotation::new_link_goto(rect, target_p);
                    if let AnnotationType::Link {
                        border_width: ref mut bw,
                        ..
                    } = a.annot_type
                    {
                        *bw = border_width;
                    }
                    a
                };

                let mut msg = None;
                if let Some(tab) = self.active_tab_mut() {
                    let curr = tab.current_page;
                    match add_annotation_to_page(&mut tab.doc, curr, &annot) {
                        Ok(_) => {
                            tab.is_modified = true;
                            tab.invalidate_render();
                            msg = Some("Hyperlink created successfully".into());
                        }
                        Err(e) => {
                            msg = Some(format!("Failed to create link: {}", e));
                        }
                    }
                }
                if let Some(m) = msg {
                    self.status_message = m;
                }
            }

            if close {
                self.show_link_dialog = false;
            }
        }

        // Add Form Field Dialog (FR-11.1)
        if self.show_add_form_field_dialog {
            let mut close = false;
            let mut create_field = false;
            egui::Window::new("Add Interactive Form Field")
                .collapsible(false)
                .resizable(false)
                .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Field Type:");
                        egui::ComboBox::from_id_salt("form_field_type_combo")
                            .selected_text(match self.form_field_type_idx {
                                0 => "📝 Text Field (Tx)",
                                1 => "☑ Checkbox (Btn)",
                                2 => "🔘 Radio Button (Btn)",
                                3 => "▾ Dropdown Choice (Ch)",
                                4 => "📋 List Box (Ch)",
                                5 => "🔘 Push Button (Btn)",
                                _ => "📊 Barcode (Tx/Barcode)",
                            })
                            .show_ui(ui, |ui| {
                                ui.selectable_value(
                                    &mut self.form_field_type_idx,
                                    0,
                                    "📝 Text Field (Tx)",
                                );
                                ui.selectable_value(
                                    &mut self.form_field_type_idx,
                                    1,
                                    "☑ Checkbox (Btn)",
                                );
                                ui.selectable_value(
                                    &mut self.form_field_type_idx,
                                    2,
                                    "🔘 Radio Button (Btn)",
                                );
                                ui.selectable_value(
                                    &mut self.form_field_type_idx,
                                    3,
                                    "▾ Dropdown Choice (Ch)",
                                );
                                ui.selectable_value(
                                    &mut self.form_field_type_idx,
                                    4,
                                    "📋 List Box (Ch)",
                                );
                                ui.selectable_value(
                                    &mut self.form_field_type_idx,
                                    5,
                                    "🔘 Push Button (Btn)",
                                );
                                ui.selectable_value(
                                    &mut self.form_field_type_idx,
                                    6,
                                    "📊 Barcode (Tx/Barcode)",
                                );
                            });
                    });

                    ui.horizontal(|ui| {
                        ui.label("Field Name:");
                        ui.text_edit_singleline(&mut self.form_field_name);
                    });

                    match self.form_field_type_idx {
                        0 => {
                            ui.horizontal(|ui| {
                                ui.label("Default Text:");
                                ui.text_edit_singleline(&mut self.form_field_value);
                            });
                        }
                        1 => {
                            let mut checked = self.form_field_value == "Yes";
                            if ui.checkbox(&mut checked, "Checked by default").changed() {
                                self.form_field_value =
                                    if checked { "Yes".into() } else { "Off".into() };
                            }
                        }
                        2 => {
                            ui.horizontal(|ui| {
                                ui.label("Export Value:");
                                ui.text_edit_singleline(&mut self.form_field_radio_export);
                            });
                            let mut selected = self.form_field_value == "Yes";
                            if ui.checkbox(&mut selected, "Selected by default").changed() {
                                self.form_field_value =
                                    if selected { "Yes".into() } else { "Off".into() };
                            }
                        }
                        3 => {
                            ui.horizontal(|ui| {
                                ui.label("Options (comma-separated):");
                                ui.text_edit_singleline(&mut self.form_field_options_csv);
                            });
                            ui.horizontal(|ui| {
                                ui.label("Default Selected:");
                                ui.text_edit_singleline(&mut self.form_field_value);
                            });
                        }
                        4 => {
                            ui.horizontal(|ui| {
                                ui.label("Options (comma-separated):");
                                ui.text_edit_singleline(&mut self.form_field_options_csv);
                            });
                            ui.horizontal(|ui| {
                                ui.label("Default Selected:");
                                ui.text_edit_singleline(&mut self.form_field_value);
                            });
                            ui.checkbox(
                                &mut self.form_field_multiselect,
                                "Allow Multiple Selection",
                            );
                        }
                        5 => {
                            ui.horizontal(|ui| {
                                ui.label("Button Label:");
                                ui.text_edit_singleline(&mut self.form_field_value);
                            });
                        }
                        _ => {
                            ui.horizontal(|ui| {
                                ui.label("Barcode Symbology:");
                                egui::ComboBox::from_id_salt("barcode_symbology_combo")
                                    .selected_text(match self.form_field_barcode_symbology_idx {
                                        0 => "QR Code (ISO/IEC 18004)",
                                        1 => "Code 128",
                                        _ => "PDF417",
                                    })
                                    .show_ui(ui, |ui| {
                                        ui.selectable_value(
                                            &mut self.form_field_barcode_symbology_idx,
                                            0,
                                            "QR Code (ISO/IEC 18004)",
                                        );
                                        ui.selectable_value(
                                            &mut self.form_field_barcode_symbology_idx,
                                            1,
                                            "Code 128",
                                        );
                                        ui.selectable_value(
                                            &mut self.form_field_barcode_symbology_idx,
                                            2,
                                            "PDF417",
                                        );
                                    });
                            });
                            ui.horizontal(|ui| {
                                ui.label("Initial Barcode Data:");
                                ui.text_edit_singleline(&mut self.form_field_value);
                            });
                        }
                    }

                    ui.add_space(4.0);
                    ui.weak(format!(
                        "Dimensions: [{:.1}, {:.1}, {:.1}, {:.1}] pt",
                        self.form_field_rect[0],
                        self.form_field_rect[1],
                        self.form_field_rect[2],
                        self.form_field_rect[3]
                    ));

                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        if ui.button("➕ Create Field").clicked() {
                            create_field = true;
                            close = true;
                        }
                        if ui.button("Cancel").clicked() {
                            close = true;
                        }
                    });
                });

            if create_field {
                let rect = self.form_field_rect;
                let name = self.form_field_name.trim().to_string();
                let type_idx = self.form_field_type_idx;
                let val_str = self.form_field_value.clone();
                let opts_csv = self.form_field_options_csv.clone();
                let radio_export = self.form_field_radio_export.clone();
                let multiselect = self.form_field_multiselect;
                let barcode_sym = match self.form_field_barcode_symbology_idx {
                    0 => BarcodeSymbology::QrCode,
                    1 => BarcodeSymbology::Code128,
                    _ => BarcodeSymbology::Pdf417,
                };

                let field = match type_idx {
                    0 => {
                        let mut f = FormField::new_text(name, rect);
                        if !val_str.is_empty() {
                            f.value = Some(val_str);
                        }
                        f
                    }
                    1 => {
                        let checked = val_str == "Yes";
                        FormField::new_checkbox(name, rect, checked)
                    }
                    2 => {
                        let selected = val_str == "Yes";
                        FormField::new_radio_button(name, radio_export, rect, selected)
                    }
                    3 => {
                        let opts: Vec<String> = opts_csv
                            .split(',')
                            .map(|s| s.trim().to_string())
                            .filter(|s| !s.is_empty())
                            .collect();
                        let sel = if val_str.is_empty() {
                            opts.first().cloned()
                        } else {
                            Some(val_str)
                        };
                        FormField::new_choice(name, rect, opts, sel)
                    }
                    4 => {
                        let opts: Vec<String> = opts_csv
                            .split(',')
                            .map(|s| s.trim().to_string())
                            .filter(|s| !s.is_empty())
                            .collect();
                        let sel = if val_str.is_empty() {
                            opts.first().cloned()
                        } else {
                            Some(val_str)
                        };
                        FormField::new_listbox(name, rect, opts, sel, multiselect)
                    }
                    5 => {
                        let label = if val_str.is_empty() {
                            "Button"
                        } else {
                            &val_str
                        };
                        FormField::new_pushbutton(name, rect, label)
                    }
                    _ => {
                        let initial = if val_str.is_empty() {
                            None
                        } else {
                            Some(val_str)
                        };
                        FormField::new_barcode(name, rect, barcode_sym, initial)
                    }
                };

                let mut msg = None;
                if let Some(tab) = self.active_tab_mut() {
                    let curr = tab.current_page;
                    match add_form_field(&mut tab.doc, curr, &field) {
                        Ok(_) => {
                            tab.is_modified = true;
                            tab.invalidate_render();
                            msg = Some(format!(
                                "Created form field '{}' on page {}",
                                field.name,
                                curr + 1
                            ));
                        }
                        Err(e) => {
                            msg = Some(format!("Failed to create form field: {}", e));
                        }
                    }
                }
                if let Some(m) = msg {
                    self.status_message = m;
                    self.sidebar_tab = SidebarTab::Forms;
                    self.sidebar_open = true;
                }
            }

            if close {
                self.show_add_form_field_dialog = false;
            }
        }

        // 17. Accessibility Checker & Tagged PDF Dialog (FR-21.1 - FR-21.10)
        if self.show_accessibility_dialog {
            let mut close = false;
            let mut run_remediation = false;
            let mut reaudit = false;
            let mut fix_lang = None;
            let mut fix_title = None;
            let mut fix_tabs = false;
            let mut fix_tags = false;

            if self.accessibility_report.is_none() {
                if let Some(tab) = self.active_tab() {
                    self.accessibility_report = audit_accessibility(&tab.doc).ok();
                }
            }

            egui::Window::new("Accessibility Checker & PDF/UA Remediation")
                .collapsible(false)
                .resizable(true)
                .default_size([650.0, 520.0])
                .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.label("Audits document against PDF/UA (ISO 14289) and WCAG 2.1 accessibility standards.");
                    ui.add_space(6.0);

                    if let Some(ref report) = self.accessibility_report {
                        let (badge_col, status_lbl) = if report.is_compliant {
                            (Color32::from_rgb(46, 204, 113), "COMPLIANT WITH PDF/UA (ISO 14289)")
                        } else {
                            (Color32::from_rgb(231, 76, 60), "NON-COMPLIANT (Accessibility Violations Detected)")
                        };

                        egui::Frame::none()
                            .fill(Color32::from_rgb(25, 32, 45))
                            .stroke(Stroke::new(1.0_f32, Color32::from_gray(70)))
                            .rounding(6.0)
                            .inner_margin(egui::Margin::same(10.0))
                            .show(ui, |ui| {
                                ui.label(RichText::new(status_lbl).color(badge_col).strong().size(13.0));
                                ui.add_space(4.0);
                                ui.horizontal(|ui| {
                                    ui.label(RichText::new(format!("✓ Passed: {}", report.passed_count)).color(Color32::from_rgb(46, 204, 113)).strong());
                                    ui.label(RichText::new(format!("✕ Failed: {}", report.failed_count)).color(Color32::from_rgb(231, 76, 60)).strong());
                                    ui.label(RichText::new(format!("⚠ Manual Check: {}", report.manual_check_count)).color(Color32::from_rgb(241, 196, 15)).strong());
                                });
                            });

                        ui.add_space(8.0);
                        ui.horizontal(|ui| {
                            if ui.button("⚡ Remediate All Automatically").on_hover_text("Fix title, display preference, natural language, tags, and structure tab order in one click").clicked() {
                                run_remediation = true;
                            }
                            if ui.button("🔄 Re-Audit Document").clicked() {
                                reaudit = true;
                            }
                        });

                        ui.separator();
                        ui.heading("Audit Rules & Verification Items");
                        ui.add_space(4.0);

                        ScrollArea::vertical().max_height(280.0).show(ui, |ui| {
                            for rule in &report.rules {
                                let (icon, col) = match rule.status {
                                    AccessibilityStatus::Passed => ("✓", Color32::from_rgb(46, 204, 113)),
                                    AccessibilityStatus::Failed => ("✕", Color32::from_rgb(231, 76, 60)),
                                    AccessibilityStatus::NeedsManualCheck => ("⚠", Color32::from_rgb(241, 196, 15)),
                                };

                                egui::Frame::none()
                                    .fill(Color32::from_rgb(22, 27, 36))
                                    .stroke(Stroke::new(1.0_f32, Color32::from_gray(50)))
                                    .rounding(4.0)
                                    .inner_margin(egui::Margin::same(8.0))
                                    .show(ui, |ui| {
                                        ui.horizontal(|ui| {
                                            ui.label(RichText::new(icon).color(col).strong().size(14.0));
                                            ui.label(RichText::new(&rule.title).strong());
                                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                                ui.label(RichText::new(&rule.standard).weak().size(10.0));
                                            });
                                        });
                                        ui.label(RichText::new(&rule.description).weak().size(11.0));
                                        if rule.status == AccessibilityStatus::Failed {
                                            ui.horizontal(|ui| {
                                                ui.label(RichText::new(format!("Hint: {}", rule.remediation_hint)).color(Color32::from_rgb(226, 160, 60)).size(11.0));
                                                if rule.can_auto_fix {
                                                    if (rule.id == "PDF-UA-DocTitle" || rule.id == "PDF-UA-DisplayDocTitle")
                                                        && ui.small_button("Fix Title").clicked()
                                                    {
                                                        fix_title = Some(self.accessibility_fix_title_input.clone());
                                                    } else if rule.id == "PDF-UA-Lang"
                                                        && ui.small_button("Fix Language").clicked()
                                                    {
                                                        fix_lang = Some(self.accessibility_fix_lang_input.clone());
                                                    } else if rule.id == "PDF-UA-TabOrder"
                                                        && ui.small_button("Fix Tab Order").clicked()
                                                    {
                                                        fix_tabs = true;
                                                    } else if (rule.id == "PDF-UA-Marked" || rule.id == "PDF-UA-StructTreeRoot")
                                                        && ui.small_button("Fix Tag Tree").clicked()
                                                    {
                                                        fix_tags = true;
                                                    }
                                                }
                                            });
                                        }
                                    });
                                ui.add_space(4.0);
                            }
                        });
                    }

                    ui.separator();
                    ui.horizontal(|ui| {
                        if ui.button("Close").clicked() {
                            close = true;
                        }
                    });
                });

            if reaudit {
                if let Some(tab) = self.active_tab() {
                    self.accessibility_report = audit_accessibility(&tab.doc).ok();
                }
            }
            if run_remediation {
                let lang = self.accessibility_fix_lang_input.clone();
                let mut updated = None;
                if let Some(tab) = self.active_tab_mut() {
                    let title = tab.title.trim_end_matches(".pdf");
                    if let Ok(rep) = remediate_all_automatic(&mut tab.doc, title, &lang) {
                        tab.is_modified = true;
                        tab.invalidate_render();
                        updated = Some(rep);
                    }
                }
                if let Some(rep) = updated {
                    self.accessibility_report = Some(rep);
                    self.status_message = "Completed accessibility remediation".into();
                }
            }
            if let Some(t) = fix_title {
                let mut updated = None;
                if let Some(tab) = self.active_tab_mut() {
                    let _ = fix_document_title(&mut tab.doc, &t);
                    tab.is_modified = true;
                    updated = audit_accessibility(&tab.doc).ok();
                }
                if let Some(rep) = updated {
                    self.accessibility_report = Some(rep);
                }
            }
            if let Some(l) = fix_lang {
                let mut updated = None;
                if let Some(tab) = self.active_tab_mut() {
                    let _ = fix_document_language(&mut tab.doc, &l);
                    tab.is_modified = true;
                    updated = audit_accessibility(&tab.doc).ok();
                }
                if let Some(rep) = updated {
                    self.accessibility_report = Some(rep);
                }
            }
            if fix_tabs {
                let mut updated = None;
                if let Some(tab) = self.active_tab_mut() {
                    let _ = fix_tab_order_to_structure(&mut tab.doc);
                    tab.is_modified = true;
                    updated = audit_accessibility(&tab.doc).ok();
                }
                if let Some(rep) = updated {
                    self.accessibility_report = Some(rep);
                }
            }
            if fix_tags {
                let mut updated = None;
                if let Some(tab) = self.active_tab_mut() {
                    let _ = fix_ensure_tagged_pdf(&mut tab.doc);
                    tab.is_modified = true;
                    updated = audit_accessibility(&tab.doc).ok();
                }
                if let Some(rep) = updated {
                    self.accessibility_report = Some(rep);
                }
            }
            if close {
                self.show_accessibility_dialog = false;
            }
        }

        // 18. Export Document Dialog (FR-03.2.4 - FR-03.2.6, FR-03.2.9)
        if self.show_export_dialog {
            let mut close = false;
            let mut do_export = false;

            egui::Window::new("Export Document")
                .collapsible(false)
                .resizable(false)
                .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.label("Export PDF content to other formats preserving structure and high quality:");
                    ui.add_space(8.0);

                    ui.horizontal(|ui| {
                        ui.label("Format:");
                        egui::ComboBox::from_id_salt("export_fmt_combo")
                            .selected_text(match self.export_format_idx {
                                0 => "🌐 HTML5 Webpage (.html)",
                                1 => "📄 Plain Text Document (.txt)",
                                2 => "🖼 High-Res PNG Image (.png)",
                                _ => "🖼 Compressed JPEG Image (.jpg)",
                            })
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut self.export_format_idx, 0, "🌐 HTML5 Webpage (.html)");
                                ui.selectable_value(&mut self.export_format_idx, 1, "📄 Plain Text Document (.txt)");
                                ui.selectable_value(&mut self.export_format_idx, 2, "🖼 High-Res PNG Image (.png)");
                                ui.selectable_value(&mut self.export_format_idx, 3, "🖼 Compressed JPEG Image (.jpg)");
                            });
                    });

                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        ui.label("Page Range:");
                        ui.radio_value(&mut self.export_pages_all, true, "All Pages");
                        ui.radio_value(&mut self.export_pages_all, false, "Custom Range");
                    });

                    if !self.export_pages_all {
                        ui.horizontal(|ui| {
                            ui.label("Pages (e.g., 1-3, 5):");
                            ui.text_edit_singleline(&mut self.export_pages_range);
                        });
                    }

                    if self.export_format_idx == 2 || self.export_format_idx == 3 {
                        ui.horizontal(|ui| {
                            ui.label("Image DPI:");
                            ui.radio_value(&mut self.export_dpi, 72.0, "72 DPI");
                            ui.radio_value(&mut self.export_dpi, 150.0, "150 DPI");
                            ui.radio_value(&mut self.export_dpi, 300.0, "300 DPI (Print)");
                        });
                    }

                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        if ui.button("Choose Destination & Export...").clicked() {
                            do_export = true;
                        }
                        if ui.button("Cancel").clicked() {
                            close = true;
                        }
                    });
                });

            if do_export {
                if let Some(tab) = self.active_tab() {
                    let total_pages = tab.doc.page_count();
                    let page_indices: Option<Vec<usize>> = if self.export_pages_all {
                        None
                    } else {
                        let ranges = parse_page_ranges(&self.export_pages_range, total_pages);
                        let mut set = Vec::new();
                        for (s, e) in ranges {
                            for p in s..=e {
                                if !set.contains(&p) {
                                    set.push(p);
                                }
                            }
                        }
                        Some(set)
                    };

                    match self.export_format_idx {
                        0 => {
                            if let Some(dest) = rfd::FileDialog::new()
                                .add_filter("HTML Document", &["html", "htm"])
                                .save_file()
                            {
                                if let Ok(html) = export_to_html(&tab.doc, page_indices.as_deref())
                                {
                                    if std::fs::write(&dest, html).is_ok() {
                                        self.status_message =
                                            format!("Exported HTML to '{}'", dest.display());
                                        close = true;
                                    }
                                }
                            }
                        }
                        1 => {
                            if let Some(dest) = rfd::FileDialog::new()
                                .add_filter("Text File", &["txt"])
                                .save_file()
                            {
                                if let Ok(txt) =
                                    export_to_plain_text(&tab.doc, page_indices.as_deref())
                                {
                                    if std::fs::write(&dest, txt).is_ok() {
                                        self.status_message =
                                            format!("Exported text to '{}'", dest.display());
                                        close = true;
                                    }
                                }
                            }
                        }
                        2 | 3 => {
                            let fmt = if self.export_format_idx == 2 {
                                ExportImageFormat::Png
                            } else {
                                ExportImageFormat::Jpeg
                            };
                            let ext = if self.export_format_idx == 2 {
                                "png"
                            } else {
                                "jpg"
                            };

                            if let Some(folder) = rfd::FileDialog::new().pick_folder() {
                                let targets: Vec<usize> =
                                    page_indices.unwrap_or_else(|| (0..total_pages).collect());
                                let mut count = 0;
                                for p in &targets {
                                    if let Ok(bytes) =
                                        export_page_to_image(&tab.doc, *p, self.export_dpi, fmt)
                                    {
                                        let out_path =
                                            folder.join(format!("page_{}.{}", p + 1, ext));
                                        if std::fs::write(&out_path, bytes).is_ok() {
                                            count += 1;
                                        }
                                    }
                                }
                                self.status_message =
                                    format!("Exported {} images to '{}'", count, folder.display());
                                close = true;
                            }
                        }
                        _ => {}
                    }
                }
            }

            if close {
                self.show_export_dialog = false;
            }
        }

        // 19. Comments & Review Summary Dialog (FR-31.5)
        if self.show_comments_summary_dialog {
            let mut close = false;
            let mut do_csv = false;

            egui::Window::new("Comments & Review Summary")
                .collapsible(false)
                .resizable(true)
                .default_size([600.0, 420.0])
                .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.label("Review markup, sticky notes, annotations, and reviewer comments in document.");
                    ui.add_space(6.0);

                    ui.horizontal(|ui| {
                        ui.label(RichText::new(format!("Total Comments: {}", self.comments_list.len())).strong());
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if ui.button("📥 Export Comments as CSV").clicked() {
                                do_csv = true;
                            }
                        });
                    });
                    ui.separator();

                    if self.comments_list.is_empty() {
                        ui.vertical_centered(|ui| {
                            ui.add_space(20.0);
                            ui.weak("No annotations or comments found in current document.");
                        });
                    } else {
                        ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
                            for (idx, c) in self.comments_list.iter().enumerate() {
                                egui::Frame::none()
                                    .fill(Color32::from_rgb(22, 28, 38))
                                    .stroke(Stroke::new(1.0_f32, Color32::from_gray(60)))
                                    .rounding(4.0)
                                    .inner_margin(egui::Margin::same(8.0))
                                    .show(ui, |ui| {
                                        ui.horizontal(|ui| {
                                            ui.label(RichText::new(format!("#{}. Page {}", idx + 1, c.page_index + 1)).strong());
                                            ui.label(RichText::new(format!("Type: {}", c.comment_type)).color(Color32::from_rgb(66, 153, 225)).size(11.0));
                                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                                ui.label(RichText::new(&c.author).italics().size(11.0));
                                            });
                                        });
                                        if !c.contents.is_empty() {
                                            ui.add_space(2.0);
                                            ui.label(&c.contents);
                                        }
                                    });
                                ui.add_space(4.0);
                            }
                        });
                    }

                    ui.separator();
                    if ui.button("Close").clicked() {
                        close = true;
                    }
                });

            if do_csv {
                if let Some(dest) = rfd::FileDialog::new()
                    .add_filter("CSV File", &["csv"])
                    .save_file()
                {
                    let csv_content = export_comments_to_csv(&self.comments_list);
                    if std::fs::write(&dest, csv_content).is_ok() {
                        self.status_message =
                            format!("Exported comments CSV to '{}'", dest.display());
                        close = true;
                    }
                }
            }

            if close {
                self.show_comments_summary_dialog = false;
            }
        }

        // 20. Merge Comments from Reviewer Copy Dialog (FR-31.3)
        if self.show_merge_comments_dialog {
            let mut close = false;

            egui::Window::new("Merge Reviewer Comments")
                .collapsible(false)
                .resizable(false)
                .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.label("Import annotations, stamps, notes, and highlights from a reviewer copy into this master PDF:");
                    ui.add_space(8.0);

                    if ui.button("📂 Select Reviewer PDF Copy...").clicked() {
                        if let Some(reviewer_path) = rfd::FileDialog::new()
                            .add_filter("PDF Document", &["pdf"])
                            .pick_file()
                        {
                            match PdfDocument::load_from_file(&reviewer_path) {
                                Ok(reviewer_doc) => {
                                    if let Some(tab) = self.active_tab_mut() {
                                        match merge_reviewer_annotations(&mut tab.doc, &reviewer_doc) {
                                            Ok(count) => {
                                                tab.is_modified = true;
                                                tab.invalidate_render();
                                                self.status_message = format!(
                                                    "Successfully merged {} new comments/annotations from '{}'",
                                                    count,
                                                    reviewer_path.display()
                                                );
                                                close = true;
                                            }
                                            Err(e) => {
                                                self.status_message = format!("Merge comments error: {}", e);
                                            }
                                        }
                                    }
                                }
                                Err(e) => {
                                    self.status_message = format!("Failed to read reviewer PDF: {}", e);
                                }
                            }
                        }
                    }

                    ui.add_space(10.0);
                    if ui.button("Cancel").clicked() {
                        close = true;
                    }
                });

            if close {
                self.show_merge_comments_dialog = false;
            }
        }

        // 21. Measurement Scale & Calibration Dialog (FR-26.5)
        if self.show_measure_scale_dialog {
            let mut close = false;

            egui::Window::new("Measurement Scale & Calibration")
                .collapsible(false)
                .resizable(false)
                .default_size([460.0, 400.0])
                .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.label("Configure scale ratio, drawing units, and geometric snapping for dimension tools:");
                    ui.add_space(8.0);

                    ui.group(|ui| {
                        ui.label(RichText::new("Scale Ratio Presets").strong());
                        let presets = [
                            ("1:1 (Full Size)", ScaleRatio::default()),
                            ("1:20", ScaleRatio::dimensionless(20.0)),
                            ("1:50", ScaleRatio::dimensionless(50.0)),
                            ("1:100 (Metric Architectural)", ScaleRatio::dimensionless(100.0)),
                            ("1:200", ScaleRatio::dimensionless(200.0)),
                            ("1:500 (Civil / Site Plan)", ScaleRatio::dimensionless(500.0)),
                            ("1/4\" = 1' (1:48 US Architectural)", ScaleRatio::new(0.25, LinearUnit::Inches, 1.0, LinearUnit::Feet)),
                            ("1/8\" = 1' (1:96)", ScaleRatio::new(0.125, LinearUnit::Inches, 1.0, LinearUnit::Feet)),
                            ("1\" = 10' (1:120 Engineering)", ScaleRatio::architectural_inch_to_feet(10.0)),
                            ("1\" = 20' (1:240)", ScaleRatio::architectural_inch_to_feet(20.0)),
                            ("1 cm = 1 m (1:100)", ScaleRatio::new(1.0, LinearUnit::Centimeters, 1.0, LinearUnit::Meters)),
                            ("1 mm = 1 m (1:1000)", ScaleRatio::new(1.0, LinearUnit::Millimeters, 1.0, LinearUnit::Meters)),
                            ("Custom", ScaleRatio::new(self.measure_scale_custom_drawing_val, self.measure_unit, self.measure_scale_custom_world_val, self.measure_unit)),
                        ];

                        egui::ComboBox::from_label("Preset")
                            .selected_text(presets.get(self.measure_scale_preset_idx).map(|(name, _)| *name).unwrap_or("Custom"))
                            .show_ui(ui, |ui| {
                                for (idx, (name, ratio)) in presets.iter().enumerate() {
                                    if ui.selectable_value(&mut self.measure_scale_preset_idx, idx, *name).clicked() {
                                        self.measure_scale = ratio.clone();
                                    }
                                }
                            });

                        if self.measure_scale_preset_idx == 12 {
                            ui.horizontal(|ui| {
                                ui.label("Drawing:");
                                if ui.add(egui::DragValue::new(&mut self.measure_scale_custom_drawing_val).speed(0.1).range(0.01..=1000.0)).changed() {
                                    self.measure_scale.drawing_value = self.measure_scale_custom_drawing_val;
                                }
                                ui.label(self.measure_unit.abbreviation());

                                ui.label("=");

                                ui.label("World:");
                                if ui.add(egui::DragValue::new(&mut self.measure_scale_custom_world_val).speed(0.1).range(0.01..=100000.0)).changed() {
                                    self.measure_scale.world_value = self.measure_scale_custom_world_val;
                                }
                                ui.label(self.measure_unit.abbreviation());
                            });
                        }

                        ui.add_space(4.0);
                        ui.label(RichText::new(format!("Current Ratio: {}", self.measure_scale.to_display_string())).color(Color32::from_rgb(0, 180, 216)));
                    });

                    ui.add_space(6.0);
                    ui.group(|ui| {
                        ui.label(RichText::new("Display Units").strong());
                        ui.horizontal(|ui| {
                            ui.label("Distance Unit:");
                            egui::ComboBox::from_id_salt("linear_unit_combo")
                                .selected_text(self.measure_unit.abbreviation())
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(&mut self.measure_unit, LinearUnit::Inches, "Inches (in)");
                                    ui.selectable_value(&mut self.measure_unit, LinearUnit::Feet, "Feet (ft)");
                                    ui.selectable_value(&mut self.measure_unit, LinearUnit::Millimeters, "Millimeters (mm)");
                                    ui.selectable_value(&mut self.measure_unit, LinearUnit::Centimeters, "Centimeters (cm)");
                                    ui.selectable_value(&mut self.measure_unit, LinearUnit::Meters, "Meters (m)");
                                    ui.selectable_value(&mut self.measure_unit, LinearUnit::Yards, "Yards (yd)");
                                    ui.selectable_value(&mut self.measure_unit, LinearUnit::Points, "Points (pt)");
                                });

                            ui.add_space(10.0);
                            ui.label("Area Unit:");
                            egui::ComboBox::from_id_salt("area_unit_combo")
                                .selected_text(self.measure_area_unit.abbreviation())
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(&mut self.measure_area_unit, AreaUnit::SquareFeet, "Square Feet (sq ft)");
                                    ui.selectable_value(&mut self.measure_area_unit, AreaUnit::SquareMeters, "Square Meters (sq m)");
                                    ui.selectable_value(&mut self.measure_area_unit, AreaUnit::SquareInches, "Square Inches (sq in)");
                                    ui.selectable_value(&mut self.measure_area_unit, AreaUnit::SquareCentimeters, "Square Centimeters (sq cm)");
                                    ui.selectable_value(&mut self.measure_area_unit, AreaUnit::Acres, "Acres (ac)");
                                });
                        });
                    });

                    ui.add_space(6.0);
                    ui.group(|ui| {
                        ui.label(RichText::new("Reference Calibration").strong());
                        ui.label("Calibrate scale from a known distance on paper (e.g. 10 ft doorway):");
                        ui.horizontal(|ui| {
                            ui.label("Known Distance:");
                            ui.add(egui::DragValue::new(&mut self.measure_calib_real_val).speed(0.1).range(0.01..=100000.0));
                            ui.label(self.measure_unit.abbreviation());

                            if ui.button("🎯 Calibrate Scale").clicked() {
                                self.measure_scale = ScaleRatio::new(1.0, LinearUnit::Inches, self.measure_calib_real_val, self.measure_unit);
                                self.status_message = format!("Calibrated scale: {}", self.measure_scale.to_display_string());
                            }
                        });
                    });

                    ui.add_space(6.0);
                    ui.group(|ui| {
                        ui.label(RichText::new("Snapping & Annotations").strong());
                        ui.horizontal(|ui| {
                            ui.checkbox(&mut self.measure_snap.snap_endpoints, "Endpoints");
                            ui.checkbox(&mut self.measure_snap.snap_midpoints, "Midpoints");
                            ui.checkbox(&mut self.measure_snap.snap_intersections, "Intersections");
                        });
                        ui.add_space(2.0);
                        ui.checkbox(&mut self.measure_bake_as_annot, "Bake dimension lines into PDF as annotations");
                    });

                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        if ui.button("Done").clicked() {
                            close = true;
                        }
                    });
                });

            if close {
                self.show_measure_scale_dialog = false;
            }
        }

        // 22. PDF Portfolio Studio Dialog (FR-29.2.1)
        if self.show_portfolio_dialog {
            let mut close = false;
            let mut create_portfolio_action = false;

            egui::Window::new("Create PDF Portfolio")
                .collapsible(false)
                .resizable(true)
                .default_size([540.0, 420.0])
                .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.label("Bundle multiple heterogeneous files (PDFs, spreadsheets, images, source files) into a unified PDF Portfolio container:");
                    ui.add_space(8.0);

                    ui.horizontal(|ui| {
                        ui.label("Portfolio Title:");
                        ui.text_edit_singleline(&mut self.portfolio_title_input);
                    });

                    ui.horizontal(|ui| {
                        ui.label("Description:");
                        ui.text_edit_singleline(&mut self.portfolio_desc_input);
                    });

                    ui.horizontal(|ui| {
                        ui.label("Default View Layout:");
                        ui.radio_value(&mut self.portfolio_layout_idx, 0, "Tile View (Cards)");
                        ui.radio_value(&mut self.portfolio_layout_idx, 1, "Details View (Table)");
                    });

                    ui.separator();

                    ui.horizontal(|ui| {
                        ui.label(RichText::new(format!("Embedded Files ({})", self.portfolio_input_paths.len())).strong());
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if ui.button("➕ Add Files...").clicked() {
                                if let Some(files) = rfd::FileDialog::new().pick_files() {
                                    self.portfolio_input_paths.extend(files);
                                }
                            }
                        });
                    });

                    let mut remove_idx = None;
                    ScrollArea::vertical().max_height(180.0).show(ui, |ui| {
                        if self.portfolio_input_paths.is_empty() {
                            ui.weak("No files added yet. Click 'Add Files...' to choose documents to bundle.");
                        } else {
                            for (idx, path) in self.portfolio_input_paths.iter().enumerate() {
                                ui.horizontal(|ui| {
                                    let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("file");
                                    ui.label(format!("{}. {}", idx + 1, filename));
                                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                        if ui.small_button("🗑").clicked() {
                                            remove_idx = Some(idx);
                                        }
                                    });
                                });
                            }
                        }
                    });

                    if let Some(idx) = remove_idx {
                        self.portfolio_input_paths.remove(idx);
                    }

                    ui.separator();
                    ui.horizontal(|ui| {
                        let can_create = !self.portfolio_input_paths.is_empty();
                        ui.add_enabled_ui(can_create, |ui| {
                            if ui.button("🗂 Create PDF Portfolio").clicked() {
                                create_portfolio_action = true;
                            }
                        });

                        if ui.button("Cancel").clicked() {
                            close = true;
                        }
                    });
                });

            if create_portfolio_action {
                if let Some(out_path) = rfd::FileDialog::new()
                    .add_filter("PDF Portfolio", &["pdf"])
                    .set_file_name("portfolio.pdf")
                    .save_file()
                {
                    let mut items = Vec::new();
                    for path in &self.portfolio_input_paths {
                        if let Ok(item) = PortfolioItem::from_file_path(path, None) {
                            items.push(item);
                        }
                    }

                    let layout = match self.portfolio_layout_idx {
                        1 => PortfolioLayout::Details,
                        _ => PortfolioLayout::Tile,
                    };

                    let desc_opt = if self.portfolio_desc_input.trim().is_empty() {
                        None
                    } else {
                        Some(self.portfolio_desc_input.trim())
                    };

                    match create_portfolio(&items, layout, &self.portfolio_title_input, desc_opt) {
                        Ok(port_doc) => {
                            if let Ok(()) = port_doc.save_to_file(&out_path) {
                                self.status_message = format!(
                                    "Created PDF Portfolio '{}' with {} files",
                                    out_path.display(),
                                    items.len()
                                );
                                close = true;
                            }
                        }
                        Err(e) => {
                            self.status_message = format!("Portfolio creation failed: {}", e);
                        }
                    }
                }
            }

            if close {
                self.show_portfolio_dialog = false;
            }
        }

        // 23. Fast Web View / Linearization Dialog (FR-02.8, FR-25.8)
        if self.show_linearize_dialog {
            let mut close = false;
            let mut do_linearize = false;

            egui::Window::new("Linearize for Fast Web View")
                .collapsible(false)
                .resizable(false)
                .default_size([460.0, 240.0])
                .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.label("Linearizing reorganizes document objects and hint tables to allow web browsers and PDF readers to display the first page immediately before the entire file finishes downloading.");
                    ui.add_space(8.0);

                    if let Some(tab) = self.active_tab() {
                        let bytes = tab.doc.to_bytes().unwrap_or_default();
                        let linearized = is_linearized(&bytes);
                        ui.horizontal(|ui| {
                            ui.label("Current Document Status:");
                            if linearized {
                                ui.label(RichText::new("✔ Web-Optimized (Linearized)").color(Color32::from_rgb(0, 200, 100)).strong());
                            } else {
                                ui.label(RichText::new("✖ Standard PDF (Not Linearized)").color(Color32::from_rgb(220, 140, 0)).strong());
                            }
                        });
                    }

                    ui.add_space(10.0);
                    ui.separator();
                    ui.horizontal(|ui| {
                        if ui.button("⚡ Linearize & Save...").clicked() {
                            do_linearize = true;
                        }
                        if ui.button("Close").clicked() {
                            close = true;
                        }
                    });
                });

            if do_linearize {
                if let Some(tab) = self.active_tab() {
                    match linearize_document(&tab.doc) {
                        Ok(lin_bytes) => {
                            if let Some(dest) = rfd::FileDialog::new()
                                .add_filter("Linearized PDF Document", &["pdf"])
                                .set_file_name("web_optimized.pdf")
                                .save_file()
                            {
                                if std::fs::write(&dest, lin_bytes).is_ok() {
                                    self.status_message =
                                        format!("Linearized PDF saved to '{}'", dest.display());
                                    close = true;
                                }
                            }
                        }
                        Err(e) => {
                            self.status_message = format!("Linearization failed: {}", e);
                        }
                    }
                }
            }

            if close {
                self.show_linearize_dialog = false;
            }
        }

        // 24. JavaScript Console & Debugger Dialog (FR-12.9)
        if self.show_js_console_dialog {
            let mut close = false;
            let mut run_script = false;
            let mut run_calcs = false;
            let mut clear_log = false;

            egui::Window::new("JavaScript Console & Debugger")
                .open(&mut self.show_js_console_dialog)
                .default_size([650.0, 480.0])
                .resizable(true)
                .show(ctx, |ui| {
                    ui.label(RichText::new("Acrobat JavaScript Sandbox Environment").strong());
                    ui.label("Execute document/field scripts, test calculations, and inspect form objects (ISO 32000-1 Clause 12.6.4.16).");
                    ui.separator();

                    ui.label(RichText::new("Console Output:").strong());
                    ScrollArea::vertical().max_height(180.0).show(ui, |ui| {
                        ui.vertical(|ui| {
                            if self.js_console_output.is_empty() {
                                ui.label(RichText::new("// Console ready. Type a script below and click Execute.").italics().color(Color32::GRAY));
                            } else {
                                for line in &self.js_console_output {
                                    ui.monospace(line);
                                }
                            }
                        });
                    });

                    ui.separator();
                    ui.label(RichText::new("Script Input:").strong());
                    ui.add(egui::TextEdit::multiline(&mut self.js_console_input).font(egui::TextStyle::Monospace).desired_rows(5).desired_width(f32::INFINITY));

                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui.button("▶ Execute Script").clicked() {
                            run_script = true;
                        }
                        if ui.button("⚡ Calculate All Form Fields").clicked() {
                            run_calcs = true;
                        }
                        if ui.button("🗑 Clear Console").clicked() {
                            clear_log = true;
                        }
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if ui.button("Close").clicked() {
                                close = true;
                            }
                        });
                    });
                });

            if clear_log {
                self.js_console_output.clear();
            }

            if run_script {
                let script = self.js_console_input.clone();
                let mut log_entries = Vec::new();
                if let Some(idx) = self.active_tab_idx {
                    if let Some(tab) = self.tabs.get_mut(idx) {
                        let mut js_ctx = JsExecutionContext::new();
                        let _ = js_ctx.sync_from_document(&tab.doc);
                        match execute_script(&mut js_ctx, &script) {
                            Ok(res) => {
                                log_entries.push(format!("> {}", script.trim()));
                                for msg in &js_ctx.console_log {
                                    log_entries.push(format!("[Console] {}", msg));
                                }
                                for alert in &js_ctx.alerts {
                                    log_entries.push(format!(
                                        "[Alert: {}] {}",
                                        alert.title, alert.message
                                    ));
                                }
                                log_entries.push(format!("=> {:?}", res));
                                let _ = js_ctx.sync_to_document(&mut tab.doc);
                                tab.is_modified = true;
                                tab.invalidate_render();
                            }
                            Err(e) => {
                                log_entries.push(format!("❌ Error: {}", e));
                            }
                        }
                    }
                } else {
                    log_entries.push("❌ Error: No document open".to_string());
                }
                self.js_console_output.extend(log_entries);
            }

            if run_calcs {
                let mut log_entries = Vec::new();
                if let Some(idx) = self.active_tab_idx {
                    if let Some(tab) = self.tabs.get_mut(idx) {
                        let mut js_ctx = JsExecutionContext::new();
                        let _ = js_ctx.sync_from_document(&tab.doc);
                        match execute_all_field_calculations(&mut js_ctx) {
                            Ok(count) => {
                                let _ = js_ctx.sync_to_document(&mut tab.doc);
                                tab.is_modified = true;
                                tab.invalidate_render();
                                log_entries.push(format!(
                                    "⚡ Evaluated calculations for {} form fields",
                                    count
                                ));
                            }
                            Err(e) => {
                                log_entries.push(format!("❌ Calculation Error: {}", e));
                            }
                        }
                    }
                }
                self.js_console_output.extend(log_entries);
            }

            if close {
                self.show_js_console_dialog = false;
            }
        }

        // 25. Scanning & Document Enhancement Dialog (FR-32)
        if self.show_scan_dialog {
            let mut close = false;
            let mut pick_scans = false;
            let mut process_scans = false;

            egui::Window::new("Scan & Enhance Pages")
                .open(&mut self.show_scan_dialog)
                .default_size([540.0, 420.0])
                .resizable(true)
                .show(ctx, |ui| {
                    ui.label(RichText::new("Scan Enhancement Pipeline (TWAIN / Flatbed / Feeder)").strong());
                    ui.label("Auto-detect skew, remove scanner bed dark borders, despeckle, and whiten background.");
                    ui.separator();

                    ui.horizontal(|ui| {
                        if ui.button("📁 Add Scanned Images...").clicked() {
                            pick_scans = true;
                        }
                        if !self.scan_input_paths.is_empty() && ui.button("Clear List").clicked() {
                            self.scan_input_paths.clear();
                        }
                    });

                    ui.label(format!("Queued Pages: {} image file(s)", self.scan_input_paths.len()));
                    ScrollArea::vertical().max_height(100.0).show(ui, |ui| {
                        for (i, p) in self.scan_input_paths.iter().enumerate() {
                            ui.label(format!("{}. {}", i + 1, p.display()));
                        }
                    });

                    ui.separator();
                    ui.label(RichText::new("Enhancement & Correction Filters:").strong());
                    ui.checkbox(&mut self.scan_auto_deskew, "Auto-Deskew Rotation (Projection profile analysis)");
                    ui.checkbox(&mut self.scan_auto_crop, "Auto-Crop Page Boundaries (Remove bed margins)");
                    ui.checkbox(&mut self.scan_despeckle, "Despeckle (Remove salt-and-pepper scan noise)");
                    ui.checkbox(&mut self.scan_whiten, "Whiten Paper Background (Clean illumination)");
                    ui.checkbox(&mut self.scan_binarize, "Otsu Binarization (Pure 1-bit Black & White)");

                    ui.add_space(10.0);
                    ui.separator();
                    ui.horizontal(|ui| {
                        let can_process = !self.scan_input_paths.is_empty();
                        if ui.add_enabled(can_process, Button::new("🚀 Process & Create PDF")).clicked() {
                            process_scans = true;
                        }
                        if ui.button("Close").clicked() {
                            close = true;
                        }
                    });
                });

            if pick_scans {
                if let Some(files) = rfd::FileDialog::new()
                    .add_filter(
                        "Scanned Image Files",
                        &["png", "jpg", "jpeg", "bmp", "tiff", "webp"],
                    )
                    .pick_files()
                {
                    self.scan_input_paths.extend(files);
                }
            }

            if process_scans {
                let mut images = Vec::new();
                for p in &self.scan_input_paths {
                    if let Ok(img) = image::open(p) {
                        images.push(img.to_rgba8());
                    }
                }

                if !images.is_empty() {
                    let config = ScannerJobConfig {
                        auto_deskew: self.scan_auto_deskew,
                        auto_crop: self.scan_auto_crop,
                        despeckle: self.scan_despeckle,
                        whiten_background: self.scan_whiten,
                        binarize: self.scan_binarize,
                        ..Default::default()
                    };

                    match batch_scan_to_pdf(&images, &config) {
                        Ok(doc) => {
                            let id = self.next_tab_id;
                            self.next_tab_id += 1;
                            let title = format!("Scanned Document ({} pages).pdf", images.len());
                            let mut tab = DocumentTab::new(id, title, doc, None);
                            tab.is_modified = true;
                            self.tabs.push(tab);
                            self.active_tab_idx = Some(self.tabs.len() - 1);
                            self.status_message =
                                format!("Created enhanced PDF from {} scanned pages", images.len());
                            self.scan_input_paths.clear();
                            close = true;
                        }
                        Err(e) => {
                            self.status_message = format!("Scan processing failed: {}", e);
                        }
                    }
                }
            }

            if close {
                self.show_scan_dialog = false;
            }
        }

        // 26. Print Imposition & PostScript Dialog (FR-35)
        if self.show_print_dialog {
            let mut close = false;
            let mut do_print_ps = false;

            egui::Window::new("Print & Imposition Setup")
                .default_size([520.0, 440.0])
                .resizable(true)
                .show(ctx, |ui| {
                    ui.label(RichText::new("Professional Print Setup (ISO 32000-1 / PostScript Level 3)").strong());
                    ui.label("Configure page range, saddle-stitch booklet imposition, N-up tiling, and PostScript output.");
                    ui.separator();

                    ui.horizontal(|ui| {
                        ui.label("Page Range:");
                        ui.text_edit_singleline(&mut self.print_pages_range);
                        ui.label("(e.g. 'all', '1-3, 5')");
                    });

                    ui.horizontal(|ui| {
                        ui.label("Copies:");
                        ui.add(egui::DragValue::new(&mut self.print_copies).range(1..=99));
                        ui.checkbox(&mut self.print_collate, "Collate");
                    });

                    ui.separator();
                    ui.label(RichText::new("Page Layout & Imposition:").strong());
                    egui::ComboBox::from_label("Imposition Mode")
                        .selected_text(match self.print_layout_idx {
                            0 => "Standard (1-Up Full Page)",
                            1 => "Saddle-Stitch Booklet (Automatic imposition)",
                            2 => "2 Pages Per Sheet (2-Up)",
                            3 => "4 Pages Per Sheet (4-Up)",
                            4 => "Poster Tiling (Cut Marks & Overlap)",
                            _ => "Standard",
                        })
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut self.print_layout_idx, 0, "Standard (1-Up Full Page)");
                            ui.selectable_value(&mut self.print_layout_idx, 1, "Saddle-Stitch Booklet (Automatic imposition)");
                            ui.selectable_value(&mut self.print_layout_idx, 2, "2 Pages Per Sheet (2-Up)");
                            ui.selectable_value(&mut self.print_layout_idx, 3, "4 Pages Per Sheet (4-Up)");
                            ui.selectable_value(&mut self.print_layout_idx, 4, "Poster Tiling (Cut Marks & Overlap)");
                        });

                    if let Some(idx) = self.active_tab_idx {
                        if let Some(tab) = self.tabs.get(idx) {
                            let print_cfg = PrintJobConfig {
                                page_range_str: self.print_pages_range.clone(),
                                layout_mode: match self.print_layout_idx {
                                    1 => PrintLayoutMode::Booklet {
                                        binding: BookletBinding::LeftToRight,
                                        subset: ravpdf_tools::BookletSubset::BothSides,
                                    },
                                    2 => PrintLayoutMode::NUp {
                                        count: NUpCount::TwoUp,
                                        order: NUpOrder::Horizontal,
                                        draw_border: true,
                                    },
                                    3 => PrintLayoutMode::NUp {
                                        count: NUpCount::FourUp,
                                        order: NUpOrder::Horizontal,
                                        draw_border: true,
                                    },
                                    4 => PrintLayoutMode::Poster {
                                        tile_scale: 1.0,
                                        overlap_points: 36.0,
                                        cut_marks: true,
                                    },
                                    _ => PrintLayoutMode::Standard,
                                },
                                ..Default::default()
                            };

                            if let Ok(rep) = generate_print_preview_layout(&tab.doc, &print_cfg) {
                                ui.add_space(8.0);
                                ui.group(|ui| {
                                    ui.label(RichText::new("Imposition Preview Summary:").strong().color(Color32::from_rgb(80, 180, 240)));
                                    ui.label(format!("• Logical Pages: {}", rep.total_logical_pages));
                                    ui.label(format!("• Physical Printed Sheets: {}", rep.total_physical_sheets));
                                    ui.label(format!("• {}", rep.summary_description));
                                });
                            }
                        }
                    }

                    ui.add_space(10.0);
                    ui.separator();
                    ui.horizontal(|ui| {
                        if ui.button("🖨 Export PostScript Print Job (.ps)...").clicked() {
                            do_print_ps = true;
                        }
                        if ui.button("Close").clicked() {
                            close = true;
                        }
                    });
                });

            if do_print_ps {
                if let Some(tab) = self.active_tab() {
                    let print_cfg = PrintJobConfig {
                        page_range_str: self.print_pages_range.clone(),
                        copies: self.print_copies,
                        collate: self.print_collate,
                        layout_mode: match self.print_layout_idx {
                            1 => PrintLayoutMode::Booklet {
                                binding: BookletBinding::LeftToRight,
                                subset: ravpdf_tools::BookletSubset::BothSides,
                            },
                            2 => PrintLayoutMode::NUp {
                                count: NUpCount::TwoUp,
                                order: NUpOrder::Horizontal,
                                draw_border: true,
                            },
                            3 => PrintLayoutMode::NUp {
                                count: NUpCount::FourUp,
                                order: NUpOrder::Horizontal,
                                draw_border: true,
                            },
                            4 => PrintLayoutMode::Poster {
                                tile_scale: 1.0,
                                overlap_points: 36.0,
                                cut_marks: true,
                            },
                            _ => PrintLayoutMode::Standard,
                        },
                        ..Default::default()
                    };

                    match generate_postscript(&tab.doc, &print_cfg) {
                        Ok(ps_code) => {
                            if let Some(dest) = rfd::FileDialog::new()
                                .add_filter("PostScript Print Job", &["ps"])
                                .set_file_name("print_job.ps")
                                .save_file()
                            {
                                if std::fs::write(&dest, ps_code).is_ok() {
                                    self.status_message = format!(
                                        "PostScript print job saved to '{}'",
                                        dest.display()
                                    );
                                    close = true;
                                }
                            }
                        }
                        Err(e) => {
                            self.status_message = format!("Print generation error: {}", e);
                        }
                    }
                }
            }

            if close {
                self.show_print_dialog = false;
            }
        }

        // 27. Plugins & Extensions Manager Dialog (FR-36)
        if self.show_plugins_dialog {
            let mut close = false;
            let mut trigger_action: Option<(String, String)> = None;

            egui::Window::new("Plugins & Extensions Manager")
                .open(&mut self.show_plugins_dialog)
                .default_size([580.0, 420.0])
                .resizable(true)
                .show(ctx, |ui| {
                    ui.label(RichText::new("RavPDF Extension Architecture & Sandboxed Plugins").strong());
                    ui.label("Manage loaded plugins, capability permissions, and trigger custom extension actions.");
                    ui.separator();

                    let plugins = self.plugin_registry.list_plugins();
                    if plugins.is_empty() {
                        ui.label(RichText::new("No extensions installed.").italics().color(Color32::GRAY));
                    } else {
                        ScrollArea::vertical().max_height(260.0).show(ui, |ui| {
                            for p in plugins {
                                ui.group(|ui| {
                                    ui.horizontal(|ui| {
                                        ui.label(RichText::new(&p.manifest.name).strong());
                                        ui.label(format!("v{}", p.manifest.version));
                                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                            ui.label(if p.enabled {
                                                RichText::new("● Enabled").color(Color32::from_rgb(60, 200, 100))
                                            } else {
                                                RichText::new("○ Disabled").color(Color32::GRAY)
                                            });
                                        });
                                    });
                                    ui.label(RichText::new(&p.manifest.description).small());
                                    ui.label(format!("ID: {} • Author: {}", p.manifest.id, p.manifest.author));
                                    ui.label(format!("Permissions: {:?}", p.granted_permissions));

                                    ui.horizontal(|ui| {
                                        if p.manifest.id == "org.ravpdf.sample-watermark" && ui.button("⚡ Apply Sample Watermark Action").clicked() {
                                            trigger_action = Some((p.manifest.id.clone(), "apply_draft_watermark".to_string()));
                                        }
                                    });
                                });
                            }
                        });
                    }

                    ui.add_space(8.0);
                    ui.separator();
                    ui.horizontal(|ui| {
                        if ui.button("Close").clicked() {
                            close = true;
                        }
                    });
                });

            if let Some((plugin_id, act_id)) = trigger_action {
                if let Some(idx) = self.active_tab_idx {
                    if let Some(tab) = self.tabs.get_mut(idx) {
                        match self.plugin_registry.dispatch_action(
                            &plugin_id,
                            &act_id,
                            Some(&mut tab.doc),
                        ) {
                            Ok(Some(msg)) => {
                                tab.is_modified = true;
                                tab.invalidate_render();
                                self.status_message = format!("Plugin: {}", msg);
                            }
                            Ok(None) => {
                                self.status_message = "Plugin action executed".to_string();
                            }
                            Err(e) => {
                                self.status_message = format!("Plugin error: {}", e);
                            }
                        }
                    }
                } else {
                    self.status_message = "No document open for plugin action".to_string();
                }
            }

            if close {
                self.show_plugins_dialog = false;
            }
        }

        // 28. Export Form Data Dialog (FR-11.3.2)
        if self.show_export_form_dialog {
            let mut close = false;
            let mut do_export = false;

            egui::Window::new("Export Form Data")
                .collapsible(false)
                .resizable(false)
                .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.label(
                        "Export interactive form field values to standard interchange formats:",
                    );
                    ui.add_space(6.0);

                    ui.horizontal(|ui| {
                        ui.label("Export Format:");
                        egui::ComboBox::from_id_salt("form_export_fmt_combo")
                            .selected_text(match self.form_export_format_idx {
                                0 => "JSON (.json)",
                                1 => "FDF - Forms Data Format (.fdf)",
                                2 => "XFDF - XML Forms Data (.xfdf)",
                                3 => "CSV - Comma-Separated Values (.csv)",
                                _ => "XML - Generic Form Schema (.xml)",
                            })
                            .show_ui(ui, |ui| {
                                ui.selectable_value(
                                    &mut self.form_export_format_idx,
                                    0,
                                    "JSON (.json)",
                                );
                                ui.selectable_value(
                                    &mut self.form_export_format_idx,
                                    1,
                                    "FDF - Forms Data Format (.fdf)",
                                );
                                ui.selectable_value(
                                    &mut self.form_export_format_idx,
                                    2,
                                    "XFDF - XML Forms Data (.xfdf)",
                                );
                                ui.selectable_value(
                                    &mut self.form_export_format_idx,
                                    3,
                                    "CSV - Comma-Separated Values (.csv)",
                                );
                                ui.selectable_value(
                                    &mut self.form_export_format_idx,
                                    4,
                                    "XML - Generic Form Schema (.xml)",
                                );
                            });
                    });

                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        if ui.button("📤 Export As...").clicked() {
                            do_export = true;
                            close = true;
                        }
                        if ui.button("Cancel").clicked() {
                            close = true;
                        }
                    });
                });

            if do_export {
                if let Some(tab) = self.active_tab() {
                    let (fmt, ext) = match self.form_export_format_idx {
                        1 => (FormDataFormat::Fdf, "fdf"),
                        2 => (FormDataFormat::Xfdf, "xfdf"),
                        3 => (FormDataFormat::Csv, "csv"),
                        4 => (FormDataFormat::Xml, "xml"),
                        _ => (FormDataFormat::Json, "json"),
                    };

                    match export_form_data(&tab.doc, fmt) {
                        Ok(data) => {
                            if let Some(dest) = rfd::FileDialog::new()
                                .add_filter(format!("Form Data (*.{})", ext), &[ext])
                                .set_file_name(format!("form_data.{}", ext))
                                .save_file()
                            {
                                match std::fs::write(&dest, data) {
                                    Ok(_) => {
                                        self.status_message =
                                            format!("Exported form data to '{}'", dest.display());
                                    }
                                    Err(e) => {
                                        self.status_message =
                                            format!("Failed to save form data: {}", e);
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            self.status_message = format!("Export error: {}", e);
                        }
                    }
                }
            }

            if close {
                self.show_export_form_dialog = false;
            }
        }

        // 29. Tab Order Configuration Dialog (FR-11.2.7)
        if self.show_tab_order_dialog {
            let mut close = false;
            let mut apply_tab_order = false;

            egui::Window::new("Set Page Tab Order")
                .collapsible(false)
                .resizable(false)
                .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.label("Configure keyboard tab traversal order conforming to ISO 32000-1 Clause 12.4.2:");
                    ui.add_space(6.0);

                    ui.radio_value(&mut self.tab_order_idx, 0, "Row Order (/R) — Left-to-right, top-to-bottom");
                    ui.radio_value(&mut self.tab_order_idx, 1, "Column Order (/C) — Top-to-bottom, left-to-right");
                    ui.radio_value(&mut self.tab_order_idx, 2, "Structure Tree (/S) — Logical Tagged PDF order");
                    ui.radio_value(&mut self.tab_order_idx, 3, "Manual Order (/A) — Array sequence in page dictionary");

                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        if ui.button("Apply Tab Order").clicked() {
                            apply_tab_order = true;
                            close = true;
                        }
                        if ui.button("Cancel").clicked() {
                            close = true;
                        }
                    });
                });

            if apply_tab_order {
                let order = match self.tab_order_idx {
                    0 => TabOrder::Row,
                    1 => TabOrder::Column,
                    2 => TabOrder::Structure,
                    _ => TabOrder::Manual,
                };
                if let Some(tab) = self.active_tab_mut() {
                    let curr = tab.current_page;
                    match apply_page_tab_order(&mut tab.doc, curr, order) {
                        Ok(()) => {
                            tab.is_modified = true;
                            tab.invalidate_render();
                            self.status_message =
                                format!("Applied {:?} tab order to page {}", order, curr + 1);
                        }
                        Err(e) => {
                            self.status_message = format!("Failed to apply tab order: {}", e);
                        }
                    }
                }
            }

            if close {
                self.show_tab_order_dialog = false;
            }
        }

        // 30. Embed Rich Media Dialog (FR-27)
        if self.show_rich_media_dialog {
            let mut close = false;
            let mut do_embed = false;

            egui::Window::new("Embed Rich Media / Video")
                .collapsible(false)
                .resizable(false)
                .default_size([460.0, 320.0])
                .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.label("Embed video/audio or streaming content conforming to ISO 32000-1 Clause 12.5.6.21:");
                    ui.add_space(6.0);

                    ui.horizontal(|ui| {
                        ui.label("Title / Name:");
                        ui.text_edit_singleline(&mut self.rich_media_title);
                    });

                    ui.horizontal(|ui| {
                        ui.radio_value(&mut self.rich_media_is_url, false, "Local File");
                        ui.radio_value(&mut self.rich_media_is_url, true, "Streaming URL");
                    });

                    if self.rich_media_is_url {
                        ui.horizontal(|ui| {
                            ui.label("URL:");
                            ui.text_edit_singleline(&mut self.rich_media_url_or_path);
                        });
                    } else {
                        ui.horizontal(|ui| {
                            ui.label("File:");
                            ui.text_edit_singleline(&mut self.rich_media_url_or_path);
                            if ui.button("Browse...").clicked() {
                                if let Some(path) = rfd::FileDialog::new()
                                    .add_filter("Media Files (*.mp4, *.mov, *.mp3, *.aac)", &["mp4", "mov", "mp3", "aac"])
                                    .pick_file()
                                {
                                    self.rich_media_url_or_path = path.to_string_lossy().to_string();
                                }
                            }
                        });
                    }

                    ui.add_space(4.0);
                    ui.checkbox(&mut self.rich_media_autoplay, "Auto-play on page open");
                    ui.checkbox(&mut self.rich_media_loop, "Loop playback");

                    ui.weak(format!("Bounding Box: [{:.1}, {:.1}, {:.1}, {:.1}] pt",
                        self.rich_media_rect[0], self.rich_media_rect[1],
                        self.rich_media_rect[2], self.rich_media_rect[3]));

                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        if ui.button("Embed Media").clicked() {
                            do_embed = true;
                            close = true;
                        }
                        if ui.button("Cancel").clicked() {
                            close = true;
                        }
                    });
                });

            if do_embed {
                let target = self.rich_media_url_or_path.trim().to_string();
                if target.is_empty() {
                    self.status_message = "Please provide a media file or URL".into();
                } else {
                    let content_res = if self.rich_media_is_url {
                        let mime = if target.ends_with(".mp3") {
                            "audio/mp3".to_string()
                        } else if target.ends_with(".aac") {
                            "audio/aac".to_string()
                        } else if target.ends_with(".mov") {
                            "video/quicktime".to_string()
                        } else {
                            "video/mp4".to_string()
                        };
                        Ok(MediaContent::ExternalUrl {
                            url: target,
                            mime_type: mime,
                        })
                    } else {
                        let p = std::path::Path::new(&target);
                        match std::fs::read(p) {
                            Ok(bytes) => {
                                let ext = p
                                    .extension()
                                    .and_then(|e| e.to_str())
                                    .unwrap_or("")
                                    .to_lowercase();
                                let mime = match ext.as_str() {
                                    "mp3" => "audio/mp3".to_string(),
                                    "aac" => "audio/aac".to_string(),
                                    "mov" => "video/quicktime".to_string(),
                                    _ => "video/mp4".to_string(),
                                };
                                let fname = p
                                    .file_name()
                                    .and_then(|n| n.to_str())
                                    .unwrap_or("media.mp4")
                                    .to_string();
                                Ok(MediaContent::Embedded {
                                    data: bytes,
                                    mime_type: mime,
                                    filename: fname,
                                })
                            }
                            Err(e) => Err(format!("Failed to read media file: {}", e)),
                        }
                    };

                    match content_res {
                        Ok(content) => {
                            let cfg = RichMediaConfig {
                                title: self.rich_media_title.clone(),
                                content,
                                rect: self.rich_media_rect,
                                poster_image: None,
                                auto_play: self.rich_media_autoplay,
                                loop_playback: self.rich_media_loop,
                                show_controls: true,
                                volume: 1.0,
                                activation: if self.rich_media_autoplay {
                                    RichMediaActivation::PageOpen
                                } else {
                                    RichMediaActivation::UserClick
                                },
                                deactivation: RichMediaDeactivation::PageClose,
                            };
                            if let Some(tab) = self.active_tab_mut() {
                                let curr = tab.current_page;
                                match add_rich_media_annotation(&mut tab.doc, curr, &cfg) {
                                    Ok(_) => {
                                        tab.is_modified = true;
                                        tab.invalidate_render();
                                        self.status_message = format!(
                                            "Embedded rich media '{}' on page {}",
                                            cfg.title,
                                            curr + 1
                                        );
                                    }
                                    Err(e) => {
                                        self.status_message =
                                            format!("Failed to embed rich media: {}", e);
                                    }
                                }
                            }
                        }
                        Err(err_msg) => {
                            self.status_message = err_msg;
                        }
                    }
                }
            }

            if close {
                self.show_rich_media_dialog = false;
            }
        }

        // 31. Embed 3D Model Dialog (FR-28)
        if self.show_threed_dialog {
            let mut close = false;
            let mut do_embed = false;

            egui::Window::new("Embed 3D Model")
                .collapsible(false)
                .resizable(false)
                .default_size([460.0, 320.0])
                .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.label("Embed interactive U3D (ECMA-363) or PRC (ISO 14739-1) model conforming to ISO 32000-1 Clause 13.6:");
                    ui.add_space(6.0);

                    ui.horizontal(|ui| {
                        let path_str = self.threed_model_path.as_ref()
                            .and_then(|p| p.file_name())
                            .and_then(|n| n.to_str())
                            .unwrap_or("(No 3D model selected)");
                        ui.label(format!("File: {}", path_str));
                        if ui.button("Browse (.u3d, .prc)...").clicked() {
                            if let Some(path) = rfd::FileDialog::new()
                                .add_filter("3D Models (*.u3d, *.prc)", &["u3d", "prc"])
                                .pick_file()
                            {
                                self.threed_model_path = Some(path);
                            }
                        }
                    });

                    ui.horizontal(|ui| {
                        ui.label("Render Mode:");
                        egui::ComboBox::from_id_salt("threed_render_combo")
                            .selected_text(match self.threed_render_mode_idx {
                                0 => "Solid",
                                1 => "Solid Wireframe",
                                2 => "Wireframe",
                                3 => "Shaded Illustration",
                                4 => "Transparent",
                                5 => "Transparent Wireframe",
                                6 => "Hidden Line",
                                _ => "Bounding Box",
                            })
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut self.threed_render_mode_idx, 0, "Solid");
                                ui.selectable_value(&mut self.threed_render_mode_idx, 1, "Solid Wireframe");
                                ui.selectable_value(&mut self.threed_render_mode_idx, 2, "Wireframe");
                                ui.selectable_value(&mut self.threed_render_mode_idx, 3, "Shaded Illustration");
                                ui.selectable_value(&mut self.threed_render_mode_idx, 4, "Transparent");
                                ui.selectable_value(&mut self.threed_render_mode_idx, 5, "Transparent Wireframe");
                                ui.selectable_value(&mut self.threed_render_mode_idx, 6, "Hidden Line");
                                ui.selectable_value(&mut self.threed_render_mode_idx, 7, "Bounding Box");
                            });
                    });

                    ui.horizontal(|ui| {
                        ui.label("Lighting Preset:");
                        egui::ComboBox::from_id_salt("threed_light_combo")
                            .selected_text(match self.threed_lighting_idx {
                                0 => "Day",
                                1 => "Night",
                                2 => "White",
                                3 => "Hard",
                                4 => "Primary",
                                5 => "Blue",
                                6 => "Red",
                                7 => "Cube",
                                8 => "CAD",
                                9 => "Headlamp",
                                _ => "None",
                            })
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut self.threed_lighting_idx, 0, "Day");
                                ui.selectable_value(&mut self.threed_lighting_idx, 1, "Night");
                                ui.selectable_value(&mut self.threed_lighting_idx, 2, "White");
                                ui.selectable_value(&mut self.threed_lighting_idx, 3, "Hard");
                                ui.selectable_value(&mut self.threed_lighting_idx, 4, "Primary");
                                ui.selectable_value(&mut self.threed_lighting_idx, 5, "Blue");
                                ui.selectable_value(&mut self.threed_lighting_idx, 6, "Red");
                                ui.selectable_value(&mut self.threed_lighting_idx, 7, "Cube");
                                ui.selectable_value(&mut self.threed_lighting_idx, 8, "CAD");
                                ui.selectable_value(&mut self.threed_lighting_idx, 9, "Headlamp");
                                ui.selectable_value(&mut self.threed_lighting_idx, 10, "None");
                            });
                    });

                    ui.weak(format!("Bounding Box: [{:.1}, {:.1}, {:.1}, {:.1}] pt",
                        self.threed_rect[0], self.threed_rect[1],
                        self.threed_rect[2], self.threed_rect[3]));

                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        if ui.button("Embed 3D Model").clicked() {
                            do_embed = true;
                            close = true;
                        }
                        if ui.button("Cancel").clicked() {
                            close = true;
                        }
                    });
                });

            if do_embed {
                if let Some(ref path) = self.threed_model_path {
                    match std::fs::read(path) {
                        Ok(data) => {
                            let ext = path
                                .extension()
                                .and_then(|e| e.to_str())
                                .unwrap_or("")
                                .to_lowercase();
                            let format = if ext == "prc" {
                                ThreeDFormat::Prc
                            } else {
                                ThreeDFormat::U3d
                            };
                            let render_mode = match self.threed_render_mode_idx {
                                0 => ThreeDRenderMode::Solid,
                                1 => ThreeDRenderMode::SolidWireframe,
                                2 => ThreeDRenderMode::Wireframe,
                                3 => ThreeDRenderMode::ShadedIllustration,
                                4 => ThreeDRenderMode::Transparent,
                                5 => ThreeDRenderMode::TransparentWireframe,
                                6 => ThreeDRenderMode::HiddenLine,
                                _ => ThreeDRenderMode::BoundingBox,
                            };
                            let lighting = match self.threed_lighting_idx {
                                0 => ThreeDLighting::Day,
                                1 => ThreeDLighting::Night,
                                2 => ThreeDLighting::White,
                                3 => ThreeDLighting::Hard,
                                4 => ThreeDLighting::Primary,
                                5 => ThreeDLighting::Blue,
                                6 => ThreeDLighting::Red,
                                7 => ThreeDLighting::Cube,
                                8 => ThreeDLighting::Cad,
                                9 => ThreeDLighting::Headlamp,
                                _ => ThreeDLighting::None,
                            };

                            let view = ThreeDView {
                                name: "Default View".to_string(),
                                render_mode,
                                lighting,
                                ..Default::default()
                            };

                            let cfg = ThreeDConfig {
                                format,
                                model_data: data,
                                rect: self.threed_rect,
                                views: vec![view],
                                default_view_index: 0,
                                model_tree: None,
                            };

                            if let Some(tab) = self.active_tab_mut() {
                                let curr = tab.current_page;
                                match add_3d_annotation(&mut tab.doc, curr, &cfg) {
                                    Ok(_) => {
                                        tab.is_modified = true;
                                        tab.invalidate_render();
                                        self.status_message =
                                            format!("Embedded 3D model on page {}", curr + 1);
                                    }
                                    Err(e) => {
                                        self.status_message =
                                            format!("Failed to embed 3D model: {}", e);
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            self.status_message = format!("Failed to read 3D file: {}", e);
                        }
                    }
                } else {
                    self.status_message = "Please select a 3D model file first".into();
                }
            }

            if close {
                self.show_threed_dialog = false;
            }
        }

        // --- Optical Character Recognition (OCR) Dialog (FR-16) ---
        if self.show_ocr_dialog {
            let mut close = false;
            let mut run_ocr = false;

            egui::Window::new("Recognize Text (OCR Engine)")
                .collapsible(false)
                .resizable(true)
                .default_size([500.0, 480.0])
                .show(ctx, |ui| {
                    ui.heading("Optical Character Recognition");
                    ui.label(
                        "Generate searchable, selectable text layers from scanned raster pages.",
                    );
                    ui.separator();

                    ui.horizontal(|ui| {
                        ui.label("Document Language:");
                        egui::ComboBox::from_id_salt("ocr_lang_combo")
                            .selected_text(match self.ocr_lang_idx {
                                0 => "English (eng)",
                                1 => "Spanish (spa)",
                                2 => "French (fra)",
                                3 => "German (deu)",
                                4 => "Italian (ita)",
                                5 => "Portuguese (por)",
                                6 => "Japanese (jpn)",
                                7 => "Chinese Simplified (chi_sim)",
                                _ => "English (eng)",
                            })
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut self.ocr_lang_idx, 0, "English (eng)");
                                ui.selectable_value(&mut self.ocr_lang_idx, 1, "Spanish (spa)");
                                ui.selectable_value(&mut self.ocr_lang_idx, 2, "French (fra)");
                                ui.selectable_value(&mut self.ocr_lang_idx, 3, "German (deu)");
                                ui.selectable_value(&mut self.ocr_lang_idx, 4, "Italian (ita)");
                                ui.selectable_value(&mut self.ocr_lang_idx, 5, "Portuguese (por)");
                                ui.selectable_value(&mut self.ocr_lang_idx, 6, "Japanese (jpn)");
                                ui.selectable_value(
                                    &mut self.ocr_lang_idx,
                                    7,
                                    "Chinese Simplified (chi_sim)",
                                );
                            });
                    });

                    ui.horizontal(|ui| {
                        ui.label("Output Format:");
                        egui::ComboBox::from_id_salt("ocr_mode_combo")
                            .selected_text(match self.ocr_mode_idx {
                                0 => "Searchable Image (Invisible 3 Tr Layer)",
                                1 => "Searchable Image Exact (Precise Positioning)",
                                2 => "Editable Text (Vector Replacement)",
                                _ => "Searchable Image",
                            })
                            .show_ui(ui, |ui| {
                                ui.selectable_value(
                                    &mut self.ocr_mode_idx,
                                    0,
                                    "Searchable Image (Invisible 3 Tr Layer)",
                                );
                                ui.selectable_value(
                                    &mut self.ocr_mode_idx,
                                    1,
                                    "Searchable Image Exact (Precise Positioning)",
                                );
                                ui.selectable_value(
                                    &mut self.ocr_mode_idx,
                                    2,
                                    "Editable Text (Vector Replacement)",
                                );
                            });
                    });

                    ui.horizontal(|ui| {
                        ui.label("Target Resolution:");
                        egui::ComboBox::from_id_salt("ocr_dpi_combo")
                            .selected_text(format!("{} DPI", self.ocr_dpi))
                            .show_ui(ui, |ui| {
                                ui.selectable_value(
                                    &mut self.ocr_dpi,
                                    150,
                                    "150 DPI (Fast / Draft)",
                                );
                                ui.selectable_value(
                                    &mut self.ocr_dpi,
                                    300,
                                    "300 DPI (Recommended / Standard)",
                                );
                                ui.selectable_value(
                                    &mut self.ocr_dpi,
                                    600,
                                    "600 DPI (Archival / High Precision)",
                                );
                            });
                    });

                    ui.checkbox(
                        &mut self.ocr_auto_rotate,
                        "Auto-detect orientation and correct rotation",
                    );

                    ui.separator();
                    ui.label("Page Range:");
                    ui.radio_value(&mut self.ocr_all_pages, true, "All Pages in Document");
                    ui.radio_value(&mut self.ocr_all_pages, false, "Current Page Only");

                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        if ui.button("⚡ Run OCR Recognition").clicked() {
                            run_ocr = true;
                        }
                        if ui.button("Close").clicked() {
                            close = true;
                        }
                    });

                    if let Some(ref res) = self.ocr_result {
                        ui.separator();
                        ui.heading("Recognition Results Summary");
                        ui.label(format!("• Total Recognized Words: {}", res.total_words));
                        ui.label(format!(
                            "• Average Confidence Score: {:.1}%",
                            res.average_confidence * 100.0
                        ));
                        ui.label(format!(
                            "• Uncertain Words Flagged: {}",
                            res.total_uncertain_words
                        ));
                        ui.label(format!("• Pages Processed: {}", res.pages.len()));

                        ui.collapsing("Extracted Text Preview", |ui| {
                            egui::ScrollArea::vertical()
                                .max_height(140.0)
                                .show(ui, |ui| {
                                    for p in &res.pages {
                                        ui.strong(format!("Page {}:", p.page_index + 1));
                                        ui.label(&p.full_text);
                                    }
                                });
                        });
                    }
                });

            if run_ocr {
                let all_pages = self.ocr_all_pages;
                let lang = match self.ocr_lang_idx {
                    0 => "eng",
                    1 => "spa",
                    2 => "fra",
                    3 => "deu",
                    4 => "ita",
                    5 => "por",
                    6 => "jpn",
                    7 => "chi_sim",
                    _ => "eng",
                };
                let output_mode = match self.ocr_mode_idx {
                    0 => OcrOutputMode::SearchableImage,
                    1 => OcrOutputMode::SearchableImageExact,
                    2 => OcrOutputMode::EditableText,
                    _ => OcrOutputMode::SearchableImage,
                };
                let cfg = OcrConfig {
                    language: lang.to_string(),
                    output_mode,
                    dpi: self.ocr_dpi,
                    auto_rotate: self.ocr_auto_rotate,
                    min_confidence: 0.70,
                    preserve_appearance: true,
                };

                let ocr_outcome = if let Some(tab) = self.active_tab_mut() {
                    let page_range = if all_pages {
                        None
                    } else {
                        Some(vec![tab.current_page])
                    };
                    let outcome = perform_ocr_document(&mut tab.doc, &cfg, page_range.as_deref());
                    if outcome.is_ok() {
                        tab.is_modified = true;
                        tab.invalidate_render();
                    }
                    Some(outcome)
                } else {
                    None
                };

                if let Some(outcome) = ocr_outcome {
                    match outcome {
                        Ok(res) => {
                            self.status_message = format!(
                                "OCR completed: {} words recognized across {} page(s)",
                                res.total_words,
                                res.pages.len()
                            );
                            self.ocr_result = Some(res);
                        }
                        Err(e) => {
                            self.status_message = format!("OCR failed: {}", e);
                        }
                    }
                }
            }

            if close {
                self.show_ocr_dialog = false;
            }
        }

        // Print Production Output Preview & Plate Separations Dialog (FR-23.2)
        if self.show_output_preview_dialog {
            let mut close = false;
            let mut inspect = false;
            let mut reset_plates = false;

            egui::Window::new("Output Preview & Separations")
                .collapsible(false)
                .resizable(true)
                .default_size([460.0, 520.0])
                .anchor(Align2::RIGHT_TOP, [-20.0, 60.0])
                .show(ctx, |ui| {
                    ui.heading("Separations & Press Density");
                    ui.add_space(4.0);

                    // Plate controls
                    ui.group(|ui| {
                        ui.label(RichText::new("Process CMYK Plates:").strong());
                        ui.horizontal(|ui| {
                            ui.checkbox(&mut self.plate_config.show_cyan, "Cyan (C)");
                            ui.checkbox(&mut self.plate_config.show_magenta, "Magenta (M)");
                        });
                        ui.horizontal(|ui| {
                            ui.checkbox(&mut self.plate_config.show_yellow, "Yellow (Y)");
                            ui.checkbox(&mut self.plate_config.show_black, "Black (K)");
                        });
                        ui.checkbox(
                            &mut self.plate_config.single_plate_as_grayscale,
                            "Render single plate as press film density (Grayscale)",
                        );
                        if ui.button("Show All Plates (CMYK)").clicked() {
                            reset_plates = true;
                        }
                    });

                    ui.add_space(8.0);

                    // Total Area Coverage (TAC) controls
                    ui.group(|ui| {
                        ui.label(RichText::new("Total Area Coverage (TAC / TIC):").strong());
                        ui.checkbox(
                            &mut self.plate_config.show_tac_warning,
                            "Highlight TAC Ink Limits Exceeded",
                        );
                        let mut tac_pct = (self.plate_config.tac_threshold * 100.0).round() as u32;
                        ui.horizontal(|ui| {
                            ui.label("TAC Limit:");
                            if ui
                                .add(egui::Slider::new(&mut tac_pct, 150..=400).suffix("%"))
                                .changed()
                            {
                                self.plate_config.tac_threshold = (tac_pct as f32) / 100.0;
                            }
                        });
                        ui.weak("Highlights regions where ink coverage exceeds press limit with high-contrast neon overlay.");
                    });

                    // Live analysis readout
                    if let Some(ref ana) = self.separation_analysis {
                        ui.add_space(8.0);
                        ui.group(|ui| {
                            ui.label(RichText::new("Live Plate Analysis:").strong());
                            ui.label(format!("• Max TAC on Page: {:.1}%", ana.max_tac * 100.0));
                            let tac_color = if ana.tac_exceeded_pixels > 0 {
                                Color32::from_rgb(220, 38, 38)
                            } else {
                                Color32::from_rgb(46, 160, 67)
                            };
                            ui.colored_label(
                                tac_color,
                                format!(
                                    "• TAC Exceeded Pixels: {} ({:.2}%)",
                                    ana.tac_exceeded_pixels, ana.tac_exceeded_percentage
                                ),
                            );
                            ui.label(format!("• Cyan Plate Coverage: {:.1}%", ana.cyan_coverage_pct));
                            ui.label(format!("• Magenta Plate Coverage: {:.1}%", ana.magenta_coverage_pct));
                            ui.label(format!("• Yellow Plate Coverage: {:.1}%", ana.yellow_coverage_pct));
                            ui.label(format!("• Black Plate Coverage: {:.1}%", ana.black_coverage_pct));
                        });
                    }

                    ui.add_space(8.0);

                    // Prepress Object Inspector (FR-23.2.5)
                    ui.group(|ui| {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Object Inspector:").strong());
                            if ui.button("🔍 Inspect Current Page Objects").clicked() {
                                inspect = true;
                            }
                        });

                        if let Some(ref report) = self.object_inspection_report {
                            ui.add_space(4.0);
                            ScrollArea::vertical().max_height(140.0).show(ui, |ui| {
                                ui.label(RichText::new(format!("Page {} Prepress Objects:", report.page_index + 1)).underline());
                                ui.label(format!("• Transparency Present: {}", if report.transparency_present { "Yes (Blend Modes)" } else { "None" }));
                                ui.label(format!("• Overprint Settings: {}", if report.overprint_present { "Overprint Enabled" } else { "None" }));

                                ui.strong("Color Spaces:");
                                if report.color_spaces.is_empty() {
                                    ui.weak("  (None detected)");
                                } else {
                                    for cs in &report.color_spaces {
                                        ui.label(format!("  • {}", cs));
                                    }
                                }

                                ui.strong("Fonts:");
                                if report.fonts.is_empty() {
                                    ui.weak("  (None detected)");
                                } else {
                                    for font in &report.fonts {
                                        ui.label(format!("  • {}", font));
                                    }
                                }

                                ui.strong("Images:");
                                if report.images_info.is_empty() {
                                    ui.weak("  (None detected)");
                                } else {
                                    for img in &report.images_info {
                                        ui.label(format!("  • {}", img));
                                    }
                                }
                            });
                        }
                    });

                    ui.add_space(10.0);
                    if ui.button("Close Output Preview").clicked() {
                        close = true;
                    }
                });

            if reset_plates {
                self.plate_config.show_cyan = true;
                self.plate_config.show_magenta = true;
                self.plate_config.show_yellow = true;
                self.plate_config.show_black = true;
                self.plate_config.single_plate_as_grayscale = false;
            }

            if inspect {
                if let Some(tab) = self.active_tab() {
                    let rep = inspect_page_objects(&tab.doc, tab.current_page, [0.0, 0.0]).ok();
                    self.object_inspection_report = rep;
                }
            }

            if close {
                self.show_output_preview_dialog = false;
                if let Some(tab) = self.active_tab_mut() {
                    tab.invalidate_render();
                }
            }
        }

        // Color Management & Soft-Proofing Dialog (FR-24)
        if self.show_color_management_dialog {
            let mut close = false;
            let mut apply_soft_proof_toggle = false;
            let mut execute_convert = false;
            let mut execute_embed_oi = false;

            egui::Window::new("Color Management & Soft Proofing")
                .collapsible(false)
                .resizable(true)
                .default_size([540.0, 560.0])
                .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.heading("Soft-Proofing Simulation");
                    ui.label("Simulate press output, ink gamut, paper white, and black ink on your display.");
                    ui.add_space(4.0);

                    ui.group(|ui| {
                        let mut sp_active = self.soft_proof_active;
                        if ui.checkbox(&mut sp_active, RichText::new("Enable CMYK Soft-Proofing").strong()).changed() {
                            self.soft_proof_active = sp_active;
                            apply_soft_proof_toggle = true;
                        }

                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            ui.label("Working CMYK Profile:");
                            egui::ComboBox::from_id_salt("sp_cmyk_profile")
                                .selected_text(self.soft_proof_config.cmyk_profile.display_name())
                                .show_ui(ui, |ui| {
                                    if ui.selectable_value(&mut self.soft_proof_config.cmyk_profile, StandardIccProfile::USWebCoatedSWOP, "U.S. Web Coated (SWOP) v2").clicked() {
                                        apply_soft_proof_toggle = true;
                                    }
                                    if ui.selectable_value(&mut self.soft_proof_config.cmyk_profile, StandardIccProfile::FOGRA39, "Coated FOGRA39 (ISO 12647-2)").clicked() {
                                        apply_soft_proof_toggle = true;
                                    }
                                    if ui.selectable_value(&mut self.soft_proof_config.cmyk_profile, StandardIccProfile::GRACoL2006, "GRACoL 2006 Coated 1").clicked() {
                                        apply_soft_proof_toggle = true;
                                    }
                                    if ui.selectable_value(&mut self.soft_proof_config.cmyk_profile, StandardIccProfile::JapanColor2001Coated, "Japan Color 2001 Coated").clicked() {
                                        apply_soft_proof_toggle = true;
                                    }
                                });
                        });

                        ui.horizontal(|ui| {
                            ui.label("Rendering Intent:");
                            egui::ComboBox::from_id_salt("sp_rendering_intent")
                                .selected_text(self.soft_proof_config.rendering_intent.to_pdf_name())
                                .show_ui(ui, |ui| {
                                    if ui.selectable_value(&mut self.soft_proof_config.rendering_intent, RenderingIntent::RelativeColorimetric, "Relative Colorimetric (Standard)").clicked() {
                                        apply_soft_proof_toggle = true;
                                    }
                                    if ui.selectable_value(&mut self.soft_proof_config.rendering_intent, RenderingIntent::Perceptual, "Perceptual (Photographic)").clicked() {
                                        apply_soft_proof_toggle = true;
                                    }
                                    if ui.selectable_value(&mut self.soft_proof_config.rendering_intent, RenderingIntent::Saturation, "Saturation (Vivid Graphics)").clicked() {
                                        apply_soft_proof_toggle = true;
                                    }
                                    if ui.selectable_value(&mut self.soft_proof_config.rendering_intent, RenderingIntent::AbsoluteColorimetric, "Absolute Colorimetric (Proofing)").clicked() {
                                        apply_soft_proof_toggle = true;
                                    }
                                });
                        });

                        if ui.checkbox(&mut self.soft_proof_config.simulate_paper_white, "Simulate Paper White (unprinted stock tint)").changed() {
                            apply_soft_proof_toggle = true;
                        }
                        if ui.checkbox(&mut self.soft_proof_config.simulate_black_ink, "Simulate Black Ink (press dynamic range floor)").changed() {
                            apply_soft_proof_toggle = true;
                        }
                    });

                    ui.add_space(8.0);
                    ui.separator();
                    ui.heading("Convert Document Colors");
                    ui.label("Transform color operators in all page content streams to a target working color space.");
                    ui.add_space(4.0);

                    ui.group(|ui| {
                        ui.horizontal(|ui| {
                            ui.label("Target Color Space:");
                            ui.radio_value(&mut self.color_convert_target_idx, 0, "DeviceCMYK (Press)");
                            ui.radio_value(&mut self.color_convert_target_idx, 1, "DeviceRGB (Screen)");
                            ui.radio_value(&mut self.color_convert_target_idx, 2, "DeviceGray (Monochrome)");
                        });
                        if ui.button("🔄 Convert All Page Colors").clicked() {
                            execute_convert = true;
                        }
                    });

                    ui.add_space(8.0);
                    ui.separator();
                    ui.heading("Output Intent & ICC Profiles");
                    ui.label("Embed PDF/X or PDF/A OutputIntent with embedded ICC binary profile.");
                    ui.add_space(4.0);

                    ui.group(|ui| {
                        ui.horizontal(|ui| {
                            ui.label("ICC Profile:");
                            egui::ComboBox::from_id_salt("oi_embed_profile")
                                .selected_text(match self.output_intent_profile_idx {
                                    0 => "sRGB IEC61966-2.1",
                                    1 => "Adobe RGB (1998)",
                                    2 => "Display P3",
                                    3 => "U.S. Web Coated (SWOP) v2",
                                    4 => "Coated FOGRA39",
                                    5 => "GRACoL 2006",
                                    6 => "Japan Color 2001 Coated",
                                    _ => "Dot Gain 20%",
                                })
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(&mut self.output_intent_profile_idx, 0, "sRGB IEC61966-2.1");
                                    ui.selectable_value(&mut self.output_intent_profile_idx, 1, "Adobe RGB (1998)");
                                    ui.selectable_value(&mut self.output_intent_profile_idx, 2, "Display P3");
                                    ui.selectable_value(&mut self.output_intent_profile_idx, 3, "U.S. Web Coated (SWOP) v2");
                                    ui.selectable_value(&mut self.output_intent_profile_idx, 4, "Coated FOGRA39");
                                    ui.selectable_value(&mut self.output_intent_profile_idx, 5, "GRACoL 2006");
                                    ui.selectable_value(&mut self.output_intent_profile_idx, 6, "Japan Color 2001 Coated");
                                    ui.selectable_value(&mut self.output_intent_profile_idx, 7, "Dot Gain 20%");
                                });
                        });
                        ui.horizontal(|ui| {
                            ui.label("Condition Identifier:");
                            ui.text_edit_singleline(&mut self.output_intent_condition);
                        });
                        ui.horizontal(|ui| {
                            ui.label("Registry URL:");
                            ui.text_edit_singleline(&mut self.output_intent_registry);
                        });
                        if ui.button("📌 Embed Output Intent & ICC Profile").clicked() {
                            execute_embed_oi = true;
                        }
                    });

                    ui.add_space(10.0);
                    if ui.button("Close").clicked() {
                        close = true;
                    }
                });

            if apply_soft_proof_toggle {
                if let Some(tab) = self.active_tab_mut() {
                    tab.invalidate_render();
                }
            }

            if execute_convert {
                let target = match self.color_convert_target_idx {
                    0 => "DeviceCMYK",
                    1 => "DeviceRGB",
                    _ => "DeviceGray",
                };
                if let Some(tab) = self.active_tab_mut() {
                    match convert_document_colors(&mut tab.doc, target) {
                        Ok(modified) => {
                            tab.is_modified = true;
                            tab.invalidate_render();
                            self.status_message = format!(
                                "Converted document colors to {}: {} stream(s) modified",
                                target, modified
                            );
                        }
                        Err(e) => {
                            self.status_message = format!("Color conversion failed: {}", e);
                        }
                    }
                }
            }

            if execute_embed_oi {
                let profile = match self.output_intent_profile_idx {
                    0 => StandardIccProfile::SRgb,
                    1 => StandardIccProfile::AdobeRgb1998,
                    2 => StandardIccProfile::DisplayP3,
                    3 => StandardIccProfile::USWebCoatedSWOP,
                    4 => StandardIccProfile::FOGRA39,
                    5 => StandardIccProfile::GRACoL2006,
                    6 => StandardIccProfile::JapanColor2001Coated,
                    _ => StandardIccProfile::DotGain20,
                };
                let cond = self.output_intent_condition.clone();
                let reg = self.output_intent_registry.clone();

                if let Some(tab) = self.active_tab_mut() {
                    match embed_output_intent(&mut tab.doc, profile, &cond, &reg) {
                        Ok(oi_id) => {
                            tab.is_modified = true;
                            self.status_message =
                                format!("Embedded Output Intent {} for '{}'", oi_id, cond);
                        }
                        Err(e) => {
                            self.status_message = format!("Embedding Output Intent failed: {}", e);
                        }
                    }
                }
            }

            if close {
                self.show_color_management_dialog = false;
            }
        }

        // Prepress Tools Modal (FR-23.3)
        if self.show_prepress_dialog {
            let mut close = false;
            let mut execute_add_marks = false;
            let mut execute_flatten = false;
            let mut execute_convert_spots = false;
            let mut execute_add_trap = false;

            egui::Window::new("Commercial Prepress & Print Production")
                .collapsible(false)
                .resizable(true)
                .default_size([600.0, 520.0])
                .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.selectable_value(&mut self.prepress_tab_idx, 0, "📐 Printer Marks");
                        ui.selectable_value(&mut self.prepress_tab_idx, 1, "🎨 Ink Manager");
                        ui.selectable_value(&mut self.prepress_tab_idx, 2, "🪟 Transparency Flattener");
                        ui.selectable_value(&mut self.prepress_tab_idx, 3, "🪤 Trap Presets");
                    });
                    ui.separator();

                    match self.prepress_tab_idx {
                        0 => {
                            ui.heading("Add Prepress Printer Marks");
                            ui.label("Embed cutting ticks, bleed boundaries, registration bullseyes, color bars, and slug info.");
                            ui.add_space(4.0);
                            ui.group(|ui| {
                                ui.checkbox(&mut self.printer_marks_config.trim_marks, "Trim Marks (corner cutting boundaries)");
                                ui.checkbox(&mut self.printer_marks_config.bleed_marks, "Bleed Marks (outer artwork boundaries)");
                                ui.checkbox(&mut self.printer_marks_config.registration_marks, "Registration Marks (4-edge alignment targets)");
                                ui.checkbox(&mut self.printer_marks_config.color_bars, "Color Bars (CMYK tint step wedges)");
                                ui.checkbox(&mut self.printer_marks_config.page_information, "Page Information (job title, page number, timestamp in slug)");
                                ui.add_space(4.0);
                                ui.horizontal(|ui| {
                                    ui.label("Slug Margin (pt):");
                                    ui.add(egui::DragValue::new(&mut self.printer_marks_config.slug_margin).speed(1.0).range(18.0..=72.0));
                                    ui.label("Bleed Offset (pt):");
                                    ui.add(egui::DragValue::new(&mut self.printer_marks_config.bleed_offset).speed(0.5).range(3.0..=24.0));
                                });
                            });
                            ui.add_space(8.0);
                            if ui.button("✅ Inject Printer Marks into Pages").clicked() {
                                execute_add_marks = true;
                            }
                        }
                        1 => {
                            ui.heading("Ink Manager & Separation Plates");
                            ui.label("Inspect process inks and spot color separations; map spot plates to process CMYK.");
                            ui.add_space(4.0);
                            if let Some(tab) = self.active_tab() {
                                let inks = get_document_inks(&tab.doc);
                                egui::ScrollArea::vertical().max_height(200.0).show(ui, |ui| {
                                    for (i, ink) in inks.iter().enumerate() {
                                        ui.horizontal(|ui| {
                                            let badge = if ink.is_spot { "SPOT" } else { "PROCESS" };
                                            ui.label(RichText::new(format!("{}. {} [{}]", i + 1, ink.name, badge)).strong());
                                            ui.label(format!("CMYK: ({:.2}, {:.2}, {:.2}, {:.2})", ink.process_cmyk.0, ink.process_cmyk.1, ink.process_cmyk.2, ink.process_cmyk.3));
                                        });
                                    }
                                });
                            }
                            ui.add_space(8.0);
                            if ui.button("🔄 Convert All Spot Colors to Process CMYK").clicked() {
                                execute_convert_spots = true;
                            }
                        }
                        2 => {
                            ui.heading("Transparency Flattener");
                            ui.label("Flatten transparency groups and normalize opacity for prepress PostScript RIPs.");
                            ui.add_space(4.0);
                            ui.group(|ui| {
                                ui.horizontal(|ui| {
                                    ui.label("Raster / Vector Balance:");
                                    ui.add(egui::Slider::new(&mut self.transparency_flatten_config.raster_vector_balance, 0..=100).text("% vectors"));
                                });
                                ui.checkbox(&mut self.transparency_flatten_config.flatten_opacity, "Normalize ExtGState opacity (/CA /ca) to 1.0");
                                ui.checkbox(&mut self.transparency_flatten_config.normalize_blend_modes, "Reset blend modes (/BM) to /Normal");
                                ui.checkbox(&mut self.transparency_flatten_config.remove_page_transparency_groups, "Remove page-level /Group /Transparency dictionaries");
                            });
                            ui.add_space(8.0);
                            if ui.button("⚡ Flatten Document Transparency").clicked() {
                                execute_flatten = true;
                            }
                        }
                        _ => {
                            ui.heading("Trap Presets");
                            ui.label("Manage ink trapping tolerances to prevent press registration gaps.");
                            ui.add_space(4.0);
                            if let Some(tab) = self.active_tab() {
                                let presets = get_trap_presets(&tab.doc);
                                for (i, p) in presets.iter().enumerate() {
                                    ui.label(format!("{}. '{}': Trap Width: {} pt, Black Width: {} pt", i + 1, p.name, p.trap_width, p.black_width));
                                }
                            }
                            ui.separator();
                            ui.label(RichText::new("Configure New Trap Preset:").strong());
                            ui.horizontal(|ui| {
                                ui.label("Name:");
                                ui.text_edit_singleline(&mut self.new_trap_preset_name);
                            });
                            ui.horizontal(|ui| {
                                ui.label("Trap Width (pt):");
                                ui.add(egui::DragValue::new(&mut self.new_trap_width).speed(0.05).range(0.05..=2.0));
                                ui.label("Black Width (pt):");
                                ui.add(egui::DragValue::new(&mut self.new_black_width).speed(0.05).range(0.1..=4.0));
                            });
                            if ui.button("➕ Save Trap Preset").clicked() {
                                execute_add_trap = true;
                            }
                        }
                    }

                    ui.add_space(10.0);
                    if ui.button("Close").clicked() {
                        close = true;
                    }
                });

            if execute_add_marks {
                let cfg = self.printer_marks_config.clone();
                if let Some(tab) = self.active_tab_mut() {
                    match add_printer_marks(&mut tab.doc, &cfg) {
                        Ok(count) => {
                            tab.is_modified = true;
                            tab.invalidate_render();
                            self.status_message =
                                format!("Injected prepress printer marks into {} page(s)", count);
                        }
                        Err(e) => {
                            self.status_message = format!("Printer marks error: {}", e);
                        }
                    }
                }
            }

            if execute_convert_spots {
                let cfg = InkManagerConfig {
                    mappings: Vec::new(),
                    convert_all_spots_to_process: true,
                };
                if let Some(tab) = self.active_tab_mut() {
                    match apply_ink_manager(&mut tab.doc, &cfg) {
                        Ok(count) => {
                            tab.is_modified = true;
                            tab.invalidate_render();
                            self.status_message =
                                format!("Converted {} spot color space(s) to DeviceCMYK", count);
                        }
                        Err(e) => {
                            self.status_message = format!("Ink conversion error: {}", e);
                        }
                    }
                }
            }

            if execute_flatten {
                let cfg = self.transparency_flatten_config.clone();
                if let Some(tab) = self.active_tab_mut() {
                    match flatten_document_transparency(&mut tab.doc, &cfg) {
                        Ok(rep) => {
                            tab.is_modified = true;
                            tab.invalidate_render();
                            self.status_message = format!(
                                "Flattened transparency: {} ExtGState, {} groups across {} pages",
                                rep.extgstate_flattened,
                                rep.groups_removed,
                                rep.total_pages_processed
                            );
                        }
                        Err(e) => {
                            self.status_message = format!("Transparency flattening error: {}", e);
                        }
                    }
                }
            }

            if execute_add_trap {
                let preset = TrapPreset {
                    name: self.new_trap_preset_name.clone(),
                    trap_width: self.new_trap_width,
                    black_width: self.new_black_width,
                    ..Default::default()
                };
                if let Some(tab) = self.active_tab_mut() {
                    match set_trap_presets(&mut tab.doc, std::slice::from_ref(&preset)) {
                        Ok(()) => {
                            tab.is_modified = true;
                            self.status_message = format!("Saved trap preset '{}'", preset.name);
                        }
                        Err(e) => {
                            self.status_message = format!("Trap preset error: {}", e);
                        }
                    }
                }
            }

            if close {
                self.show_prepress_dialog = false;
            }
        }

        // Standards Compliance Wizard Modal (FR-22)
        if self.show_standards_dialog {
            let mut close = false;
            let mut execute_validate = false;
            let mut execute_convert = false;

            egui::Window::new("Standards Compliance Wizard (PDF/A, PDF/X, PDF/E)")
                .collapsible(false)
                .resizable(true)
                .default_size([580.0, 520.0])
                .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.heading("Target Standard Profile");
                    ui.horizontal(|ui| {
                        egui::ComboBox::from_id_salt("std_target_format")
                            .selected_text(match self.standards_format_idx {
                                0 => "PDF/A-1b (ISO 19005-1:2005 Archival)",
                                1 => "PDF/A-2b (ISO 19005-2:2011 Archival)",
                                2 => "PDF/A-3b (ISO 19005-3:2012 Archival)",
                                3 => "PDF/X-1a (ISO 15930-1:2001 Prepress CMYK)",
                                4 => "PDF/X-3 (ISO 15930-3:2002 Prepress)",
                                5 => "PDF/X-4 (ISO 15930-7:2010 Prepress)",
                                _ => "PDF/E-1 (ISO 24517-1:2008 Engineering)",
                            })
                            .show_ui(ui, |ui| {
                                ui.selectable_value(
                                    &mut self.standards_format_idx,
                                    0,
                                    "PDF/A-1b (ISO 19005-1:2005 Archival)",
                                );
                                ui.selectable_value(
                                    &mut self.standards_format_idx,
                                    1,
                                    "PDF/A-2b (ISO 19005-2:2011 Archival)",
                                );
                                ui.selectable_value(
                                    &mut self.standards_format_idx,
                                    2,
                                    "PDF/A-3b (ISO 19005-3:2012 Archival)",
                                );
                                ui.selectable_value(
                                    &mut self.standards_format_idx,
                                    3,
                                    "PDF/X-1a (ISO 15930-1:2001 Prepress CMYK)",
                                );
                                ui.selectable_value(
                                    &mut self.standards_format_idx,
                                    4,
                                    "PDF/X-3 (ISO 15930-3:2002 Prepress)",
                                );
                                ui.selectable_value(
                                    &mut self.standards_format_idx,
                                    5,
                                    "PDF/X-4 (ISO 15930-7:2010 Prepress)",
                                );
                                ui.selectable_value(
                                    &mut self.standards_format_idx,
                                    6,
                                    "PDF/E-1 (ISO 24517-1:2008 Engineering)",
                                );
                            });
                    });

                    ui.horizontal(|ui| {
                        ui.label("OutputIntent Profile:");
                        egui::ComboBox::from_id_salt("std_icc_profile")
                            .selected_text(match self.standards_icc_idx {
                                0 => "Auto (Format Default)",
                                1 => "sRGB IEC61966-2.1 (RGB)",
                                2 => "U.S. Web Coated (SWOP) v2 (CMYK)",
                                3 => "FOGRA39 / ISO 12647-2 (CMYK)",
                                4 => "GRACoL 2006 Coated (CMYK)",
                                _ => "Japan Color 2001 Coated (CMYK)",
                            })
                            .show_ui(ui, |ui| {
                                ui.selectable_value(
                                    &mut self.standards_icc_idx,
                                    0,
                                    "Auto (Format Default)",
                                );
                                ui.selectable_value(
                                    &mut self.standards_icc_idx,
                                    1,
                                    "sRGB IEC61966-2.1 (RGB)",
                                );
                                ui.selectable_value(
                                    &mut self.standards_icc_idx,
                                    2,
                                    "U.S. Web Coated (SWOP) v2 (CMYK)",
                                );
                                ui.selectable_value(
                                    &mut self.standards_icc_idx,
                                    3,
                                    "FOGRA39 / ISO 12647-2 (CMYK)",
                                );
                                ui.selectable_value(
                                    &mut self.standards_icc_idx,
                                    4,
                                    "GRACoL 2006 Coated (CMYK)",
                                );
                                ui.selectable_value(
                                    &mut self.standards_icc_idx,
                                    5,
                                    "Japan Color 2001 Coated (CMYK)",
                                );
                            });
                    });

                    ui.add_space(4.0);
                    ui.group(|ui| {
                        ui.checkbox(
                            &mut self.standards_remove_js,
                            "Remove JavaScript actions (mandatory for PDF/A-1)",
                        );
                        ui.checkbox(
                            &mut self.standards_remove_enc,
                            "Strip encryption passwords (mandatory for PDF/A)",
                        );
                        ui.checkbox(
                            &mut self.standards_convert_cmyk,
                            "Convert all colors to CMYK (mandatory for PDF/X-1a)",
                        );
                    });

                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        if ui.button("🔍 Validate Document Compliance").clicked() {
                            execute_validate = true;
                        }
                        if ui.button("⚙️ Convert & Enforce Standard").clicked() {
                            execute_convert = true;
                        }
                    });

                    if let Some(ref rep) = self.standards_validation_report {
                        ui.separator();
                        let status_text = if rep.is_compliant {
                            "✓ COMPLIANT"
                        } else {
                            "✗ NON-COMPLIANT"
                        };
                        ui.label(
                            RichText::new(format!(
                                "Validation Result for {}: {}",
                                rep.standard.display_name(),
                                status_text
                            ))
                            .strong(),
                        );
                        egui::ScrollArea::vertical()
                            .max_height(160.0)
                            .show(ui, |ui| {
                                for check in &rep.checks {
                                    ui.horizontal(|ui| {
                                        let badge = if check.passed { "PASS" } else { "FAIL" };
                                        ui.label(format!(
                                            "[{}] {}: {}",
                                            badge, check.code, check.description
                                        ));
                                    });
                                }
                            });
                    }

                    ui.add_space(8.0);
                    if ui.button("Close").clicked() {
                        close = true;
                    }
                });

            let std_enum = match self.standards_format_idx {
                0 => StandardFormat::PdfA1b,
                1 => StandardFormat::PdfA2b,
                2 => StandardFormat::PdfA3b,
                3 => StandardFormat::PdfX1a,
                4 => StandardFormat::PdfX3,
                5 => StandardFormat::PdfX4,
                _ => StandardFormat::PdfE1,
            };

            if execute_validate {
                if let Some(tab) = self.active_tab() {
                    let rep = validate_standard_compliance(&tab.doc, std_enum);
                    self.standards_validation_report = Some(rep);
                }
            }

            if execute_convert {
                let icc_enum = match self.standards_icc_idx {
                    1 => StandardIccProfile::SRgb,
                    2 => StandardIccProfile::USWebCoatedSWOP,
                    3 => StandardIccProfile::FOGRA39,
                    4 => StandardIccProfile::GRACoL2006,
                    5 => StandardIccProfile::JapanColor2001Coated,
                    _ => {
                        if std_enum == StandardFormat::PdfX1a {
                            StandardIccProfile::USWebCoatedSWOP
                        } else {
                            StandardIccProfile::SRgb
                        }
                    }
                };
                let cfg = StandardConversionConfig {
                    target: std_enum,
                    icc_profile: icc_enum,
                    remove_javascript: self.standards_remove_js,
                    remove_encryption: self.standards_remove_enc,
                    convert_colors_to_cmyk: self.standards_convert_cmyk,
                    embed_output_intent: true,
                    embed_xmp_metadata: true,
                };
                let result = if let Some(tab) = self.active_tab_mut() {
                    match convert_to_standard(&mut tab.doc, &cfg) {
                        Ok(rep) => {
                            tab.is_modified = true;
                            let val = validate_standard_compliance(&tab.doc, std_enum);
                            Ok((rep.actions_applied.len(), val))
                        }
                        Err(e) => Err(e.to_string()),
                    }
                } else {
                    Err("No active document".to_string())
                };

                match result {
                    Ok((count, val)) => {
                        self.status_message = format!(
                            "Converted document to {}: {} action(s) applied",
                            std_enum.display_name(),
                            count
                        );
                        self.standards_validation_report = Some(val);
                    }
                    Err(e) => {
                        self.status_message = format!("Standard conversion failed: {}", e);
                    }
                }
            }

            if close {
                self.show_standards_dialog = false;
            }
        }

        // In-Place Text Editor Dialog (FR-04.1 - FR-04.5, FR-04.12)
        if self.show_text_editor_dialog {
            let mut close = false;
            let mut apply_replace = false;
            let mut apply_insert = false;
            let mut refresh_spans = false;

            egui::Window::new("In-Place Text Editor")
                .collapsible(false)
                .resizable(true)
                .default_width(540.0)
                .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.label(RichText::new("Directly inspect, edit in-place, replace typography, or reflow paragraphs:").italics());
                    ui.add_space(4.0);

                    ui.horizontal(|ui| {
                        ui.selectable_value(&mut self.text_editor_block_mode, false, "✏️ In-Place Text Replace");
                        ui.selectable_value(&mut self.text_editor_block_mode, true, "📐 Formatted Paragraph Reflow");
                        if ui.button("🔄 Refresh Spans").clicked() {
                            refresh_spans = true;
                        }
                    });

                    ui.separator();

                    if !self.text_editor_block_mode {
                        ui.label(RichText::new("Extracted Text Spans on Current Page:").strong());
                        egui::ScrollArea::vertical().max_height(140.0).show(ui, |ui| {
                            if self.text_editor_spans.is_empty() {
                                ui.label(RichText::new("(No text spans detected in content stream)").weak());
                            } else {
                                for (idx, span) in self.text_editor_spans.iter().enumerate() {
                                    let is_selected = self.text_editor_selected_idx == Some(idx);
                                    let row_text = format!("\"{}\" • {} ({:.1}pt) at ({:.0}, {:.0})", span.text, span.font_name, span.font_size, span.x, span.y);
                                    if ui.selectable_label(is_selected, row_text).clicked() {
                                        self.text_editor_selected_idx = Some(idx);
                                        self.text_editor_target_text = span.text.clone();
                                        self.text_editor_new_text = span.text.clone();
                                        self.text_editor_font_size = span.font_size;
                                    }
                                }
                            }
                        });

                        ui.add_space(6.0);
                        ui.horizontal(|ui| {
                            ui.label("Find Text:");
                            ui.text_edit_singleline(&mut self.text_editor_target_text);
                        });
                        ui.horizontal(|ui| {
                            ui.label("Replace Text:");
                            ui.text_edit_singleline(&mut self.text_editor_new_text);
                        });

                        ui.horizontal(|ui| {
                            ui.label("Font Replacement:");
                            egui::ComboBox::from_id_salt("text_editor_font_combo")
                                .selected_text(match self.text_editor_font_idx {
                                    1 => "Helvetica-Bold",
                                    2 => "Helvetica-Oblique",
                                    3 => "Times-Roman",
                                    4 => "Times-Bold",
                                    5 => "Courier",
                                    6 => "Courier-Bold",
                                    _ => "Keep Original / Helvetica",
                                })
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(&mut self.text_editor_font_idx, 0, "Keep Original / Helvetica");
                                    ui.selectable_value(&mut self.text_editor_font_idx, 1, "Helvetica-Bold");
                                    ui.selectable_value(&mut self.text_editor_font_idx, 2, "Helvetica-Oblique");
                                    ui.selectable_value(&mut self.text_editor_font_idx, 3, "Times-Roman");
                                    ui.selectable_value(&mut self.text_editor_font_idx, 4, "Times-Bold");
                                    ui.selectable_value(&mut self.text_editor_font_idx, 5, "Courier");
                                    ui.selectable_value(&mut self.text_editor_font_idx, 6, "Courier-Bold");
                                });
                            ui.label("Size:");
                            ui.add(egui::DragValue::new(&mut self.text_editor_font_size).range(6.0..=72.0).speed(0.5));
                        });

                        let target_f_name = match self.text_editor_font_idx {
                            1 => "Helvetica-Bold",
                            2 => "Helvetica-Oblique",
                            3 => "Times-Roman",
                            4 => "Times-Bold",
                            5 => "Courier",
                            6 => "Courier-Bold",
                            _ => "Helvetica",
                        };
                        let sub_info = suggest_font_substitute(target_f_name);
                        ui.label(RichText::new(format!("Font mapping: {} ({})", sub_info.substitute_font, sub_info.note)).weak());

                        ui.add_space(8.0);
                        ui.horizontal(|ui| {
                            if ui.button("✓ Apply In-Place Replacement").clicked() {
                                apply_replace = true;
                                close = true;
                            }
                            if ui.button("Cancel").clicked() {
                                close = true;
                            }
                        });
                    } else {
                        ui.label(RichText::new("Insert or reflow a formatted paragraph with alignment and decorations:").italics());
                        ui.add_space(4.0);
                        ui.label("Paragraph Text:");
                        ui.text_edit_multiline(&mut self.text_editor_insert_text);

                        ui.horizontal(|ui| {
                            ui.label("Alignment:");
                            egui::ComboBox::from_id_salt("text_editor_align_combo")
                                .selected_text(match self.text_editor_align_idx {
                                    1 => "Center",
                                    2 => "Right",
                                    3 => "Justify",
                                    _ => "Left",
                                })
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(&mut self.text_editor_align_idx, 0, "Left");
                                    ui.selectable_value(&mut self.text_editor_align_idx, 1, "Center");
                                    ui.selectable_value(&mut self.text_editor_align_idx, 2, "Right");
                                    ui.selectable_value(&mut self.text_editor_align_idx, 3, "Justify");
                                });

                            ui.checkbox(&mut self.text_editor_underline, "Underline");
                            ui.checkbox(&mut self.text_editor_strikethrough, "Strikethrough");
                        });

                        ui.horizontal(|ui| {
                            ui.label("Font Size:");
                            ui.add(egui::DragValue::new(&mut self.text_editor_font_size).range(8.0..=48.0));
                        });

                        ui.add_space(8.0);
                        ui.horizontal(|ui| {
                            if ui.button("✓ Insert Formatted Paragraph").clicked() {
                                apply_insert = true;
                                close = true;
                            }
                            if ui.button("Cancel").clicked() {
                                close = true;
                            }
                        });
                    }
                });

            if refresh_spans {
                if let Some(tab) = self.active_tab() {
                    let curr = tab.current_page;
                    if let Ok(spans) = extract_page_text_spans(&tab.doc, curr) {
                        self.text_editor_spans = spans;
                    }
                }
            }

            if apply_replace {
                let target = self.text_editor_target_text.clone();
                let replacement = self.text_editor_new_text.clone();
                let font_name = match self.text_editor_font_idx {
                    1 => Some("Helvetica-Bold"),
                    2 => Some("Helvetica-Oblique"),
                    3 => Some("Times-Roman"),
                    4 => Some("Times-Bold"),
                    5 => Some("Courier"),
                    6 => Some("Courier-Bold"),
                    _ => None,
                };
                let font_sz = Some(self.text_editor_font_size);

                let mut replace_res = None;
                if let Some(tab) = self.active_tab_mut() {
                    let curr = tab.current_page;
                    match replace_text_span_in_page(
                        &mut tab.doc,
                        curr,
                        &target,
                        &replacement,
                        font_name,
                        font_sz,
                        None,
                    ) {
                        Ok(count) => {
                            tab.is_modified = true;
                            tab.invalidate_render();
                            replace_res = Some(Ok((count, curr + 1)));
                        }
                        Err(e) => {
                            replace_res = Some(Err(e.to_string()));
                        }
                    }
                }
                if let Some(res) = replace_res {
                    match res {
                        Ok((count, page_num)) => {
                            self.status_message = format!(
                                "Replaced {} text occurrence(s) in-place on page {}",
                                count, page_num
                            );
                        }
                        Err(err) => {
                            self.status_message = format!("Text replacement error: {}", err);
                        }
                    }
                }
            }

            if apply_insert {
                let text_to_insert = self.text_editor_insert_text.clone();
                let font_name = match self.text_editor_font_idx {
                    1 => "Helvetica-Bold",
                    2 => "Helvetica-Oblique",
                    3 => "Times-Roman",
                    4 => "Times-Bold",
                    5 => "Courier",
                    6 => "Courier-Bold",
                    _ => "Helvetica",
                };
                let alignment = match self.text_editor_align_idx {
                    1 => TextAlignment::Center,
                    2 => TextAlignment::Right,
                    3 => TextAlignment::Justify,
                    _ => TextAlignment::Left,
                };

                let opts = FormattedTextBlockOptions {
                    rect: [72.0, 300.0, 468.0, 150.0],
                    font_family: font_name.to_string(),
                    font_size: self.text_editor_font_size,
                    alignment,
                    line_spacing: self.text_editor_font_size * 1.35,
                    char_spacing: 0.0,
                    color: (0.1, 0.1, 0.1),
                    underline: self.text_editor_underline,
                    strikethrough: self.text_editor_strikethrough,
                };

                let mut insert_res = None;
                if let Some(tab) = self.active_tab_mut() {
                    let curr = tab.current_page;
                    match insert_formatted_text_block(&mut tab.doc, curr, &text_to_insert, &opts) {
                        Ok(_) => {
                            tab.is_modified = true;
                            tab.invalidate_render();
                            insert_res = Some(Ok(curr + 1));
                        }
                        Err(e) => {
                            insert_res = Some(Err(e.to_string()));
                        }
                    }
                }
                if let Some(res) = insert_res {
                    match res {
                        Ok(page_num) => {
                            self.status_message =
                                format!("Inserted formatted text block on page {}", page_num);
                        }
                        Err(err) => {
                            self.status_message = format!("Insert text block error: {}", err);
                        }
                    }
                }
            }

            if close {
                self.show_text_editor_dialog = false;
            }
        }

        // Markdown Document Generation Dialog (FR-02.5, FR-02.6, FR-03.1.6)
        if self.show_markdown_dialog {
            let mut close = false;
            let mut create_doc = false;

            egui::Window::new("Create PDF from Markdown")
                .collapsible(false)
                .resizable(true)
                .default_width(560.0)
                .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.label(
                        RichText::new(
                            "Generate publication-quality multi-page PDF documents from Markdown:",
                        )
                        .italics(),
                    );
                    ui.add_space(4.0);

                    ui.horizontal(|ui| {
                        ui.label("Document Title (Header):");
                        ui.text_edit_singleline(&mut self.markdown_title);
                    });

                    ui.horizontal(|ui| {
                        ui.label("Base Font Size:");
                        ui.add(
                            egui::DragValue::new(&mut self.markdown_font_size).range(9.0..=18.0),
                        );
                        ui.label("Page Margins:");
                        ui.add(egui::DragValue::new(&mut self.markdown_margin).range(18.0..=108.0));
                    });

                    ui.add_space(4.0);
                    ui.label("Markdown Content (# Headings, - Lists, > Quotes, ``` Code):");
                    ui.add(
                        egui::TextEdit::multiline(&mut self.markdown_input)
                            .desired_rows(12)
                            .desired_width(f32::INFINITY),
                    );

                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui.button("📄 Generate & Open PDF").clicked() {
                            create_doc = true;
                            close = true;
                        }
                        if ui.button("Cancel").clicked() {
                            close = true;
                        }
                    });
                });

            if create_doc {
                let md = self.markdown_input.clone();
                let title = if self.markdown_title.trim().is_empty() {
                    None
                } else {
                    Some(self.markdown_title.trim().to_string())
                };

                let opts = MarkdownPdfOptions {
                    page_size: paper_sizes::A4,
                    margin: self.markdown_margin,
                    base_font_size: self.markdown_font_size,
                    line_spacing: self.markdown_font_size * 1.35,
                    title,
                    include_page_numbers: true,
                };

                match create_pdf_from_markdown(&md, &opts) {
                    Ok(new_doc) => {
                        let page_cnt = new_doc.page_count();
                        let id = self.next_tab_id;
                        self.next_tab_id += 1;
                        let title = format!("Markdown-Doc-{}.pdf", id);
                        let mut tab = DocumentTab::new(id, title, new_doc, None);
                        tab.is_modified = true;
                        self.tabs.push(tab);
                        self.active_tab_idx = Some(self.tabs.len() - 1);
                        self.status_message = format!(
                            "Generated new {} page(s) PDF document from Markdown",
                            page_cnt
                        );
                    }
                    Err(e) => {
                        self.status_message = format!("Markdown PDF generation failed: {}", e);
                    }
                }
            }

            if close {
                self.show_markdown_dialog = false;
            }
        }

        // Image Editor & Transform Pipeline Dialog (FR-05.1 - FR-05.12)
        if self.show_image_editor_dialog {
            let mut close = false;
            let mut apply_transform = false;
            let mut replace_file = false;
            let mut delete_img = false;
            let mut apply_crop = false;
            let mut z_order_action: Option<ZOrderAction> = None;

            egui::Window::new("🖼️ Edit & Transform Images")
                .collapsible(false)
                .resizable(true)
                .default_width(580.0)
                .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    if self.image_editor_list.is_empty() {
                        ui.label(RichText::new("No image XObjects found on the current page.").italics());
                        ui.add_space(8.0);
                        if ui.button("Close").clicked() {
                            close = true;
                        }
                        return;
                    }

                    ui.label(RichText::new("Select an image to inspect, reposition, resize, rotate, crop, or adjust opacity:").small());
                    ui.add_space(4.0);

                    // Image selector combo / radio list
                    ui.horizontal(|ui| {
                        ui.label("Active Image:");
                        let cur_sel = self.image_editor_selected_idx.unwrap_or(0);
                        let cur_name = self.image_editor_list.get(cur_sel).map(|im| im.name.clone()).unwrap_or_default();
                        egui::ComboBox::from_id_salt("image_editor_selector")
                            .selected_text(format!("/{}", cur_name))
                            .show_ui(ui, |ui| {
                                for (i, im) in self.image_editor_list.iter().enumerate() {
                                    if ui.selectable_label(self.image_editor_selected_idx == Some(i), format!("/{} ({}x{} px)", im.name, im.pixel_width, im.pixel_height)).clicked() {
                                        self.image_editor_selected_idx = Some(i);
                                        self.image_editor_pos_x = im.display_x;
                                        self.image_editor_pos_y = im.display_y;
                                        self.image_editor_width = im.display_width;
                                        self.image_editor_height = im.display_height;
                                        self.image_editor_rotation = im.rotation_deg;
                                        self.image_editor_opacity = im.opacity;
                                        self.image_editor_flip_h = im.flipped_h;
                                        self.image_editor_flip_v = im.flipped_v;
                                    }
                                }
                            });
                    });

                    if let Some(sel_idx) = self.image_editor_selected_idx {
                        if let Some(im) = self.image_editor_list.get(sel_idx).cloned() {
                            ui.separator();
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(format!("Pixel Dimensions: {}x{} px", im.pixel_width, im.pixel_height)).monospace());
                                ui.separator();
                                ui.label(RichText::new(format!("ColorSpace: {}", im.color_space)).monospace());
                                ui.separator();
                                ui.label(RichText::new(format!("Filter: {}", im.filter.as_deref().unwrap_or("Raw"))).monospace());
                            });
                            ui.separator();

                            // Position & Snapping
                            ui.horizontal(|ui| {
                                ui.label("Position X:");
                                ui.add(egui::DragValue::new(&mut self.image_editor_pos_x).speed(1.0).range(0.0..=2000.0));
                                ui.label("Y:");
                                ui.add(egui::DragValue::new(&mut self.image_editor_pos_y).speed(1.0).range(0.0..=2000.0));
                                ui.checkbox(&mut self.image_editor_snapping_enabled, "Magnetic Snap");
                            });

                            // Alignment Guide Snapping Calculation (FR-05.9)
                            if self.image_editor_snapping_enabled {
                                let (page_w, page_h) = if let Some(tab) = self.active_tab() {
                                    if let Some(p) = tab.doc.get_page(tab.current_page) {
                                        ((p.crop_box[2] - p.crop_box[0]).abs(), (p.crop_box[3] - p.crop_box[1]).abs())
                                    } else {
                                        (612.0, 792.0)
                                    }
                                } else {
                                    (612.0, 792.0)
                                };

                                let other_boxes: Vec<[f64; 4]> = self.image_editor_list
                                    .iter()
                                    .enumerate()
                                    .filter(|(i, _)| *i != sel_idx)
                                    .map(|(_, o)| [o.display_x, o.display_y, o.display_width, o.display_height])
                                    .collect();

                                let snapped = snap_to_alignment_guides(
                                    (self.image_editor_pos_x, self.image_editor_pos_y),
                                    (self.image_editor_width, self.image_editor_height),
                                    [0.0, 0.0, page_w, page_h],
                                    &other_boxes,
                                    6.0,
                                );

                                if !snapped.active_guides.is_empty() {
                                    self.image_editor_pos_x = snapped.snapped_x;
                                    self.image_editor_pos_y = snapped.snapped_y;
                                    self.image_editor_active_guides = snapped.active_guides.clone();

                                    ui.horizontal(|ui| {
                                        ui.label(RichText::new("Snapping Guides:").color(Color32::from_rgb(56, 161, 105)));
                                        for g in &snapped.active_guides {
                                            ui.label(RichText::new(format!("[{} @ {:.1} pt]", g.label, g.coordinate)).small());
                                        }
                                    });
                                }
                            }

                            // Size & Aspect Ratio Lock (FR-05.4)
                            ui.horizontal(|ui| {
                                ui.label("Width (pt):");
                                let orig_w = self.image_editor_width;
                                if ui.add(egui::DragValue::new(&mut self.image_editor_width).speed(1.0).range(5.0..=2000.0)).changed() && self.image_editor_lock_aspect && orig_w > 0.0 {
                                    let ratio = im.display_height / im.display_width;
                                    self.image_editor_height = self.image_editor_width * ratio;
                                }
                                ui.label("Height (pt):");
                                if ui.add(egui::DragValue::new(&mut self.image_editor_height).speed(1.0).range(5.0..=2000.0)).changed() && self.image_editor_lock_aspect && im.display_height > 0.0 {
                                    let ratio = im.display_width / im.display_height;
                                    self.image_editor_width = self.image_editor_height * ratio;
                                }
                                ui.checkbox(&mut self.image_editor_lock_aspect, "Lock Aspect Ratio");
                            });

                            // Rotation (FR-05.5) & Flip (FR-05.7)
                            ui.horizontal(|ui| {
                                ui.label("Rotation (deg):");
                                ui.add(egui::Slider::new(&mut self.image_editor_rotation, 0.0..=360.0).step_by(0.1).suffix("°"));
                                if ui.button("0°").clicked() { self.image_editor_rotation = 0.0; }
                                if ui.button("90°").clicked() { self.image_editor_rotation = 90.0; }
                                if ui.button("180°").clicked() { self.image_editor_rotation = 180.0; }
                                if ui.button("270°").clicked() { self.image_editor_rotation = 270.0; }
                            });

                            ui.horizontal(|ui| {
                                ui.checkbox(&mut self.image_editor_flip_h, "Flip Horizontal");
                                ui.checkbox(&mut self.image_editor_flip_v, "Flip Vertical");
                            });

                            // Opacity (FR-05.10)
                            ui.horizontal(|ui| {
                                ui.label("Opacity / Transparency:");
                                ui.add(egui::Slider::new(&mut self.image_editor_opacity, 0.05..=1.0).text("Alpha"));
                            });

                            // Vector Cropping (FR-05.6)
                            ui.collapsing("Vector Cropping Frame", |ui| {
                                ui.horizontal(|ui| {
                                    ui.label("Crop X:");
                                    ui.add(egui::DragValue::new(&mut self.image_editor_crop_x));
                                    ui.label("Y:");
                                    ui.add(egui::DragValue::new(&mut self.image_editor_crop_y));
                                    ui.label("W:");
                                    ui.add(egui::DragValue::new(&mut self.image_editor_crop_w));
                                    ui.label("H:");
                                    ui.add(egui::DragValue::new(&mut self.image_editor_crop_h));
                                });
                                if ui.button("✂ Apply Vector Crop").clicked() {
                                    apply_crop = true;
                                }
                            });

                            // Z-Order Management (FR-05.11)
                            ui.horizontal(|ui| {
                                ui.label("Z-Order (Layering):");
                                if ui.button("⬆ Bring to Front").clicked() { z_order_action = Some(ZOrderAction::BringToFront); }
                                if ui.button("⬇ Send to Back").clicked() { z_order_action = Some(ZOrderAction::SendToBack); }
                                if ui.button("Move Forward").clicked() { z_order_action = Some(ZOrderAction::MoveForward); }
                                if ui.button("Move Backward").clicked() { z_order_action = Some(ZOrderAction::MoveBackward); }
                            });

                            ui.separator();

                            // Action buttons
                            ui.horizontal(|ui| {
                                if ui.button("💾 Apply Transforms").clicked() {
                                    apply_transform = true;
                                }
                                if ui.button("🔄 Replace with New Image File...").clicked() {
                                    replace_file = true;
                                }
                                if ui.button("🗑️ Delete Image").clicked() {
                                    delete_img = true;
                                }
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    if ui.button("Close").clicked() {
                                        close = true;
                                    }
                                });
                            });
                        }
                    }
                });

            // Handle actions
            if let Some(sel_idx) = self.image_editor_selected_idx {
                if let Some(im) = self.image_editor_list.get(sel_idx).cloned() {
                    if apply_transform {
                        let opts = ImageTransformOptions {
                            position: Some((self.image_editor_pos_x, self.image_editor_pos_y)),
                            size: Some((self.image_editor_width, self.image_editor_height)),
                            rotation_deg: Some(self.image_editor_rotation),
                            flip_h: self.image_editor_flip_h,
                            flip_v: self.image_editor_flip_v,
                            lock_aspect_ratio: self.image_editor_lock_aspect,
                        };

                        let opacity = self.image_editor_opacity;
                        if let Some(tab) = self.active_tab_mut() {
                            let curr = tab.current_page;
                            let _ = transform_page_image(&mut tab.doc, curr, &im.name, &opts);
                            let _ = set_page_image_opacity(&mut tab.doc, curr, &im.name, opacity);
                            tab.is_modified = true;
                            tab.invalidate_render();
                            self.status_message =
                                format!("Transformed image '/{}' on page {}", im.name, curr + 1);
                        }
                        self.open_image_editor_dialog();
                    }

                    if apply_crop {
                        let cr = [
                            self.image_editor_crop_x,
                            self.image_editor_crop_y,
                            self.image_editor_crop_w,
                            self.image_editor_crop_h,
                        ];
                        if let Some(tab) = self.active_tab_mut() {
                            let curr = tab.current_page;
                            let _ = crop_page_image(&mut tab.doc, curr, &im.name, cr);
                            tab.is_modified = true;
                            tab.invalidate_render();
                            self.status_message = format!(
                                "Applied vector crop to image '/{}' on page {}",
                                im.name,
                                curr + 1
                            );
                        }
                        self.open_image_editor_dialog();
                    }

                    if let Some(action) = z_order_action {
                        if let Some(tab) = self.active_tab_mut() {
                            let curr = tab.current_page;
                            let _ = reorder_page_image(&mut tab.doc, curr, &im.name, action);
                            tab.is_modified = true;
                            tab.invalidate_render();
                            self.status_message = format!(
                                "Updated Z-order for image '/{}' on page {}",
                                im.name,
                                curr + 1
                            );
                        }
                        self.open_image_editor_dialog();
                    }

                    if replace_file {
                        if let Some(new_file) = rfd::FileDialog::new()
                            .add_filter(
                                "Images",
                                &["png", "jpg", "jpeg", "bmp", "tiff", "webp", "gif"],
                            )
                            .pick_file()
                        {
                            if let Some(tab) = self.active_tab_mut() {
                                let curr = tab.current_page;
                                match replace_page_image_from_file(
                                    &mut tab.doc,
                                    curr,
                                    &im.name,
                                    &new_file,
                                ) {
                                    Ok(_) => {
                                        tab.is_modified = true;
                                        tab.invalidate_render();
                                        self.status_message = format!(
                                            "Replaced image '/{}' with '{}'",
                                            im.name,
                                            new_file.display()
                                        );
                                    }
                                    Err(e) => {
                                        self.status_message =
                                            format!("Failed to replace image: {}", e);
                                    }
                                }
                            }
                            self.open_image_editor_dialog();
                        }
                    }

                    if delete_img {
                        if let Some(tab) = self.active_tab_mut() {
                            let curr = tab.current_page;
                            let _ = delete_page_image(&mut tab.doc, curr, &im.name);
                            tab.is_modified = true;
                            tab.invalidate_render();
                            self.status_message =
                                format!("Deleted image '/{}' from page {}", im.name, curr + 1);
                        }
                        self.open_image_editor_dialog();
                    }
                }
            }

            if close {
                self.show_image_editor_dialog = false;
            }
        }

        // Rubber Stamps & Sign Here Dialog (FR-10.1 - FR-10.7)
        if self.show_stamp_dialog {
            let mut close = false;
            let mut apply_stamp = false;

            egui::Window::new("🏷️ Rubber Stamps & Sign Here")
                .collapsible(false)
                .resizable(true)
                .default_width(520.0)
                .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.label(
                        RichText::new(
                            "Insert ISO 32000-compliant vector rubber stamps and sign-here marks:",
                        )
                        .small(),
                    );
                    ui.add_space(4.0);

                    // Category tabs
                    ui.horizontal(|ui| {
                        let categories =
                            ["Standard Business", "Sign Here", "Dynamic Stamps", "Custom"];
                        for (idx, cat) in categories.iter().enumerate() {
                            if ui
                                .selectable_label(self.stamp_category_idx == idx, *cat)
                                .clicked()
                            {
                                self.stamp_category_idx = idx;
                            }
                        }
                    });
                    ui.separator();

                    let stamp_presets: &[StandardStamp] = match self.stamp_category_idx {
                        0 => &[
                            StandardStamp::Approved,
                            StandardStamp::NotApproved,
                            StandardStamp::Draft,
                            StandardStamp::Final,
                            StandardStamp::Confidential,
                            StandardStamp::ForPublicRelease,
                            StandardStamp::Void,
                            StandardStamp::PreliminaryResults,
                            StandardStamp::InformationOnly,
                            StandardStamp::ForComment,
                            StandardStamp::Expired,
                        ],
                        1 => &[
                            StandardStamp::SignHere,
                            StandardStamp::InitialHere,
                            StandardStamp::Witness,
                            StandardStamp::Accepted,
                        ],
                        2 => &[
                            StandardStamp::Approved,
                            StandardStamp::Confidential,
                            StandardStamp::Accepted,
                            StandardStamp::Draft,
                        ],
                        _ => &[StandardStamp::Custom(self.stamp_custom_label.clone())],
                    };

                    // Grid palette of stamp buttons
                    ui.label("Select Stamp Preset:");
                    ui.horizontal_wrapped(|ui| {
                        for (idx, s) in stamp_presets.iter().enumerate() {
                            let col = s.default_color();
                            let is_selected = self.stamp_type_idx == idx;
                            let btn_text = RichText::new(s.label())
                                .color(Color32::from_rgb(
                                    (col.0 * 255.0) as u8,
                                    (col.1 * 255.0) as u8,
                                    (col.2 * 255.0) as u8,
                                ))
                                .strong();

                            if ui.selectable_label(is_selected, btn_text).clicked() {
                                self.stamp_type_idx = idx;
                                let (def_w, def_h) = s.default_dimensions();
                                self.stamp_width = def_w;
                                self.stamp_height = def_h;
                            }
                        }
                    });

                    if self.stamp_category_idx == 3 {
                        ui.horizontal(|ui| {
                            ui.label("Custom Stamp Text:");
                            ui.text_edit_singleline(&mut self.stamp_custom_label);
                        });
                    }

                    ui.separator();

                    // Dynamic Stamp Options (FR-10.3)
                    ui.checkbox(
                        &mut self.stamp_include_dynamic_date,
                        "Embed Dynamic User & Timestamp",
                    );
                    if self.stamp_include_dynamic_date {
                        ui.horizontal(|ui| {
                            ui.label("Author / Reviewer Name:");
                            ui.text_edit_singleline(&mut self.stamp_author_input);
                        });
                        let now_str = chrono::Utc::now().format("%Y-%m-%d %H:%M UTC").to_string();
                        ui.label(
                            RichText::new(format!(
                                "Preview: By: {} | {}",
                                self.stamp_author_input, now_str
                            ))
                            .small()
                            .italics(),
                        );
                    }

                    ui.separator();

                    // Position, Dimensions, Tilt, Opacity
                    ui.horizontal(|ui| {
                        ui.label("Position X:");
                        ui.add(egui::DragValue::new(&mut self.stamp_pos_x));
                        ui.label("Y:");
                        ui.add(egui::DragValue::new(&mut self.stamp_pos_y));
                    });

                    ui.horizontal(|ui| {
                        ui.label("Width:");
                        ui.add(egui::DragValue::new(&mut self.stamp_width).range(40.0..=600.0));
                        ui.label("Height:");
                        ui.add(egui::DragValue::new(&mut self.stamp_height).range(20.0..=300.0));
                        ui.label("Tilt Angle:");
                        ui.add(
                            egui::DragValue::new(&mut self.stamp_tilt_deg)
                                .range(-45.0..=45.0)
                                .suffix("°"),
                        );
                    });

                    ui.horizontal(|ui| {
                        ui.label("Opacity:");
                        ui.add(egui::Slider::new(&mut self.stamp_opacity, 0.2..=1.0));
                    });

                    // Pop-up Note (FR-10.7)
                    ui.horizontal(|ui| {
                        ui.label("Pop-up Note:");
                        ui.text_edit_singleline(&mut self.stamp_note_input);
                    });

                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui.button("🏷️ Stamp Current Page").clicked() {
                            apply_stamp = true;
                            close = true;
                        }
                        if ui.button("Cancel").clicked() {
                            close = true;
                        }
                    });
                });

            if apply_stamp {
                let chosen_stamp = match self.stamp_category_idx {
                    0 => match self.stamp_type_idx {
                        0 => StandardStamp::Approved,
                        1 => StandardStamp::NotApproved,
                        2 => StandardStamp::Draft,
                        3 => StandardStamp::Final,
                        4 => StandardStamp::Confidential,
                        5 => StandardStamp::ForPublicRelease,
                        6 => StandardStamp::Void,
                        7 => StandardStamp::PreliminaryResults,
                        8 => StandardStamp::InformationOnly,
                        9 => StandardStamp::ForComment,
                        _ => StandardStamp::Expired,
                    },
                    1 => match self.stamp_type_idx {
                        0 => StandardStamp::SignHere,
                        1 => StandardStamp::InitialHere,
                        2 => StandardStamp::Witness,
                        _ => StandardStamp::Accepted,
                    },
                    2 => match self.stamp_type_idx {
                        0 => StandardStamp::Approved,
                        1 => StandardStamp::Confidential,
                        2 => StandardStamp::Accepted,
                        _ => StandardStamp::Draft,
                    },
                    _ => StandardStamp::Custom(self.stamp_custom_label.clone()),
                };

                let config = StampConfig {
                    stamp_type: chosen_stamp.clone(),
                    rect: [
                        self.stamp_pos_x,
                        self.stamp_pos_y,
                        self.stamp_width,
                        self.stamp_height,
                    ],
                    author: if self.stamp_author_input.trim().is_empty() {
                        None
                    } else {
                        Some(self.stamp_author_input.trim().to_string())
                    },
                    dynamic_date: if self.stamp_include_dynamic_date {
                        Some(chrono::Utc::now())
                    } else {
                        None
                    },
                    custom_text: if self.stamp_category_idx == 3
                        && !self.stamp_custom_label.trim().is_empty()
                    {
                        Some(self.stamp_custom_label.trim().to_string())
                    } else {
                        None
                    },
                    color: None,
                    opacity: self.stamp_opacity,
                    rotation_deg: self.stamp_tilt_deg,
                    popup_note: if self.stamp_note_input.trim().is_empty() {
                        None
                    } else {
                        Some(self.stamp_note_input.trim().to_string())
                    },
                };

                if let Some(tab) = self.active_tab_mut() {
                    let curr = tab.current_page;
                    match add_stamp_annotation(&mut tab.doc, curr, &config) {
                        Ok(_) => {
                            tab.is_modified = true;
                            tab.invalidate_render();
                            self.status_message = format!(
                                "Applied '{}' stamp on page {}",
                                chosen_stamp.label(),
                                curr + 1
                            );
                        }
                        Err(e) => {
                            self.status_message = format!("Failed to apply stamp: {}", e);
                        }
                    }
                }
            }

            if close {
                self.show_stamp_dialog = false;
            }
        }

        // Drawing & Shape Tools Dialog (FR-09.1 - FR-09.12)
        if self.show_drawing_dialog {
            let mut close = false;
            let mut apply_shape = false;
            let mut apply_ink = false;
            let mut apply_erase = false;

            egui::Window::new("📐 Drawing & Shape Tools")
                .collapsible(false)
                .resizable(true)
                .default_width(560.0)
                .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.label(RichText::new("Insert ISO 32000-compliant vector shapes, cloudy borders, ink drawings, and line endings:").small());
                    ui.add_space(4.0);

                    // Category Tabs
                    ui.horizontal(|ui| {
                        let categories = [
                            "Shapes",
                            "Polygons",
                            "Freehand Ink",
                            "Eraser Tool",
                        ];
                        for (idx, cat) in categories.iter().enumerate() {
                            if ui.selectable_label(self.drawing_category_idx == idx, *cat).clicked() {
                                self.drawing_category_idx = idx;
                            }
                        }
                    });
                    ui.separator();

                    match self.drawing_category_idx {
                        0 => {
                            // Geometric Shapes
                            ui.horizontal(|ui| {
                                ui.label("Shape Type:");
                                let shapes = ["Rectangle", "Circle / Ellipse", "Line", "Single Arrow", "Double Arrow"];
                                for (idx, s) in shapes.iter().enumerate() {
                                    if ui.selectable_label(self.drawing_shape_idx == idx, *s).clicked() {
                                        self.drawing_shape_idx = idx;
                                    }
                                }
                            });
                            ui.separator();

                            if self.drawing_shape_idx < 2 {
                                // Rectangle or Circle
                                ui.horizontal(|ui| {
                                    ui.label("Pos X:");
                                    ui.add(egui::DragValue::new(&mut self.drawing_pos_x));
                                    ui.label("Y:");
                                    ui.add(egui::DragValue::new(&mut self.drawing_pos_y));
                                    ui.label("Width:");
                                    ui.add(egui::DragValue::new(&mut self.drawing_width).range(10.0..=1000.0));
                                    ui.label("Height:");
                                    ui.add(egui::DragValue::new(&mut self.drawing_height).range(10.0..=1000.0));
                                });
                            } else {
                                // Line or Arrow
                                ui.horizontal(|ui| {
                                    ui.label("Start X:");
                                    ui.add(egui::DragValue::new(&mut self.drawing_line_start_x));
                                    ui.label("Y:");
                                    ui.add(egui::DragValue::new(&mut self.drawing_line_start_y));
                                    ui.label("End X:");
                                    ui.add(egui::DragValue::new(&mut self.drawing_line_end_x));
                                    ui.label("Y:");
                                    ui.add(egui::DragValue::new(&mut self.drawing_line_end_y));
                                });

                                ui.horizontal(|ui| {
                                    let endings = ["None", "OpenArrow", "ClosedArrow", "Circle", "Diamond", "Square", "Butt", "Slash"];
                                    ui.label("Start Ending:");
                                    egui::ComboBox::from_id_salt("line_start_style")
                                        .selected_text(endings[self.drawing_line_start_style_idx.min(endings.len() - 1)])
                                        .show_ui(ui, |ui| {
                                            for (idx, name) in endings.iter().enumerate() {
                                                ui.selectable_value(&mut self.drawing_line_start_style_idx, idx, *name);
                                            }
                                        });

                                    ui.label("End Ending:");
                                    egui::ComboBox::from_id_salt("line_end_style")
                                        .selected_text(endings[self.drawing_line_end_style_idx.min(endings.len() - 1)])
                                        .show_ui(ui, |ui| {
                                            for (idx, name) in endings.iter().enumerate() {
                                                ui.selectable_value(&mut self.drawing_line_end_style_idx, idx, *name);
                                            }
                                        });
                                });
                            }

                            ui.separator();
                            ui.horizontal(|ui| {
                                ui.label("Stroke Width:");
                                ui.add(egui::DragValue::new(&mut self.drawing_stroke_width).range(0.5..=30.0).suffix(" pt"));
                                ui.label("Stroke Color:");
                                ui.color_edit_button_rgb(&mut self.drawing_stroke_color);
                            });

                            if self.drawing_shape_idx < 2 {
                                ui.horizontal(|ui| {
                                    ui.checkbox(&mut self.drawing_has_fill, "Fill Interior (/IC)");
                                    if self.drawing_has_fill {
                                        ui.color_edit_button_rgb(&mut self.drawing_fill_color);
                                    }
                                });

                                ui.horizontal(|ui| {
                                    ui.checkbox(&mut self.drawing_is_cloudy, "☁️ Cloudy / Scalloped Border");
                                    if self.drawing_is_cloudy {
                                        ui.label("Intensity:");
                                        ui.add(egui::Slider::new(&mut self.drawing_cloudy_intensity, 0.5..=3.0));
                                    }
                                });
                            }

                            ui.horizontal(|ui| {
                                ui.label("Border Dash:");
                                let dashes = ["Solid", "Dashed [4 4]", "Dotted [2 2]", "Dash-Dot [6 3 2 3]"];
                                for (idx, d) in dashes.iter().enumerate() {
                                    if ui.selectable_label(self.drawing_border_dash_idx == idx, *d).clicked() {
                                        self.drawing_border_dash_idx = idx;
                                    }
                                }
                            });

                            ui.horizontal(|ui| {
                                ui.label("Opacity:");
                                ui.add(egui::Slider::new(&mut self.drawing_opacity, 0.1..=1.0));
                                ui.checkbox(&mut self.drawing_locked, "🔒 Lock Annotation");
                            });

                            ui.horizontal(|ui| {
                                ui.label("Pop-up Note:");
                                ui.text_edit_singleline(&mut self.drawing_note_input);
                            });

                            ui.add_space(8.0);
                            ui.horizontal(|ui| {
                                if ui.button("📐 Insert Shape onto Current Page").clicked() {
                                    apply_shape = true;
                                    close = true;
                                }
                                if ui.button("Cancel").clicked() {
                                    close = true;
                                }
                            });
                        }
                        1 => {
                            // Polygons & Polylines
                            ui.label("Closed Polygon or Open Polyline:");
                            ui.horizontal(|ui| {
                                ui.label("Vertices Coordinates (x1,y1; x2,y2; ...):");
                            });
                            ui.text_edit_singleline(&mut self.drawing_polygon_vertices_input);

                            ui.horizontal(|ui| {
                                ui.label("Presets:");
                                if ui.small_button("Triangle").clicked() {
                                    self.drawing_polygon_vertices_input = "100,100; 200,250; 300,100".to_string();
                                }
                                if ui.small_button("Pentagon").clicked() {
                                    self.drawing_polygon_vertices_input = "200,300; 280,240; 250,150; 150,150; 120,240".to_string();
                                }
                                if ui.small_button("Star").clicked() {
                                    self.drawing_polygon_vertices_input = "200,320; 225,250; 300,250; 240,205; 260,135; 200,175; 140,135; 160,205; 100,250; 175,250".to_string();
                                }
                                if ui.small_button("Zigzag Polyline").clicked() {
                                    self.drawing_polygon_vertices_input = "80,200; 140,260; 200,200; 260,260; 320,200".to_string();
                                }
                            });
                            ui.separator();

                            ui.horizontal(|ui| {
                                ui.label("Stroke Width:");
                                ui.add(egui::DragValue::new(&mut self.drawing_stroke_width).range(0.5..=30.0).suffix(" pt"));
                                ui.label("Stroke Color:");
                                ui.color_edit_button_rgb(&mut self.drawing_stroke_color);
                            });

                            ui.horizontal(|ui| {
                                ui.checkbox(&mut self.drawing_has_fill, "Fill Polygon Interior");
                                if self.drawing_has_fill {
                                    ui.color_edit_button_rgb(&mut self.drawing_fill_color);
                                }
                                ui.checkbox(&mut self.drawing_is_cloudy, "☁️ Cloudy Perimeter");
                            });

                            ui.horizontal(|ui| {
                                ui.label("Opacity:");
                                ui.add(egui::Slider::new(&mut self.drawing_opacity, 0.1..=1.0));
                                ui.checkbox(&mut self.drawing_locked, "🔒 Lock");
                            });

                            ui.add_space(8.0);
                            ui.horizontal(|ui| {
                                if ui.button("🔷 Insert Polygon / Polyline").clicked() {
                                    apply_shape = true;
                                    close = true;
                                }
                                if ui.button("Cancel").clicked() {
                                    close = true;
                                }
                            });
                        }
                        2 => {
                            // Freehand Ink Tool
                            ui.label("Freehand Ink Drawing:");
                            ui.label(RichText::new("Sequence of point coordinates: x1,y1; x2,y2; ... (separate multiple strokes with '|')").small());
                            ui.text_edit_multiline(&mut self.drawing_ink_strokes_input);

                            ui.horizontal(|ui| {
                                ui.label("Ink Presets:");
                                if ui.small_button("Checkmark").clicked() {
                                    self.drawing_ink_strokes_input = "100,150; 120,130; 160,180".to_string();
                                }
                                if ui.small_button("Underline Wave").clicked() {
                                    self.drawing_ink_strokes_input = "72,200; 120,205; 170,195; 220,205; 270,195; 320,200".to_string();
                                }
                                if ui.small_button("Circle Loop").clicked() {
                                    self.drawing_ink_strokes_input = "150,200; 170,230; 210,240; 240,210; 230,170; 190,160; 160,180".to_string();
                                }
                            });
                            ui.separator();

                            ui.horizontal(|ui| {
                                ui.label("Stroke Width:");
                                ui.add(egui::DragValue::new(&mut self.drawing_stroke_width).range(0.5..=30.0).suffix(" pt"));
                                ui.label("Ink Color:");
                                ui.color_edit_button_rgb(&mut self.drawing_stroke_color);
                            });

                            ui.horizontal(|ui| {
                                ui.checkbox(&mut self.drawing_ink_smooth, "✨ Apply Catmull-Rom Smoothing");
                                ui.label("Opacity:");
                                ui.add(egui::Slider::new(&mut self.drawing_opacity, 0.1..=1.0));
                                ui.checkbox(&mut self.drawing_locked, "🔒 Lock");
                            });

                            ui.add_space(8.0);
                            ui.horizontal(|ui| {
                                if ui.button("✒️ Add Ink Drawing").clicked() {
                                    apply_ink = true;
                                    close = true;
                                }
                                if ui.button("Cancel").clicked() {
                                    close = true;
                                }
                            });
                        }
                        _ => {
                            // Eraser Tool
                            ui.label("Stroke & Point Eraser:");
                            ui.label(RichText::new("Removes intersecting freehand ink strokes at the specified page coordinates.").small());
                            ui.separator();

                            ui.horizontal(|ui| {
                                ui.label("Eraser X:");
                                ui.add(egui::DragValue::new(&mut self.drawing_eraser_x));
                                ui.label("Y:");
                                ui.add(egui::DragValue::new(&mut self.drawing_eraser_y));
                                ui.label("Radius:");
                                ui.add(egui::Slider::new(&mut self.drawing_eraser_radius, 5.0..=100.0).suffix(" pt"));
                            });

                            ui.add_space(8.0);
                            ui.horizontal(|ui| {
                                if ui.button("🧹 Erase Ink Strokes at Coordinates").clicked() {
                                    apply_erase = true;
                                    close = true;
                                }
                                if ui.button("Cancel").clicked() {
                                    close = true;
                                }
                            });
                        }
                    }
                });

            if apply_shape {
                let dash = match self.drawing_border_dash_idx {
                    1 => BorderStyle::Dashed(vec![4.0, 4.0]),
                    2 => BorderStyle::Dashed(vec![2.0, 2.0]),
                    3 => BorderStyle::Dashed(vec![6.0, 3.0, 2.0, 3.0]),
                    _ => BorderStyle::Solid,
                };
                let be = if self.drawing_is_cloudy {
                    BorderEffect::Cloudy {
                        intensity: self.drawing_cloudy_intensity,
                    }
                } else {
                    BorderEffect::None
                };

                let shape_type = if self.drawing_category_idx == 1 {
                    let pts: Vec<[f64; 2]> = self
                        .drawing_polygon_vertices_input
                        .split(';')
                        .filter_map(|pair| {
                            let p: Vec<&str> = pair.split(',').map(|s| s.trim()).collect();
                            if p.len() == 2 {
                                if let (Ok(x), Ok(y)) = (p[0].parse::<f64>(), p[1].parse::<f64>()) {
                                    return Some([x, y]);
                                }
                            }
                            None
                        })
                        .collect();
                    if self.drawing_polygon_vertices_input.contains("Polyline") || pts.len() < 3 {
                        ShapeType::PolyLine {
                            vertices: if pts.len() >= 2 {
                                pts
                            } else {
                                vec![[100.0, 100.0], [200.0, 200.0]]
                            },
                            start_style: LineEndingStyle::None,
                            end_style: LineEndingStyle::None,
                        }
                    } else {
                        ShapeType::Polygon { vertices: pts }
                    }
                } else {
                    let map_ending = |idx: usize| match idx {
                        1 => LineEndingStyle::OpenArrow,
                        2 => LineEndingStyle::ClosedArrow,
                        3 => LineEndingStyle::Circle,
                        4 => LineEndingStyle::Diamond,
                        5 => LineEndingStyle::Square,
                        6 => LineEndingStyle::Butt,
                        7 => LineEndingStyle::Slash,
                        _ => LineEndingStyle::None,
                    };
                    let custom_start = map_ending(self.drawing_line_start_style_idx);
                    let custom_end = map_ending(self.drawing_line_end_style_idx);

                    match self.drawing_shape_idx {
                        0 => ShapeType::Rectangle,
                        1 => ShapeType::Circle,
                        2 => ShapeType::Line {
                            start: [self.drawing_line_start_x, self.drawing_line_start_y],
                            end: [self.drawing_line_end_x, self.drawing_line_end_y],
                            start_style: custom_start,
                            end_style: custom_end,
                        },
                        3 => ShapeType::Line {
                            start: [self.drawing_line_start_x, self.drawing_line_start_y],
                            end: [self.drawing_line_end_x, self.drawing_line_end_y],
                            start_style: LineEndingStyle::None,
                            end_style: LineEndingStyle::OpenArrow,
                        },
                        _ => ShapeType::Line {
                            start: [self.drawing_line_start_x, self.drawing_line_start_y],
                            end: [self.drawing_line_end_x, self.drawing_line_end_y],
                            start_style: LineEndingStyle::ClosedArrow,
                            end_style: LineEndingStyle::ClosedArrow,
                        },
                    }
                };

                let config = ShapeAnnotationConfig {
                    shape_type,
                    rect: [
                        self.drawing_pos_x,
                        self.drawing_pos_y,
                        self.drawing_pos_x + self.drawing_width,
                        self.drawing_pos_y + self.drawing_height,
                    ],
                    stroke_color: Some((
                        self.drawing_stroke_color[0],
                        self.drawing_stroke_color[1],
                        self.drawing_stroke_color[2],
                    )),
                    fill_color: if self.drawing_has_fill {
                        Some((
                            self.drawing_fill_color[0],
                            self.drawing_fill_color[1],
                            self.drawing_fill_color[2],
                        ))
                    } else {
                        None
                    },
                    stroke_width: self.drawing_stroke_width,
                    border_style: dash,
                    border_effect: be,
                    opacity: self.drawing_opacity,
                    author: Some("User".to_string()),
                    locked: self.drawing_locked,
                    popup_note: if self.drawing_note_input.trim().is_empty() {
                        None
                    } else {
                        Some(self.drawing_note_input.trim().to_string())
                    },
                };

                if let Some(tab) = self.active_tab_mut() {
                    let curr = tab.current_page;
                    match add_shape_annotation(&mut tab.doc, curr, &config) {
                        Ok(_) => {
                            tab.is_modified = true;
                            tab.invalidate_render();
                            self.status_message = format!("Inserted shape on page {}", curr + 1);
                        }
                        Err(e) => {
                            self.status_message = format!("Failed to insert shape: {}", e);
                        }
                    }
                }
            }

            if apply_ink {
                let stroke_groups: Vec<Vec<[f64; 2]>> = self
                    .drawing_ink_strokes_input
                    .split('|')
                    .map(|grp| {
                        grp.split(';')
                            .filter_map(|pair| {
                                let p: Vec<&str> = pair.split(',').map(|s| s.trim()).collect();
                                if p.len() == 2 {
                                    if let (Ok(x), Ok(y)) =
                                        (p[0].parse::<f64>(), p[1].parse::<f64>())
                                    {
                                        return Some([x, y]);
                                    }
                                }
                                None
                            })
                            .collect()
                    })
                    .filter(|s: &Vec<[f64; 2]>| !s.is_empty())
                    .collect();

                let config = InkAnnotationConfig {
                    strokes: if stroke_groups.is_empty() {
                        vec![vec![[100.0, 100.0], [150.0, 140.0], [200.0, 120.0]]]
                    } else {
                        stroke_groups
                    },
                    stroke_width: self.drawing_stroke_width,
                    color: (
                        self.drawing_stroke_color[0],
                        self.drawing_stroke_color[1],
                        self.drawing_stroke_color[2],
                    ),
                    opacity: self.drawing_opacity,
                    smooth: self.drawing_ink_smooth,
                    author: Some("User".to_string()),
                    locked: self.drawing_locked,
                    popup_note: if self.drawing_note_input.trim().is_empty() {
                        None
                    } else {
                        Some(self.drawing_note_input.trim().to_string())
                    },
                };

                if let Some(tab) = self.active_tab_mut() {
                    let curr = tab.current_page;
                    match add_ink_annotation(&mut tab.doc, curr, &config) {
                        Ok(_) => {
                            tab.is_modified = true;
                            tab.invalidate_render();
                            self.status_message = format!("Added ink drawing on page {}", curr + 1);
                        }
                        Err(e) => {
                            self.status_message = format!("Failed to add ink drawing: {}", e);
                        }
                    }
                }
            }

            if apply_erase {
                let erase_pt = [self.drawing_eraser_x, self.drawing_eraser_y];
                let radius = self.drawing_eraser_radius;
                if let Some(tab) = self.active_tab_mut() {
                    let curr = tab.current_page;
                    let mut erased_any = false;

                    if let Ok(annots) = get_page_annotations(&tab.doc, curr) {
                        for a in annots {
                            if matches!(a.annot_type, AnnotationType::Ink) {
                                if let Some(id) = a.id {
                                    if let Ok(true) =
                                        erase_ink_strokes(&mut tab.doc, curr, id, erase_pt, radius)
                                    {
                                        erased_any = true;
                                    }
                                }
                            }
                        }
                    }

                    if erased_any {
                        tab.is_modified = true;
                        tab.invalidate_render();
                        self.status_message = format!(
                            "Erased ink strokes at ({:.1}, {:.1}) on page {}",
                            erase_pt[0],
                            erase_pt[1],
                            curr + 1
                        );
                    } else {
                        self.status_message = format!(
                            "No ink strokes found at ({:.1}, {:.1})",
                            erase_pt[0], erase_pt[1]
                        );
                    }
                }
            }

            if close {
                self.show_drawing_dialog = false;
            }
        }

        // Callout Text Annotation Dialog (FR-08.1.3)
        if self.show_callout_dialog {
            let mut close = false;
            let mut apply_callout = false;

            egui::Window::new("🗨️ Callout Annotation")
                .collapsible(false)
                .resizable(true)
                .default_width(420.0)
                .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.label(
                        RichText::new(
                            "Configure text callout with leader arrow pointing to page feature:",
                        )
                        .small(),
                    );
                    ui.add_space(4.0);

                    ui.horizontal(|ui| {
                        ui.label("Callout Text:");
                    });
                    ui.text_edit_multiline(&mut self.callout_text_input);
                    ui.separator();

                    ui.horizontal(|ui| {
                        ui.label("Box Position X:");
                        ui.add(egui::DragValue::new(&mut self.callout_box_x));
                        ui.label("Y:");
                        ui.add(egui::DragValue::new(&mut self.callout_box_y));
                    });
                    ui.horizontal(|ui| {
                        ui.label("Width:");
                        ui.add(egui::DragValue::new(&mut self.callout_box_w).range(40.0..=600.0));
                        ui.label("Height:");
                        ui.add(egui::DragValue::new(&mut self.callout_box_h).range(20.0..=300.0));
                    });

                    ui.horizontal(|ui| {
                        ui.label("Arrow Target X:");
                        ui.add(egui::DragValue::new(&mut self.callout_leader_x));
                        ui.label("Y:");
                        ui.add(egui::DragValue::new(&mut self.callout_leader_y));
                    });

                    ui.horizontal(|ui| {
                        ui.label("Elbow / Knee X:");
                        ui.add(egui::DragValue::new(&mut self.callout_knee_x));
                        ui.label("Y:");
                        ui.add(egui::DragValue::new(&mut self.callout_knee_y));
                    });

                    ui.horizontal(|ui| {
                        ui.label("Border & Line Color:");
                        ui.color_edit_button_rgb(&mut self.callout_border_color);
                    });

                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui.button("🗨️ Insert Callout").clicked() {
                            apply_callout = true;
                            close = true;
                        }
                        if ui.button("Cancel").clicked() {
                            close = true;
                        }
                    });
                });

            if apply_callout {
                let config = CalloutConfig {
                    text: self.callout_text_input.clone(),
                    text_rect: [
                        self.callout_box_x,
                        self.callout_box_y,
                        self.callout_box_w,
                        self.callout_box_h,
                    ],
                    leader_start: (self.callout_leader_x, self.callout_leader_y),
                    leader_knee: (self.callout_knee_x, self.callout_knee_y),
                    border_color: (
                        self.callout_border_color[0],
                        self.callout_border_color[1],
                        self.callout_border_color[2],
                    ),
                    fill_color: None,
                    text_color: (0.0, 0.0, 0.0),
                    font_size: 10.0,
                    author: Some("User".into()),
                };

                if let Some(tab) = self.active_tab_mut() {
                    let curr = tab.current_page;
                    match add_callout_annotation(&mut tab.doc, curr, &config) {
                        Ok(_) => {
                            tab.is_modified = true;
                            tab.invalidate_render();
                            self.status_message =
                                format!("Added callout annotation on page {}", curr + 1);
                        }
                        Err(e) => {
                            self.status_message = format!("Failed to add callout: {}", e);
                        }
                    }
                }
            }

            if close {
                self.show_callout_dialog = false;
            }
        }

        // Typewriter Annotation Dialog (FR-08.1.4)
        if self.show_typewriter_dialog {
            let mut close = false;
            let mut apply_typewriter = false;

            egui::Window::new("⌨️ Typewriter Annotation")
                .collapsible(false)
                .resizable(true)
                .default_width(380.0)
                .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.label(
                        RichText::new("Place text directly onto page at coordinates:").small(),
                    );
                    ui.add_space(4.0);

                    ui.horizontal(|ui| {
                        ui.label("Text:");
                        ui.text_edit_singleline(&mut self.typewriter_text_input);
                    });

                    ui.horizontal(|ui| {
                        ui.label("Position X:");
                        ui.add(egui::DragValue::new(&mut self.typewriter_pos_x));
                        ui.label("Y:");
                        ui.add(egui::DragValue::new(&mut self.typewriter_pos_y));
                    });

                    ui.horizontal(|ui| {
                        ui.label("Font Size:");
                        ui.add(
                            egui::DragValue::new(&mut self.typewriter_font_size)
                                .range(6.0..=72.0)
                                .suffix(" pt"),
                        );
                        ui.label("Color:");
                        ui.color_edit_button_rgb(&mut self.typewriter_color);
                    });

                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui.button("⌨️ Add Typewritten Text").clicked() {
                            apply_typewriter = true;
                            close = true;
                        }
                        if ui.button("Cancel").clicked() {
                            close = true;
                        }
                    });
                });

            if apply_typewriter {
                let config = TypewriterConfig {
                    text: self.typewriter_text_input.clone(),
                    pos: (self.typewriter_pos_x, self.typewriter_pos_y),
                    font_size: self.typewriter_font_size,
                    color: (
                        self.typewriter_color[0],
                        self.typewriter_color[1],
                        self.typewriter_color[2],
                    ),
                    author: Some("User".into()),
                };

                if let Some(tab) = self.active_tab_mut() {
                    let curr = tab.current_page;
                    match add_typewriter_annotation(&mut tab.doc, curr, &config) {
                        Ok(_) => {
                            tab.is_modified = true;
                            tab.invalidate_render();
                            self.status_message =
                                format!("Added typewriter text on page {}", curr + 1);
                        }
                        Err(e) => {
                            self.status_message = format!("Failed to add typewriter text: {}", e);
                        }
                    }
                }
            }

            if close {
                self.show_typewriter_dialog = false;
            }
        }

        // 46. Annotation Inspector Dialog (FR-08.3)
        if self.show_annot_inspector {
            if let Some(ref sel) = self.selected_annotation {
                let mut close = false;
                let mut delete_requested = false;
                let mut apply_contents = false;
                let mut apply_color = false;
                let mut toggle_lock = false;
                let annot_id = sel.id;
                let page_idx = sel.page_index;
                let subtype_str = sel.annot_type.subtype_name().to_string();
                let rect_coords = sel.rect;

                egui::Window::new("🏷 Annotation Properties")
                    .collapsible(true)
                    .resizable(true)
                    .default_size([300.0, 320.0])
                    .show(ctx, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(format!("Type: {}", subtype_str)).strong());
                            ui.label(format!("(Page {})", page_idx + 1));
                        });
                        ui.label(format!(
                            "Rect: [{:.1}, {:.1}, {:.1}, {:.1}]",
                            rect_coords[0], rect_coords[1], rect_coords[2], rect_coords[3]
                        ));
                        ui.separator();

                        ui.label("Author / User (T):");
                        ui.text_edit_singleline(&mut self.annot_inspector_author);

                        ui.label("Contents / Note Text:");
                        ui.text_edit_multiline(&mut self.annot_inspector_contents);

                        if ui.button("💾 Apply Text & Author").clicked() {
                            apply_contents = true;
                        }

                        ui.separator();
                        ui.label("Color & Opacity:");
                        ui.horizontal(|ui| {
                            ui.color_edit_button_rgb(&mut self.annot_inspector_color);
                            ui.label("Color");
                        });
                        ui.horizontal(|ui| {
                            ui.add(
                                egui::Slider::new(&mut self.annot_inspector_opacity, 0.05..=1.0)
                                    .text("Opacity"),
                            );
                        });

                        if ui.button("🎨 Apply Color & Opacity").clicked() {
                            apply_color = true;
                        }

                        ui.separator();
                        ui.horizontal(|ui| {
                            if ui.button("🔒 Toggle Lock").clicked() {
                                toggle_lock = true;
                            }
                            if ui
                                .button(
                                    RichText::new("🗑 Delete Annotation")
                                        .color(Color32::from_rgb(239, 68, 68)),
                                )
                                .clicked()
                            {
                                delete_requested = true;
                            }
                            if ui.button("Close").clicked() {
                                close = true;
                            }
                        });
                    });

                if toggle_lock {
                    if let Some(tab) = self.active_tab_mut() {
                        let _ = set_annotation_locked(&mut tab.doc, annot_id, true);
                        tab.is_modified = true;
                        self.status_message = "Toggled annotation lock flag".into();
                    }
                } else if delete_requested {
                    if let Some(tab) = self.active_tab_mut() {
                        let _ = delete_annotation(&mut tab.doc, page_idx, annot_id);
                        tab.is_modified = true;
                        tab.invalidate_render();
                        self.status_message = "Deleted annotation".into();
                    }
                    self.selected_annotation = None;
                    self.show_annot_inspector = false;
                } else if apply_contents {
                    let contents = self.annot_inspector_contents.clone();
                    let author = self.annot_inspector_author.clone();
                    if let Some(tab) = self.active_tab_mut() {
                        let _ = update_annotation_contents(
                            &mut tab.doc,
                            annot_id,
                            &contents,
                            Some(&author),
                        );
                        tab.is_modified = true;
                        tab.invalidate_render();
                        self.status_message = "Updated annotation contents".into();
                    }
                    if let Some(ref mut s) = self.selected_annotation {
                        s.contents = Some(contents);
                        s.author = Some(author);
                    }
                } else if apply_color {
                    let col = (
                        self.annot_inspector_color[0],
                        self.annot_inspector_color[1],
                        self.annot_inspector_color[2],
                    );
                    let op = self.annot_inspector_opacity;
                    if let Some(tab) = self.active_tab_mut() {
                        let _ = update_annotation_color(&mut tab.doc, annot_id, col, Some(op));
                        tab.is_modified = true;
                        tab.invalidate_render();
                        self.status_message = "Updated annotation appearance".into();
                    }
                    if let Some(ref mut s) = self.selected_annotation {
                        s.color = Some(col);
                        s.opacity = op;
                    }
                } else if close {
                    self.show_annot_inspector = false;
                }
            } else {
                self.show_annot_inspector = false;
            }
        }

        // 47. Form Field Properties Inspector Dialog (FR-11.2, FR-11.3)
        if self.show_form_field_inspector {
            if let Some(ref sel) = self.selected_form_field {
                let mut close = false;
                let mut delete_field = false;
                let mut apply_props = false;
                let field_id = sel.id;
                let page_idx = sel.page_index;
                let field_type_name = sel.field_type.ft_name().to_string();
                let rect_coords = sel.rect;

                egui::Window::new("📝 Form Field Properties")
                    .collapsible(true)
                    .resizable(true)
                    .default_size([340.0, 420.0])
                    .show(ctx, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(format!("Type: {}", field_type_name)).strong());
                            ui.label(format!(
                                "(Page {}, ID: {}:{})",
                                page_idx + 1,
                                field_id.number,
                                field_id.generation
                            ));
                        });
                        ui.label(format!(
                            "Rect: [{:.1}, {:.1}, {:.1}, {:.1}] ({} × {})",
                            rect_coords[0],
                            rect_coords[1],
                            rect_coords[2],
                            rect_coords[3],
                            (rect_coords[2] - rect_coords[0]).abs() as i32,
                            (rect_coords[3] - rect_coords[1]).abs() as i32,
                        ));
                        ui.separator();

                        ui.label(RichText::new("General Properties").strong());
                        ui.horizontal(|ui| {
                            ui.label("Field Name (/T):");
                            ui.text_edit_singleline(&mut self.form_field_inspector_name);
                        });
                        ui.horizontal(|ui| {
                            ui.label("Tooltip (/TU):");
                            ui.text_edit_singleline(&mut self.form_field_inspector_tooltip);
                        });
                        ui.horizontal(|ui| {
                            ui.label("Default Value (/DV):");
                            ui.text_edit_singleline(&mut self.form_field_inspector_default_val);
                        });

                        ui.separator();
                        ui.label(RichText::new("Appearance").strong());
                        ui.horizontal(|ui| {
                            ui.label("Font Size:");
                            ui.add(
                                egui::Slider::new(
                                    &mut self.form_field_inspector_font_size,
                                    6.0..=72.0,
                                )
                                .suffix(" pt"),
                            );
                        });
                        ui.horizontal(|ui| {
                            ui.color_edit_button_rgb(&mut self.form_field_inspector_color);
                            ui.label("Text Color (/DA)");
                        });

                        ui.separator();
                        ui.label(RichText::new("Field Flags").strong());
                        ui.horizontal(|ui| {
                            ui.checkbox(&mut self.form_field_inspector_readonly, "Read-Only");
                            ui.checkbox(&mut self.form_field_inspector_required, "Required");
                        });

                        if sel.field_type == FormFieldType::Signature {
                            ui.separator();
                            if ui
                                .button(
                                    RichText::new("🔏 Digitally Sign This Document Now...")
                                        .color(Color32::from_rgb(59, 130, 246))
                                        .strong(),
                                )
                                .clicked()
                            {
                                self.sig_field_name = self.form_field_inspector_name.clone();
                                self.sig_target_page = page_idx;
                                self.sig_mode_idx = 0;
                                self.show_add_signature_dialog = true;
                                close = true;
                            }
                        }

                        ui.separator();
                        ui.horizontal(|ui| {
                            if ui.button("💾 Apply Properties").clicked() {
                                apply_props = true;
                            }
                            if ui
                                .button(
                                    RichText::new("🗑 Delete Field")
                                        .color(Color32::from_rgb(239, 68, 68)),
                                )
                                .clicked()
                            {
                                delete_field = true;
                            }
                            if ui.button("Close").clicked() {
                                close = true;
                            }
                        });
                    });

                if delete_field {
                    if let Some(tab) = self.active_tab_mut() {
                        let _ = delete_form_field(&mut tab.doc, field_id);
                        tab.is_modified = true;
                        tab.invalidate_render();
                        self.status_message = format!(
                            "Deleted form field (ID: {}:{})",
                            field_id.number, field_id.generation
                        );
                    }
                    self.selected_form_field = None;
                    self.show_form_field_inspector = false;
                } else if apply_props {
                    let name = self.form_field_inspector_name.clone();
                    let tooltip = self.form_field_inspector_tooltip.clone();
                    let def_val = self.form_field_inspector_default_val.clone();
                    let ro = self.form_field_inspector_readonly;
                    let req = self.form_field_inspector_required;
                    let font_size = self.form_field_inspector_font_size;
                    let color = self.form_field_inspector_color;

                    if let Some(tab) = self.active_tab_mut() {
                        let res = update_form_field_properties(
                            &mut tab.doc,
                            field_id,
                            Some(&name),
                            Some(&tooltip),
                            Some(&def_val),
                            Some(ro),
                            Some(req),
                            Some(color),
                            Some(font_size),
                        );
                        if res.is_ok() {
                            tab.is_modified = true;
                            tab.invalidate_render();
                            self.status_message =
                                format!("Updated properties for form field '{}'", name);
                        }
                    }
                    if let Some(ref mut s) = self.selected_form_field {
                        s.name = name;
                        s.tooltip = Some(tooltip);
                        s.default_value = Some(def_val);
                        s.read_only = ro;
                        s.required = req;
                        s.font_size = Some(font_size);
                        s.text_color = Some(color);
                    }
                } else if close {
                    self.show_form_field_inspector = false;
                }
            } else {
                self.show_form_field_inspector = false;
            }
        }

        // 48. Apply Redactions Confirmation Warning Dialog (FR-15.3, FR-15.6)
        if self.show_apply_redactions_confirm_dialog {
            let mut close = false;
            let mut apply_confirmed = false;
            let mut clear_all = false;
            let count = self.marked_redactions.len();

            egui::Window::new("⚠️ Apply Redactions - Irreversible Action")
                .collapsible(false)
                .resizable(false)
                .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
                .default_width(460.0)
                .show(ctx, |ui| {
                    ui.label(RichText::new("⚠️ PERMANENT & IRREVERSIBLE ACTION")
                        .strong()
                        .color(Color32::from_rgb(239, 68, 68)));
                    ui.label(
                        "Applying redactions permanently removes all marked content, underlying text streams, \
                         image pixels, and vector paths from the document. This data cannot be recovered \
                         once the document is saved."
                    );
                    ui.separator();

                    ui.label(RichText::new(format!("Marked Regions to Burn: {} region(s)", count)).strong());
                    if count == 0 {
                        ui.label(RichText::new("No redaction marks are currently staged on any page.").italics());
                    } else {
                        egui::ScrollArea::vertical().max_height(160.0).show(ui, |ui| {
                            let mut to_remove = None;
                            for (idx, r) in self.marked_redactions.iter().enumerate() {
                                ui.horizontal(|ui| {
                                    ui.label(format!("#{}: Page {} [{:.0}, {:.0}, {:.0}, {:.0}]",
                                        r.id, r.page_index + 1, r.rect[0], r.rect[1], r.rect[2], r.rect[3]));
                                    if let Some(ref text) = r.appearance.overlay_text {
                                        ui.label(format!("\"{}\"", text));
                                    }
                                    if ui.small_button("✕").clicked() {
                                        to_remove = Some(idx);
                                    }
                                });
                            }
                            if let Some(idx) = to_remove {
                                self.marked_redactions.remove(idx);
                            }
                        });
                    }

                    ui.separator();
                    ui.horizontal(|ui| {
                        if count > 0 {
                            if ui.button(RichText::new("🔥 Permanently Burn All Redactions")
                                .strong()
                                .color(Color32::from_rgb(239, 68, 68))).clicked()
                            {
                                apply_confirmed = true;
                                close = true;
                            }
                            if ui.button("Clear Marks").clicked() {
                                clear_all = true;
                            }
                        }
                        if ui.button("Cancel / Keep Marking").clicked() {
                            close = true;
                        }
                    });
                });

            if apply_confirmed && !self.marked_redactions.is_empty() {
                let items: Vec<(usize, [f64; 4], RedactionAppearance)> = self
                    .marked_redactions
                    .iter()
                    .map(|r| (r.page_index, r.rect, r.appearance.clone()))
                    .collect();
                if let Some(tab) = self.active_tab_mut() {
                    match apply_batch_redactions(&mut tab.doc, &items) {
                        Ok(num) => {
                            tab.is_modified = true;
                            tab.invalidate_render();
                            self.status_message =
                                format!("Permanently burned {} redactions", num);
                            self.marked_redactions.clear();
                        }
                        Err(e) => {
                            self.status_message = format!("Failed to apply redactions: {}", e);
                        }
                    }
                }
            } else if clear_all {
                self.marked_redactions.clear();
                self.status_message = "Cleared all marked redactions".into();
            }

            if close {
                self.show_apply_redactions_confirm_dialog = false;
            }
        }

        // 49. Sanitize Document Dialog (FR-15.5)
        if self.show_sanitize_dialog {
            let mut close = false;
            let mut execute_sanitize = false;

            egui::Window::new("🛡 Sanitize Document")
                .collapsible(false)
                .resizable(false)
                .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
                .default_width(480.0)
                .show(ctx, |ui| {
                    ui.label(RichText::new("Document Sanitization & Metadata Scrubbing").strong());
                    ui.label(
                        "Automatically purges sensitive metadata, hidden comments, file attachments, \
                         JavaScript actions, and bookmarks to prepare the document for safe external distribution."
                    );
                    ui.separator();

                    ui.label(RichText::new("Sanitization Targets:").strong());
                    ui.checkbox(
                        &mut self.sanitize_options.remove_metadata,
                        "Remove Document Metadata & XMP (Title, Author, Subject, Creator, Dates)",
                    );
                    ui.checkbox(
                        &mut self.sanitize_options.remove_annotations,
                        "Remove All Annotations, Comments & Sticky Notes (/Annots)",
                    );
                    ui.checkbox(
                        &mut self.sanitize_options.remove_attachments,
                        "Remove Embedded File Attachments (/Names /EmbeddedFiles, /AF)",
                    );
                    ui.checkbox(
                        &mut self.sanitize_options.remove_javascript,
                        "Remove Embedded JavaScript Actions (/Names /JavaScript, /OpenAction, /AA)",
                    );
                    ui.checkbox(
                        &mut self.sanitize_options.remove_bookmarks,
                        "Remove Document Outlines & Bookmarks (/Outlines)",
                    );
                    ui.checkbox(
                        &mut self.sanitize_options.flatten_forms,
                        "Flatten Interactive AcroForms into Static Vector Graphics",
                    );

                    if let Some(ref report) = self.sanitize_report {
                        ui.separator();
                        ui.label(RichText::new("Sanitization Summary:").strong().color(Color32::from_rgb(34, 197, 94)));
                        ui.label(format!("• Metadata fields removed: {}", report.metadata_fields_removed));
                        ui.label(format!("• XMP metadata stream removed: {}", if report.xmp_metadata_removed { "Yes" } else { "No" }));
                        ui.label(format!("• Annotations/comments removed: {}", report.annotations_removed));
                        ui.label(format!("• Embedded attachments removed: {}", report.attachments_removed));
                        ui.label(format!("• JavaScript actions removed: {}", report.javascript_scripts_removed));
                        ui.label(format!("• Bookmarks removed: {}", if report.bookmarks_removed { "Yes" } else { "No" }));
                        ui.label(format!("• Forms flattened: {}", if report.forms_flattened { "Yes" } else { "No" }));
                    }

                    ui.separator();
                    ui.horizontal(|ui| {
                        if ui.button(RichText::new("🛡 Sanitize Document").strong().color(Color32::from_rgb(37, 99, 235))).clicked() {
                            execute_sanitize = true;
                        }
                        if ui.button("Close").clicked() {
                            close = true;
                        }
                    });
                });

            if execute_sanitize {
                let options = self.sanitize_options.clone();
                if let Some(tab) = self.active_tab_mut() {
                    match sanitize_document(&mut tab.doc, &options) {
                        Ok(rep) => {
                            tab.is_modified = true;
                            tab.invalidate_render();
                            self.status_message =
                                "Document sanitized successfully".into();
                            self.sanitize_report = Some(rep);
                        }
                        Err(e) => {
                            self.status_message = format!("Sanitization failed: {}", e);
                        }
                    }
                }
            }

            if close {
                self.show_sanitize_dialog = false;
                self.sanitize_report = None;
            }
        }

        // Duplicate Form Field Modal (FR-11.3.5)
        if self.show_duplicate_field_dialog {
            let mut close = false;
            let mut execute_duplicate = false;
            let sel_opt = self.selected_form_field.clone();

            egui::Window::new("Duplicate Field Across Pages")
                .collapsible(false)
                .resizable(false)
                .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    if let Some(ref sel) = sel_opt {
                        ui.label(
                            RichText::new(format!(
                                "Duplicating field: '{}' ({:?})",
                                sel.name, sel.field_type
                            ))
                            .strong(),
                        );
                        ui.add_space(6.0);
                        ui.label("Target Pages (e.g. '1, 2, 4-6' or 'all'):");
                        ui.text_edit_singleline(&mut self.duplicate_field_target_pages_input);
                        ui.add_space(4.0);
                        if let Some(tab) = self.active_tab() {
                            let total = tab.doc.page_count();
                            ui.label(
                                RichText::new(format!("Document has {} total pages.", total))
                                    .weak()
                                    .size(11.0),
                            );
                            if ui.button("Select All Other Pages").clicked() {
                                let mut pages: Vec<String> = Vec::new();
                                for p in 0..total {
                                    if p != sel.page_index {
                                        pages.push((p + 1).to_string());
                                    }
                                }
                                self.duplicate_field_target_pages_input = pages.join(", ");
                            }
                        }

                        ui.add_space(8.0);
                        ui.horizontal(|ui| {
                            if ui
                                .button(
                                    RichText::new("Duplicate Field")
                                        .strong()
                                        .color(Color32::from_rgb(37, 99, 235)),
                                )
                                .clicked()
                            {
                                execute_duplicate = true;
                            }
                            if ui.button("Cancel").clicked() {
                                close = true;
                            }
                        });
                    } else {
                        ui.label("No form field selected. Please select a form field first.");
                        if ui.button("Close").clicked() {
                            close = true;
                        }
                    }
                });

            if execute_duplicate {
                if let Some(ref sel) = sel_opt {
                    let total_pages = self.active_tab().map(|t| t.doc.page_count()).unwrap_or(0);
                    let input = self.duplicate_field_target_pages_input.trim();
                    let mut target_pages: Vec<usize> = Vec::new();
                    if input.eq_ignore_ascii_case("all") {
                        for p in 0..total_pages {
                            if p != sel.page_index {
                                target_pages.push(p);
                            }
                        }
                    } else {
                        for part in input.split(',') {
                            let part = part.trim();
                            if let Some((start_str, end_str)) = part.split_once('-') {
                                if let (Ok(s), Ok(e)) = (
                                    start_str.trim().parse::<usize>(),
                                    end_str.trim().parse::<usize>(),
                                ) {
                                    for p in s..=e {
                                        if p >= 1 && p <= total_pages {
                                            let idx = p - 1;
                                            if !target_pages.contains(&idx) {
                                                target_pages.push(idx);
                                            }
                                        }
                                    }
                                }
                            } else if let Ok(p) = part.parse::<usize>() {
                                if p >= 1 && p <= total_pages {
                                    let idx = p - 1;
                                    if !target_pages.contains(&idx) {
                                        target_pages.push(idx);
                                    }
                                }
                            }
                        }
                    }

                    if !target_pages.is_empty() {
                        let mut field = match sel.field_type {
                            FormFieldType::Text => FormField::new_text(&sel.name, sel.rect),
                            FormFieldType::Checkbox => FormField::new_checkbox(
                                &sel.name,
                                sel.rect,
                                sel.value.as_deref() == Some("Yes"),
                            ),
                            FormFieldType::Radio => {
                                FormField::new_radio_button(&sel.name, "Yes", sel.rect, false)
                            }
                            FormFieldType::Choice => {
                                FormField::new_choice(&sel.name, sel.rect, vec![], None)
                            }
                            FormFieldType::ListBox => {
                                FormField::new_listbox(&sel.name, sel.rect, vec![], None, false)
                            }
                            FormFieldType::PushButton => {
                                FormField::new_pushbutton(&sel.name, sel.rect, &sel.name)
                            }
                            FormFieldType::Barcode => FormField::new_barcode(
                                &sel.name,
                                sel.rect,
                                BarcodeSymbology::QrCode,
                                Some("DATA".into()),
                            ),
                            FormFieldType::Signature => {
                                FormField::new_signature(&sel.name, sel.rect)
                            }
                        };
                        field.value = sel.value.clone();
                        field.default_value = sel.default_value.clone();
                        field.read_only = sel.read_only;
                        field.required = sel.required;
                        field.tooltip = sel.tooltip.clone();
                        field.font_size = sel.font_size;
                        field.text_color = sel.text_color;

                        if let Some(tab) = self.active_tab_mut() {
                            match ravpdf_forms::duplicate_form_field(
                                &mut tab.doc,
                                &field,
                                &target_pages,
                            ) {
                                Ok(ids) => {
                                    tab.is_modified = true;
                                    tab.invalidate_render();
                                    self.status_message = format!(
                                        "Duplicated field '{}' to {} pages",
                                        sel.name,
                                        ids.len()
                                    );
                                    close = true;
                                }
                                Err(e) => {
                                    self.status_message =
                                        format!("Failed to duplicate field: {}", e);
                                }
                            }
                        }
                    } else {
                        self.status_message = "No valid target pages specified".into();
                    }
                }
            }

            if close {
                self.show_duplicate_field_dialog = false;
            }
        }

        // XFDF Annotations Exchange Modal (ISO 19444-1 / FR-08.3.6)
        if self.show_xfdf_dialog {
            let mut close = false;
            let mut execute_import = false;

            egui::Window::new("XFDF Annotations Exchange")
                .collapsible(false)
                .resizable(true)
                .default_size([550.0, 400.0])
                .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.label(
                        RichText::new(
                            "Export and import annotations using XML Forms Data Format (XFDF)",
                        )
                        .weak()
                        .size(11.0),
                    );
                    ui.add_space(4.0);

                    ui.horizontal(|ui| {
                        if ui.button("📤 Export from Current Document").clicked() {
                            if let Some(tab) = self.active_tab() {
                                match ravpdf_annot::export_annotations_to_xfdf(&tab.doc) {
                                    Ok(xml) => {
                                        self.xfdf_buffer = xml;
                                        self.status_message = "Exported annotations to XFDF".into();
                                    }
                                    Err(e) => {
                                        self.status_message = format!("Export failed: {}", e);
                                    }
                                }
                            }
                        }
                        if ui.button("📋 Copy to Clipboard").clicked() {
                            ctx.output_mut(|o| o.copied_text = self.xfdf_buffer.clone());
                            self.status_message = "Copied XFDF to clipboard".into();
                        }
                    });

                    ui.add_space(4.0);
                    ui.label("XFDF XML Content:");
                    ScrollArea::vertical().max_height(250.0).show(ui, |ui| {
                        ui.add(
                            egui::TextEdit::multiline(&mut self.xfdf_buffer)
                                .font(egui::TextStyle::Monospace)
                                .desired_width(f32::INFINITY)
                                .desired_rows(12),
                        );
                    });

                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui
                            .button(
                                RichText::new("📥 Import Annotations")
                                    .strong()
                                    .color(Color32::from_rgb(37, 99, 235)),
                            )
                            .clicked()
                        {
                            execute_import = true;
                        }
                        if ui.button("Close").clicked() {
                            close = true;
                        }
                    });
                });

            if execute_import {
                let xml = self.xfdf_buffer.trim().to_string();
                if !xml.is_empty() {
                    if let Some(tab) = self.active_tab_mut() {
                        match ravpdf_annot::import_annotations_from_xfdf(&mut tab.doc, &xml) {
                            Ok(count) => {
                                tab.is_modified = true;
                                tab.invalidate_render();
                                self.status_message =
                                    format!("Imported {} annotations from XFDF", count);
                                close = true;
                            }
                            Err(e) => {
                                self.status_message = format!("Import failed: {}", e);
                            }
                        }
                    }
                }
            }

            if close {
                self.show_xfdf_dialog = false;
            }
        }
    }
}

impl eframe::App for RavPdfApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Enforce window title header strictly as "RAV PDF"
        ctx.send_viewport_cmd(egui::ViewportCommand::Title("RAV PDF".to_string()));

        // 1. Handle Drag-and-Drop files onto the window (EI-01.7)
        let dropped_files = ctx.input(|i| i.raw.dropped_files.clone());
        for dropped in dropped_files {
            if let Some(path) = dropped.path {
                if path
                    .extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("pdf"))
                {
                    self.open_file(path);
                    break;
                }
            }
        }

        // 2. Handle Global Keyboard Shortcuts (FR-01.2.1, FR-34.1)
        self.handle_keyboard_shortcuts(ctx);

        // 3. Render UI Components (EI-01.1 - EI-01.6)
        if self.is_full_screen && self.active_tab().is_some() {
            // Full Screen Presentation Mode (FR-01.3.6)
            self.render_canvas(ctx);
        } else {
            self.render_menu_bar(ctx);
            self.render_toolbar(ctx);
            self.render_tab_bar(ctx);

            if self.search_bar_open && self.active_tab().is_some() {
                self.render_search_bar(ctx);
            }

            self.render_status_bar(ctx);
            self.render_sidebar(ctx);
            self.render_canvas(ctx);
        }

        // 4. Render Modals (Watermark, Bates, Headers & Footers, Text Box)
        self.render_dialogs(ctx);
    }
}

fn setup_custom_theme(ctx: &egui::Context, dark: bool) {
    let mut visuals = if dark {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    };
    visuals.dark_mode = dark;

    if dark {
        visuals.panel_fill = Color32::from_rgb(24, 24, 27); // Zinc 900
        visuals.window_fill = Color32::from_rgb(32, 32, 36);
        visuals.faint_bg_color = Color32::from_rgb(39, 39, 42); // Zinc 800
        visuals.extreme_bg_color = Color32::from_rgb(18, 18, 20); // Canvas & input background
        visuals.code_bg_color = Color32::from_rgb(28, 28, 32);

        visuals.window_stroke = Stroke::new(1.0_f32, Color32::from_rgb(63, 63, 70));
        visuals.window_rounding = egui::Rounding::same(8.0);
        visuals.menu_rounding = egui::Rounding::same(6.0);

        let rounding = egui::Rounding::same(5.0);

        visuals.widgets.noninteractive.bg_fill = Color32::from_rgb(30, 30, 34);
        visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, Color32::from_rgb(48, 48, 56));
        visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, Color32::from_rgb(228, 228, 231));
        visuals.widgets.noninteractive.rounding = rounding;

        visuals.widgets.inactive.bg_fill = Color32::from_rgb(39, 39, 44);
        visuals.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, Color32::from_rgb(58, 58, 66));
        visuals.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, Color32::from_rgb(220, 220, 225));
        visuals.widgets.inactive.rounding = rounding;

        visuals.widgets.hovered.bg_fill = Color32::from_rgb(52, 52, 60);
        visuals.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, Color32::from_rgb(59, 130, 246));
        visuals.widgets.hovered.fg_stroke = Stroke::new(1.0_f32, Color32::WHITE);
        visuals.widgets.hovered.rounding = rounding;

        visuals.widgets.active.bg_fill = Color32::from_rgb(37, 99, 235);
        visuals.widgets.active.bg_stroke = Stroke::new(1.0_f32, Color32::from_rgb(96, 165, 250));
        visuals.widgets.active.fg_stroke = Stroke::new(1.0_f32, Color32::WHITE);
        visuals.widgets.active.rounding = rounding;

        visuals.widgets.open.bg_fill = Color32::from_rgb(44, 44, 50);
        visuals.widgets.open.bg_stroke = Stroke::new(1.0_f32, Color32::from_rgb(59, 130, 246));
        visuals.widgets.open.fg_stroke = Stroke::new(1.0_f32, Color32::WHITE);
        visuals.widgets.open.rounding = rounding;

        visuals.selection.bg_fill = Color32::from_rgb(37, 99, 235);
        visuals.selection.stroke = Stroke::new(1.0_f32, Color32::from_rgb(96, 165, 250));
    } else {
        visuals.panel_fill = Color32::from_rgb(241, 245, 249); // Slate 100
        visuals.window_fill = Color32::from_rgb(255, 255, 255);
        visuals.faint_bg_color = Color32::from_rgb(248, 250, 252); // Slate 50
        visuals.extreme_bg_color = Color32::from_rgb(226, 232, 240); // Slate 200
        visuals.code_bg_color = Color32::from_rgb(241, 245, 249);

        visuals.window_stroke = Stroke::new(1.0_f32, Color32::from_rgb(203, 213, 225));
        visuals.window_rounding = egui::Rounding::same(8.0);
        visuals.menu_rounding = egui::Rounding::same(6.0);

        let rounding = egui::Rounding::same(5.0);

        visuals.widgets.noninteractive.bg_fill = Color32::from_rgb(248, 250, 252);
        visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, Color32::from_rgb(226, 232, 240));
        visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, Color32::from_rgb(30, 41, 59));
        visuals.widgets.noninteractive.rounding = rounding;

        visuals.widgets.inactive.bg_fill = Color32::from_rgb(255, 255, 255);
        visuals.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, Color32::from_rgb(203, 213, 225));
        visuals.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, Color32::from_rgb(15, 23, 42));
        visuals.widgets.inactive.rounding = rounding;

        visuals.widgets.hovered.bg_fill = Color32::from_rgb(238, 242, 246);
        visuals.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, Color32::from_rgb(37, 99, 235));
        visuals.widgets.hovered.fg_stroke = Stroke::new(1.0_f32, Color32::from_rgb(15, 23, 42));
        visuals.widgets.hovered.rounding = rounding;

        visuals.widgets.active.bg_fill = Color32::from_rgb(37, 99, 235);
        visuals.widgets.active.bg_stroke = Stroke::new(1.0_f32, Color32::from_rgb(29, 78, 216));
        visuals.widgets.active.fg_stroke = Stroke::new(1.0_f32, Color32::WHITE);
        visuals.widgets.active.rounding = rounding;

        visuals.widgets.open.bg_fill = Color32::from_rgb(241, 245, 249);
        visuals.widgets.open.bg_stroke = Stroke::new(1.0_f32, Color32::from_rgb(37, 99, 235));
        visuals.widgets.open.fg_stroke = Stroke::new(1.0_f32, Color32::from_rgb(15, 23, 42));
        visuals.widgets.open.rounding = rounding;

        visuals.selection.bg_fill = Color32::from_rgb(37, 99, 235);
        visuals.selection.stroke = Stroke::new(1.0_f32, Color32::from_rgb(29, 78, 216));
    }

    ctx.set_visuals(visuals);

    let mut style = (*ctx.style()).clone();
    style.spacing.item_spacing = egui::vec2(6.0, 5.0);
    style.spacing.button_padding = egui::vec2(8.0, 4.0);
    ctx.set_style(style);
}

fn main() -> eframe::Result<()> {
    tracing_subscriber::fmt::init();

    // Support optional CLI file argument: `ravpdf-gui <file.pdf>`
    let mut initial_app = RavPdfApp::default();
    let mut args = std::env::args().skip(1);
    if let Some(arg) = args.next() {
        let path = PathBuf::from(arg);
        if path.exists() {
            initial_app.open_file(path);
        }
    }

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("RAV PDF")
            .with_inner_size([1200.0, 800.0])
            .with_min_inner_size([800.0, 600.0]),
        ..Default::default()
    };

    eframe::run_native(
        "RAV PDF",
        options,
        Box::new(|cc| {
            setup_custom_theme(&cc.egui_ctx, true);
            Ok(Box::new(initial_app))
        }),
    )
}
