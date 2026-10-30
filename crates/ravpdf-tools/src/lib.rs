//! RavPDF Document Tools, Watermarking, Bates Numbering, Metadata, Search, and Redaction.
//!
//! Traceability:
//! - FR-15 (Redaction)
//! - FR-17 (Document Comparison)
//! - FR-18 (Headers, Footers, Watermarks, Bates Numbering)
//! - FR-22 & FR-23 (Standards Conformance & Preflight)
//! - FR-25 (Optimization & Compression)
//! - FR-29 (File Attachments)
//! - FR-30 (Batch Processing & Action Wizard)
//! - FR-33 (Document Metadata)
//! - FR-34 (Search & Text Extraction)

pub mod accessibility;
pub mod attachments;
pub mod batch;
pub mod bates;
pub mod color_management;
pub mod comments;
pub mod compare;
pub mod export;
pub mod header_footer;
pub mod js;
pub mod linearize;
pub mod measure;
pub mod metadata;
pub mod ocr;
pub mod optimize;
pub mod plugin;
pub mod portfolio;
pub mod preflight;
pub mod print;
pub mod print_production;
pub mod redact;
pub mod rich_media;
pub mod scan_cleanup;
pub mod search;
pub mod standards;
pub mod threed;
pub mod watermark;

pub use accessibility::{
    audit_accessibility, fix_document_language, fix_document_title, fix_ensure_tagged_pdf,
    fix_tab_order_to_structure, remediate_all_automatic, AccessibilityCategory,
    AccessibilityReport, AccessibilityRule, AccessibilityStatus,
};
pub use attachments::{add_attachment, get_attachments, remove_attachment, FileAttachment};
pub use batch::{
    execute_batch, execute_batch_cancellable, ActionSequence, BatchAction, BatchFileResult,
    BatchJob, BatchResult,
};
pub use bates::apply_bates_numbering;
pub use color_management::{
    apply_soft_proof, convert_cmyk_to_gray, convert_cmyk_to_rgb_profile, convert_document_colors,
    convert_rgb_to_cmyk_profile, convert_rgb_to_gray, embed_output_intent, get_output_intents,
    OutputIntentInfo, RenderingIntent, SoftProofConfig, StandardIccProfile,
};
pub use comments::{
    compare_reviewer_comments, export_comments_to_csv, get_document_comments,
    merge_reviewer_annotations, AnnotationDiff, ReviewComment,
};
pub use compare::{compare_documents, generate_comparison_pdf_report, ComparisonReport, PageDiff};
pub use export::{
    export_page_to_image, export_to_html, export_to_plain_text, ExportImageFormat,
};
pub use header_footer::{apply_headers_and_footers, HeaderFooterConfig};
pub use js::{
    add_document_javascript, eval_expression, execute_all_field_calculations,
    execute_field_formatting, execute_field_validation, execute_script, get_document_javascript,
    JsAlert, JsEvent, JsExecutionContext, JsFormField, JsMailDocAction, JsSubmitFormAction, JsValue,
};
pub use linearize::{is_linearized, linearize_document, LinearizationParams};
pub use measure::{
    add_measurement_line_annotation, calculate_area, calculate_distance,
    calculate_geographic_area_sq_meters, calculate_geographic_distance_meters,
    calculate_perimeter, embed_geospatial_viewport, get_page_geospatial_viewports, snap_point,
    AreaResult, AreaUnit, DistanceResult, GeoCoordinate, GeospatialViewport, LinearUnit,
    PerimeterResult, ScaleRatio, SnapOptions,
};
pub use metadata::{get_document_info, set_document_info, DocumentInfo};
pub use ocr::{
    create_pdf_from_image_with_ocr, perform_ocr_document, perform_ocr_on_page, OcrConfig,
    OcrDocumentResult, OcrLine, OcrOutputMode, OcrPageResult, OcrWord,
};
pub use optimize::{audit_space_usage, optimize_document, OptimizationStats, SpaceAudit};
pub use plugin::{
    ContextMenuTarget, MenuCategory, PluginContextMenuEntry, PluginHost, PluginInfo,
    PluginManifest, PluginMenuItem, PluginPermission, PluginRegistry, PluginToolbarButton,
    RavPdfPlugin, SampleWatermarkPlugin,
};
pub use portfolio::{
    add_to_portfolio, create_portfolio, extract_portfolio_item, get_portfolio, is_portfolio,
    remove_from_portfolio, search_portfolio, Portfolio, PortfolioItem, PortfolioLayout,
    PortfolioSort,
};
pub use preflight::{
    execute_preflight_fixup, run_preflight, PreflightCheck, PreflightFixup, PreflightProfile,
    PreflightReport, PreflightSeverity,
};
pub use print_production::{
    add_printer_marks, apply_ink_manager, apply_plate_separations, flatten_document_transparency,
    get_document_inks, get_trap_presets, inspect_page_objects, set_trap_presets,
    DocumentInkInfo, InkManagerConfig, InkMapping, ObjectInspectionReport, PlateSeparationConfig,
    PrinterMarksConfig, SeparationAnalysis, TransparencyFlattenConfig, TransparencyFlattenReport,
    TrapPreset,
};
pub use standards::{
    convert_to_standard, embed_standard_xmp_metadata, generate_standard_xmp,
    validate_standard_compliance, StandardCheckResult, StandardConversionConfig,
    StandardConversionReport, StandardFormat, StandardValidationReport,
};
pub use print::{
    calculate_booklet_imposition, calculate_nup_imposition, calculate_poster_tiles,
    generate_postscript, generate_print_preview_layout, parse_page_range, BookletBinding,
    BookletSheet, BookletSubset, NUpCellPlacement, NUpCount, NUpOrder, NUpSheet, PosterTile,
    PrintAnnotationMode, PrintDuplex, PrintJobConfig, PrintLayoutMode, PrintOrientation,
    PrintPreviewReport, PrintScaling,
};
pub use redact::{
    apply_batch_redactions, redact_page_area, redact_page_area_with_appearance, redact_pattern,
    sanitize_document, RedactionAppearance, SanitizeOptions, SanitizeReport,
};
pub use rich_media::{
    add_rich_media_annotation, extract_rich_media_content, get_rich_media_annotations,
    MediaContent, RichMediaActivation, RichMediaAnnotation, RichMediaConfig, RichMediaDeactivation,
};
pub use scan_cleanup::{
    auto_crop_page, batch_scan_to_pdf, binarize_otsu, clean_scanned_image, compute_otsu_threshold,
    despeckle, detect_page_orientation, detect_skew_angle, list_scanner_devices, rotate_180,
    rotate_270_cw, rotate_90_cw, rotate_image_bilinear, whiten_background, ScanCleanupStats,
    ScanColorMode, ScanDpi, ScanDuplex, ScanPageSize, ScannerDevice, ScannerDriver,
    ScannerJobConfig,
};
pub use search::{
    build_search_regex, extract_all_text, extract_page_text, find_and_replace,
    find_and_replace_page, search_document, SearchMatch,
};
pub use threed::{
    add_3d_annotation, extract_3d_stream, get_3d_annotations, CrossSectionPlane, ThreeDAnnotation,
    ThreeDConfig, ThreeDFormat, ThreeDLighting, ThreeDNode, ThreeDRenderMode, ThreeDView,
};
pub use watermark::apply_text_watermark;

#[cfg(test)]
mod tests {
    use super::*;
    use ravpdf_core::document::PdfDocument;

    #[test]
    fn test_watermark_and_metadata() {
        let mut doc = PdfDocument::new();
        doc.add_blank_page(612.0, 792.0);

        let mut info = DocumentInfo::default();
        info.title = Some("Confidential Agreement".into());
        info.author = Some("Legal Counsel".into());
        set_document_info(&mut doc, &info).expect("set doc info");

        let read_info = get_document_info(&doc).expect("get doc info");
        assert_eq!(read_info.title.as_deref(), Some("Confidential Agreement"));

        apply_text_watermark(&mut doc, "CONFIDENTIAL", 0.3, 45.0, 60.0)
            .expect("apply watermark");
        apply_bates_numbering(&mut doc, "DOC", 1, 6, "").expect("apply bates");

        let serialized = doc.to_bytes().expect("serialize doc");
        let reloaded = PdfDocument::load_from_bytes(&serialized).expect("reload doc");
        assert_eq!(reloaded.page_count(), 1);
    }
}
