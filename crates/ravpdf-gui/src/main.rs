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
    add_annotation_to_page, get_page_annotations, Annotation, AnnotationType, LinkAction,
};
use ravpdf_core::document::{paper_sizes, PdfDocument};
use ravpdf_core::outline::{add_bookmark, Bookmark};
use ravpdf_edit::{
    create_pdf_from_images, create_pdf_from_text, crop_page, delete_page, duplicate_page,
    extract_pages, get_page_label, get_page_labels, insert_blank_page, insert_image_file,
    merge_documents, reorder_pages, rotate_page, set_page_labels, split_document,
    ImagePdfOptions, PageLabelRange, PageLabelStyle, TextPdfOptions,
};
use ravpdf_crypto::{
    create_signature_field, encrypt_document, get_document_signatures, Permissions,
    SignatureStatus,
};
use ravpdf_forms::{
    add_form_field, flatten_form, get_form_fields, get_page_form_fields, set_form_field_value,
    FormField, FormFieldType,
};
use ravpdf_render::{pixmap_to_png, render_page, RenderOptions};
use ravpdf_tools::{
    add_attachment, add_measurement_line_annotation, apply_bates_numbering,
    apply_headers_and_footers, apply_text_watermark, audit_accessibility, audit_space_usage,
    calculate_area, calculate_distance, compare_documents, create_portfolio,
    execute_batch, export_comments_to_csv, export_page_to_image, export_to_html,
    export_to_plain_text, extract_all_text, find_and_replace, find_and_replace_page,
    fix_document_language, fix_document_title, fix_ensure_tagged_pdf, fix_tab_order_to_structure,
    get_attachments, get_document_comments, get_page_geospatial_viewports, get_portfolio,
    is_linearized, is_portfolio, linearize_document, merge_reviewer_annotations, optimize_document,
    redact_page_area, redact_pattern, remediate_all_automatic, remove_attachment, run_preflight,
    search_document, set_document_info, AccessibilityReport, AccessibilityStatus,
    AreaUnit, BatchAction, BatchJob, BatchResult, ComparisonReport, ExportImageFormat, FileAttachment,
    HeaderFooterConfig, LinearUnit, OptimizationStats, PortfolioItem, PortfolioLayout,
    PreflightProfile, PreflightReport, PreflightSeverity, ReviewComment, ScaleRatio, SearchMatch,
    SnapOptions, SpaceAudit,
};
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

/// Parses 1-based page range string (e.g. "1-2, 3, 5-6") into 0-based index pairs (start, end) inclusive (FR-07.3).
fn parse_page_ranges(input: &str, total_pages: usize) -> Vec<(usize, usize)> {
    let mut ranges = Vec::new();
    for part in input.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        if let Some((start_s, end_s)) = part.split_once('-') {
            if let (Ok(s), Ok(e)) = (start_s.trim().parse::<usize>(), end_s.trim().parse::<usize>()) {
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
    StickyNote,
    Highlight,
    Rectangle,
    Text,
    Link,
    Redact,
    MeasureDistance,
    MeasureArea,
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
    batch_result: Option<BatchResult>,

    // Bookmark creation modal state (FR-19.2)
    new_bookmark_title: String,

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
    crop_left: f64,
    crop_bottom: f64,
    crop_right: f64,
    crop_top: f64,
    crop_all_pages: bool,

    // Digital Signatures state (FR-13)
    show_add_signature_dialog: bool,
    sig_target_page: usize,
    sig_field_name: String,
    sig_signer_name: String,
    sig_reason: String,
    sig_location: String,
    sig_is_visible: bool,

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
            crop_left: 0.0,
            crop_bottom: 0.0,
            crop_right: 612.0,
            crop_top: 792.0,
            crop_all_pages: false,

            show_add_signature_dialog: false,
            sig_target_page: 0,
            sig_field_name: "Signature1".into(),
            sig_signer_name: "Authorized Signer".into(),
            sig_reason: "Document Approved".into(),
            sig_location: "Headquarters".into(),
            sig_is_visible: true,

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
            batch_result: None,

            new_bookmark_title: String::new(),

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
        }
    }
}

impl RavPdfApp {
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
        match PdfDocument::load_from_file(&path) {
            Ok(doc) => {
                let file_name = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("Document.pdf")
                    .to_string();

                let id = self.next_tab_id;
                self.next_tab_id += 1;

                let tab = DocumentTab::new(id, file_name.clone(), doc, Some(path.clone()));
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
            .add_filter("Image Files", &["png", "jpg", "jpeg", "bmp", "webp", "tiff", "gif"])
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
                Ok(content) => {
                    match create_pdf_from_text(&content, &TextPdfOptions::default()) {
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
                            self.status_message = format!("Created PDF from text file '{}'", file.display());
                        }
                        Err(e) => {
                            self.status_message = format!("Failed to create PDF from text: {}", e);
                        }
                    }
                }
                Err(e) => {
                    self.status_message = format!("Failed to read text file: {}", e);
                }
            }
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
                self.status_message = format!("Exported {}/{} pages as PNG to '{}'", exported, page_count, folder.display());
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

                let mut annot = Annotation::new(
                    AnnotationType::Stamp(stamp_name.to_string()),
                    rect,
                );
                annot.color = Some(color);
                let _ = add_annotation_to_page(&mut tab.doc, curr_page, &annot);
                tab.is_modified = true;
                tab.invalidate_render();
                self.status_message = format!("Inserted '{}' stamp on page {}", stamp_name, curr_page + 1);
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
                        let fname = path
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("image");
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

            // Zoom shortcuts
            if i.modifiers.ctrl && (i.key_pressed(Key::Plus) || i.key_pressed(Key::Equals)) && has_doc {
                if let Some(tab) = self.active_tab_mut() {
                    tab.zoom = (tab.zoom * 1.25).min(64.0);
                    tab.invalidate_render();
                }
            }
            if i.modifiers.ctrl && i.key_pressed(Key::Minus) && has_doc {
                if let Some(tab) = self.active_tab_mut() {
                    tab.zoom = (tab.zoom / 1.25).max(0.1);
                    tab.invalidate_render();
                }
            }

            // Navigation shortcuts (FR-01.2.1)
            if has_doc && total_pages > 0 {
                if let Some(tab) = self.active_tab_mut() {
                    if (i.key_pressed(Key::PageDown) || i.key_pressed(Key::ArrowRight))
                        && tab.current_page + 1 < total_pages
                    {
                        tab.current_page += 1;
                        tab.page_input_buffer = (tab.current_page + 1).to_string();
                        tab.invalidate_render();
                    }
                    if (i.key_pressed(Key::PageUp) || i.key_pressed(Key::ArrowLeft))
                        && tab.current_page > 0
                    {
                        tab.current_page -= 1;
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
            .min_height(28.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(4.0, 0.0);

                    for (idx, tab) in self.tabs.iter().enumerate() {
                        let is_active = self.active_tab_idx == Some(idx);
                        let mod_mark = if tab.is_modified { "* " } else { "" };
                        let label_text = format!("{}📄 {}", mod_mark, tab.title);

                        let (bg_color, stroke_color) = if is_active {
                            (Color32::from_rgb(45, 55, 72), Color32::from_rgb(66, 153, 225))
                        } else {
                            (Color32::from_rgb(26, 32, 44), Color32::from_gray(60))
                        };

                        let frame = egui::Frame::none()
                            .fill(bg_color)
                            .stroke(Stroke::new(1.0_f32, stroke_color))
                            .rounding(4.0)
                            .inner_margin(egui::Margin::symmetric(8.0, 4.0));

                        frame.show(ui, |ui| {
                            ui.horizontal(|ui| {
                                let label = ui.selectable_label(is_active, label_text);
                                if label.clicked() {
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
                        .button("➕")
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
                    if ui.button("New PDF from Images... (FR-02.4)").clicked() {
                        self.create_pdf_from_images_dialog();
                        ui.close_menu();
                    }
                    if ui.button("New PDF from Text File... (FR-02.5)").clicked() {
                        self.create_pdf_from_text_dialog();
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Open... (Ctrl+O)").clicked() {
                        self.open_file_dialog();
                        ui.close_menu();
                    }
                    if ui.button("Combine / Merge Files... (FR-07.1)").clicked() {
                        self.show_combine_dialog = true;
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.add_enabled(has_doc, Button::new("Save (Ctrl+S)")).clicked() {
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
                    if ui
                        .add_enabled(has_doc, Button::new("Save As..."))
                        .clicked()
                    {
                        self.save_file_dialog();
                        ui.close_menu();
                    }
                    ui.separator();
                    ui.menu_button("Export (FR-03.2)", |ui| {
                        if ui.add_enabled(has_doc, Button::new("Export Document / Pages... (FR-03.2)")).clicked() {
                            self.show_export_dialog = true;
                            ui.close_menu();
                        }
                        if ui.add_enabled(has_doc, Button::new("Export HTML (.html)... (FR-03.2.6)")).clicked() {
                            self.export_html_dialog();
                            ui.close_menu();
                        }
                        if ui.add_enabled(has_doc, Button::new("Export Pages as PNG... (FR-03.2.4)")).clicked() {
                            self.export_pages_as_png_dialog();
                            ui.close_menu();
                        }
                        if ui.add_enabled(has_doc, Button::new("Export Text (.txt)... (FR-03.2.5)")).clicked() {
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
                        .add_enabled(has_doc, Button::new("Reduce File Size... (FR-25.1)"))
                        .clicked()
                    {
                        self.show_optimize_dialog = true;
                        ui.close_menu();
                    }
                    if ui.button("Create PDF Portfolio... (FR-29.2)").clicked() {
                        self.show_portfolio_dialog = true;
                        ui.close_menu();
                    }
                    if ui
                        .add_enabled(has_doc, Button::new("Linearize for Fast Web View... (FR-02.8)"))
                        .clicked()
                    {
                        self.show_linearize_dialog = true;
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
                        .add_enabled(has_doc, Button::new("Find in Document (Ctrl+F)"))
                        .clicked()
                    {
                        self.search_bar_open = true;
                        self.search_replace_mode = false;
                        ui.close_menu();
                    }
                    if ui
                        .add_enabled(has_doc, Button::new("Find & Replace... (Ctrl+H) (FR-04.8)"))
                        .clicked()
                    {
                        self.search_bar_open = true;
                        self.search_replace_mode = true;
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui
                        .add_enabled(has_doc, Button::new("Insert Image File... (FR-05.1)"))
                        .clicked()
                    {
                        self.insert_image_dialog();
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
                        .add_enabled(has_doc, Button::new("Duplicate Current Page (FR-06.11)"))
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
                        .add_enabled(has_doc, Button::new("Crop Page... (FR-06.8)"))
                        .clicked()
                    {
                        if let Some((page, crop)) = self.active_tab().and_then(|tab| {
                            let p = tab.doc.get_page(tab.current_page)?;
                            Some((tab.current_page, p.crop_box))
                        }) {
                            self.crop_target_page = page;
                            self.crop_left = crop[0];
                            self.crop_bottom = crop[1];
                            self.crop_right = crop[2];
                            self.crop_top = crop[3];
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
                        .add_enabled(has_doc, Button::new("Page Numbering & Labels... (FR-06.9)"))
                        .clicked()
                    {
                        if let Some(tab) = self.active_tab() {
                            self.page_labels_start_page = tab.current_page + 1;
                        }
                        self.show_page_labels_dialog = true;
                        ui.close_menu();
                    }
                    if ui
                        .add_enabled(has_doc, Button::new("Add Hyperlink... (FR-20.1)"))
                        .clicked()
                    {
                        if let Some(tab) = self.active_tab() {
                            self.link_target_page = (tab.current_page + 2).min(tab.doc.page_count());
                        }
                        self.show_link_dialog = true;
                        ui.close_menu();
                    }
                    if ui
                        .add_enabled(has_doc, Button::new("Add Form Field... (FR-11.1)"))
                        .clicked()
                    {
                        self.show_add_form_field_dialog = true;
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui
                        .add_enabled(has_doc, Button::new("Split Document... (FR-07.3)"))
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
                        let avail_w = screen.width() - (if self.sidebar_open { 260.0 } else { 60.0 });
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
                        let avail_w = screen.width() - (if self.sidebar_open { 240.0 } else { 40.0 });
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
                });

                ui.menu_button("Tools", |ui| {
                    if ui
                        .add_enabled(has_doc, Button::new("Add Watermark... (FR-18.2)"))
                        .clicked()
                    {
                        self.show_watermark_dialog = true;
                        ui.close_menu();
                    }
                    if ui
                        .add_enabled(has_doc, Button::new("Bates Numbering... (FR-18.3)"))
                        .clicked()
                    {
                        self.show_bates_dialog = true;
                        ui.close_menu();
                    }
                    if ui
                        .add_enabled(has_doc, Button::new("Headers & Footers... (FR-18.1)"))
                        .clicked()
                    {
                        self.show_header_footer_dialog = true;
                        ui.close_menu();
                    }
                    ui.separator();
                    ui.menu_button("Rubber Stamps (FR-10)", |ui| {
                        if ui.add_enabled(has_doc, Button::new("✓ APPROVED (Green)")).clicked() {
                            self.insert_rubber_stamp("APPROVED", (0.1, 0.7, 0.2));
                            ui.close_menu();
                        }
                        if ui.add_enabled(has_doc, Button::new("🔒 CONFIDENTIAL (Red)")).clicked() {
                            self.insert_rubber_stamp("CONFIDENTIAL", (0.8, 0.1, 0.1));
                            ui.close_menu();
                        }
                        if ui.add_enabled(has_doc, Button::new("📝 DRAFT (Blue)")).clicked() {
                            self.insert_rubber_stamp("DRAFT", (0.1, 0.4, 0.8));
                            ui.close_menu();
                        }
                        if ui.add_enabled(has_doc, Button::new("★ FINAL (Purple)")).clicked() {
                            self.insert_rubber_stamp("FINAL", (0.5, 0.2, 0.7));
                            ui.close_menu();
                        }
                        if ui.add_enabled(has_doc, Button::new("⊘ VOID (Dark Red)")).clicked() {
                            self.insert_rubber_stamp("VOID", (0.7, 0.0, 0.0));
                            ui.close_menu();
                        }
                    });
                    ui.separator();
                    if ui
                        .add_enabled(has_doc, Button::new("Flatten All Forms (FR-11.3.4)"))
                        .clicked()
                    {
                        if let Some(tab) = self.active_tab_mut() {
                            let _ = flatten_form(&mut tab.doc);
                            tab.is_modified = true;
                            tab.invalidate_render();
                            self.status_message = "All form fields flattened to page content".into();
                        }
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui
                        .add_enabled(has_doc, Button::new("Sanitize / Redact Patterns... (FR-15.4)"))
                        .clicked()
                    {
                        self.show_redact_pattern_dialog = true;
                        ui.close_menu();
                    }
                    if ui
                        .add_enabled(has_doc, Button::new("Protect & Encrypt Document... (FR-14)"))
                        .clicked()
                    {
                        self.show_encrypt_dialog = true;
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui
                        .add_enabled(has_doc, Button::new("Audit Space Usage... (FR-25.9)"))
                        .clicked()
                    {
                        if let Some(tab) = self.active_tab() {
                            self.space_audit = Some(audit_space_usage(&tab.doc));
                            self.show_space_audit_dialog = true;
                        }
                        ui.close_menu();
                    }
                    if ui.button("Compare Two Documents... (FR-17)").clicked() {
                        if let Some(tab) = self.active_tab() {
                            self.compare_file_a = tab.file_path.clone();
                        }
                        self.show_compare_dialog = true;
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui
                        .add_enabled(has_doc, Button::new("Preflight & Standards Validation... (FR-22, FR-23)"))
                        .clicked()
                    {
                        self.show_preflight_dialog = true;
                        ui.close_menu();
                    }
                    if ui
                        .add_enabled(has_doc, Button::new("Digital Signatures & Certificates... (FR-13)"))
                        .clicked()
                    {
                        self.sidebar_tab = SidebarTab::Signatures;
                        self.sidebar_open = true;
                        ui.close_menu();
                    }
                    if ui.button("Action Wizard (Batch Processing)... (FR-30)").clicked() {
                        self.show_batch_dialog = true;
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui
                        .add_enabled(has_doc, Button::new("Accessibility Checker... (FR-21.1)"))
                        .clicked()
                    {
                        if let Some(tab) = self.active_tab() {
                            let rep = audit_accessibility(&tab.doc).ok();
                            let title = tab.metadata.as_ref().and_then(|m| m.title.clone()).unwrap_or_else(|| tab.title.clone());
                            let lang = tab.doc.catalog().and_then(|c| c.get_str("Lang")).unwrap_or_else(|| "en-US".to_string());
                            self.accessibility_report = rep;
                            self.accessibility_fix_title_input = title;
                            self.accessibility_fix_lang_input = lang;
                            self.show_accessibility_dialog = true;
                        }
                        ui.close_menu();
                    }
                    if ui
                        .add_enabled(has_doc, Button::new("Review & Comments Summary... (FR-31.5)"))
                        .clicked()
                    {
                        if let Some(tab) = self.active_tab() {
                            self.comments_list = get_document_comments(&tab.doc).unwrap_or_default();
                            self.show_comments_summary_dialog = true;
                        }
                        ui.close_menu();
                    }
                    if ui
                        .add_enabled(has_doc, Button::new("Merge Comments from Reviewer... (FR-31.3)"))
                        .clicked()
                    {
                        self.show_merge_comments_dialog = true;
                        ui.close_menu();
                    }
                    if ui
                        .add_enabled(has_doc, Button::new("Measurement Scale & Calibration... (FR-26.5)"))
                        .clicked()
                    {
                        self.show_measure_scale_dialog = true;
                        ui.close_menu();
                    }
                });

                ui.menu_button("Help", |ui| {
                    if ui.button("About RAV PDF").clicked() {
                        self.status_message =
                            "RAV PDF 1.0 (ISO 32000 Conformance)".into();
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

                if ui
                    .add_enabled(has_doc, Button::new("💾 Save"))
                    .clicked()
                {
                    self.save_file_dialog();
                }

                ui.separator();

                if ui
                    .button(if self.sidebar_open { "◀ Sidebar" } else { "▶ Sidebar" })
                    .clicked()
                {
                    self.sidebar_open = !self.sidebar_open;
                }

                ui.separator();

                let total_pages = self.active_tab().map(|t| t.doc.page_count()).unwrap_or(0);
                let current_page = self.active_tab().map(|t| t.current_page).unwrap_or(0);
                let can_prev = has_doc && current_page > 0;
                let can_next = has_doc && current_page + 1 < total_pages;

                if ui.add_enabled(can_prev, Button::new("⏮")).clicked() {
                    if let Some(tab) = self.active_tab_mut() {
                        tab.current_page = 0;
                        tab.page_input_buffer = "1".into();
                        tab.invalidate_render();
                    }
                }
                if ui.add_enabled(can_prev, Button::new("◀")).clicked() {
                    if let Some(tab) = self.active_tab_mut() {
                        tab.current_page -= 1;
                        tab.page_input_buffer = (tab.current_page + 1).to_string();
                        tab.invalidate_render();
                    }
                }

                // Go to page quick input box (FR-01.2.6) with Page Label support (FR-06.9)
                if has_doc {
                    let page_label_opt = self.active_tab().map(|t| get_page_label(&t.doc, current_page));
                    ui.label("Page");
                    if let Some(tab) = self.active_tab_mut() {
                        let text_edit = ui.add(
                            egui::TextEdit::singleline(&mut tab.page_input_buffer)
                                .desired_width(32.0),
                        );
                        if text_edit.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                            if let Ok(p) = tab.page_input_buffer.trim().parse::<usize>() {
                                if p >= 1 && p <= total_pages {
                                    tab.current_page = p - 1;
                                    tab.invalidate_render();
                                } else {
                                    tab.page_input_buffer = (tab.current_page + 1).to_string();
                                }
                            }
                        }
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
                    if let Some(tab) = self.active_tab_mut() {
                        tab.current_page += 1;
                        tab.page_input_buffer = (tab.current_page + 1).to_string();
                        tab.invalidate_render();
                    }
                }
                if ui.add_enabled(can_next, Button::new("⏭")).clicked() {
                    if let Some(tab) = self.active_tab_mut() {
                        tab.current_page = total_pages.saturating_sub(1);
                        tab.page_input_buffer = (tab.current_page + 1).to_string();
                        tab.invalidate_render();
                    }
                }

                ui.separator();

                // Zoom controls & fit modes (FR-01.3.2)
                let zoom_pct = self.active_tab().map(|t| t.zoom * 100.0).unwrap_or(100.0);
                if ui.add_enabled(has_doc, Button::new("➖")).clicked() {
                    if let Some(tab) = self.active_tab_mut() {
                        tab.zoom = (tab.zoom / 1.2).max(0.1);
                        tab.invalidate_render();
                    }
                }
                ui.label(format!("{:.0}%", zoom_pct));
                if ui.add_enabled(has_doc, Button::new("➕")).clicked() {
                    if let Some(tab) = self.active_tab_mut() {
                        tab.zoom = (tab.zoom * 1.2).min(64.0);
                        tab.invalidate_render();
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

                // Find & Replace in document triggers (FR-34.1, FR-04.8)
                if ui.add_enabled(has_doc, Button::new("🔍 Find")).clicked() {
                    self.search_bar_open = !self.search_bar_open;
                    self.search_replace_mode = false;
                }
                if ui.add_enabled(has_doc, Button::new("🔍✏ Replace")).clicked() {
                    self.search_bar_open = true;
                    self.search_replace_mode = true;
                }
                if ui
                    .add_enabled(has_doc, Button::new("🖼 Image"))
                    .on_hover_text("Insert image onto page (FR-05.1)")
                    .clicked()
                {
                    self.insert_image_dialog();
                }

                ui.separator();

                // Tool Selector Buttons (FR-08, FR-09, FR-20)
                ui.add_enabled_ui(has_doc, |ui| {
                    ui.selectable_value(&mut self.active_tool, ActiveTool::Select, "Select");
                    ui.selectable_value(&mut self.active_tool, ActiveTool::Hand, "Pan");
                    ui.selectable_value(&mut self.active_tool, ActiveTool::StickyNote, "Note");
                    ui.selectable_value(&mut self.active_tool, ActiveTool::Highlight, "Highlight");
                    ui.selectable_value(&mut self.active_tool, ActiveTool::Rectangle, "Shape");
                    ui.selectable_value(&mut self.active_tool, ActiveTool::Text, "Text");
                    ui.selectable_value(&mut self.active_tool, ActiveTool::Link, "🔗 Link");
                    ui.selectable_value(&mut self.active_tool, ActiveTool::Redact, "Redact");
                    ui.selectable_value(&mut self.active_tool, ActiveTool::MeasureDistance, "📏 Distance");
                    ui.selectable_value(&mut self.active_tool, ActiveTool::MeasureArea, "📐 Area");

                    if self.active_tool == ActiveTool::MeasureDistance || self.active_tool == ActiveTool::MeasureArea {
                        ui.separator();
                        if ui.small_button("⚙ Scale").on_hover_text("Configure scale ratio and measurement units (FR-26.5)").clicked() {
                            self.show_measure_scale_dialog = true;
                        }
                        ui.checkbox(&mut self.measure_bake_as_annot, "Annotate")
                            .on_hover_text("Bake measurements into document as annotations (FR-26.4)");
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
                                    msg = Some(format!("Replaced {} occurrences on page {}", count, curr + 1));
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
                                    msg = Some(format!("Replaced {} occurrences across document", count));
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
                ui.horizontal_wrapped(|ui| {
                    ui.selectable_value(&mut self.sidebar_tab, SidebarTab::Thumbnails, "Pages");
                    ui.selectable_value(&mut self.sidebar_tab, SidebarTab::Bookmarks, "Outline");
                    ui.selectable_value(&mut self.sidebar_tab, SidebarTab::Annotations, "Notes");
                    ui.selectable_value(&mut self.sidebar_tab, SidebarTab::Forms, "Forms");
                    ui.selectable_value(&mut self.sidebar_tab, SidebarTab::Attachments, "Files");
                    ui.selectable_value(&mut self.sidebar_tab, SidebarTab::Signatures, "Sign");
                    ui.selectable_value(&mut self.sidebar_tab, SidebarTab::Properties, "Info");
                    ui.selectable_value(&mut self.sidebar_tab, SidebarTab::Search, "Results");
                    ui.selectable_value(&mut self.sidebar_tab, SidebarTab::Accessibility, "A11y");
                });

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
                                    .on_hover_text("Insert blank A4 page at end (FR-06.1)")
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
                                    .on_hover_text("Configure page numbering schemes (FR-06.9)")
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
                                    Stroke::new(2.0_f32, Color32::from_rgb(66, 153, 225))
                                } else {
                                    Stroke::new(1.0_f32, Color32::from_gray(60))
                                };
                                let bg = if is_selected {
                                    Color32::from_rgb(30, 41, 59)
                                } else {
                                    Color32::from_rgb(20, 24, 33)
                                };

                                egui::Frame::none()
                                    .fill(bg)
                                    .stroke(stroke)
                                    .rounding(4.0)
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
                                                    .on_hover_text("Rotate 90° clockwise (FR-06.7)")
                                                    .clicked()
                                                {
                                                    page_action = Some(PageAction::Rotate(i));
                                                }
                                                if i > 0
                                                    && ui
                                                        .small_button("▲")
                                                        .on_hover_text("Move page up (FR-06.6)")
                                                        .clicked()
                                                {
                                                    page_action = Some(PageAction::MoveUp(i));
                                                }
                                                if i + 1 < total_pages
                                                    && ui
                                                        .small_button("▼")
                                                        .on_hover_text("Move page down (FR-06.6)")
                                                        .clicked()
                                                {
                                                    page_action = Some(PageAction::MoveDown(i));
                                                }
                                                if ui
                                                    .small_button("📄")
                                                    .on_hover_text("Extract page to new tab (FR-06.3)")
                                                    .clicked()
                                                {
                                                    page_action = Some(PageAction::Extract(i));
                                                }
                                                if ui
                                                    .small_button("📋")
                                                    .on_hover_text("Duplicate page (FR-06.11)")
                                                    .clicked()
                                                {
                                                    page_action = Some(PageAction::Duplicate(i));
                                                }
                                                if ui
                                                    .small_button("✂")
                                                    .on_hover_text("Crop page (FR-06.8)")
                                                    .clicked()
                                                {
                                                    page_action = Some(PageAction::Crop(i));
                                                }
                                                if total_pages > 1
                                                    && ui
                                                        .small_button("🗑")
                                                        .on_hover_text("Delete page (FR-06.2)")
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
                                    self.crop_target_page = idx;
                                    if let Some(tab) = self.active_tab() {
                                        if let Some(p) = tab.doc.get_page(idx) {
                                            self.crop_left = p.crop_box[0];
                                            self.crop_bottom = p.crop_box[1];
                                            self.crop_right = p.crop_box[2];
                                            self.crop_top = p.crop_box[3];
                                        }
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

                    SidebarTab::Annotations => {
                        ui.label("Annotations on current page:");
                        if let Some(tab) = self.active_tab() {
                            if let Ok(annots) = get_page_annotations(&tab.doc, tab.current_page) {
                                if annots.is_empty() {
                                    ui.weak("(No annotations on this page)");
                                } else {
                                    for (idx, annot) in annots.iter().enumerate() {
                                        ui.group(|ui| {
                                            ui.label(format!(
                                                "#{}: {}",
                                                idx + 1,
                                                annot.annot_type.subtype_name()
                                            ));
                                            if let Some(ref text) = annot.contents {
                                                ui.weak(text);
                                            }
                                        });
                                    }
                                }
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
                            ui.heading("Attachments (FR-29)");
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
                        ui.heading("Digital Signatures (FR-13)");
                        ui.add_space(4.0);

                        ui.horizontal(|ui| {
                            if ui.button("➕ Sign Document...").on_hover_text("Add digital signature field (FR-13.1, FR-13.10)").clicked() {
                                if let Some(tab) = self.active_tab() {
                                    self.sig_target_page = tab.current_page;
                                }
                                self.show_add_signature_dialog = true;
                            }
                        });

                        ui.separator();

                        let signatures = self.active_tab().and_then(|tab| get_document_signatures(&tab.doc, None).ok()).unwrap_or_default();
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
                                    .on_hover_text("Add interactive form field (FR-11.1)")
                                    .clicked()
                                {
                                    self.show_add_form_field_dialog = true;
                                }
                            });
                        });
                        ui.add_space(4.0);

                        let mut flatten = false;

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
                                        .on_hover_text("Permanently burn form fields into content (FR-11.6)")
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
                                                    FormFieldType::Choice => "▾ Choice",
                                                    FormFieldType::PushButton => "🔘 Button",
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
                                if ui.button("Auditing...").on_hover_text("Refresh accessibility audit (FR-21.1)").clicked() {
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
                                        .on_hover_text("Remediate title, language, tagged PDF structure, and tab order (FR-21.3-10)")
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
                                self.status_message = "Applied automatic accessibility remediations (FR-21)".into();
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
                        let viewports = get_page_geospatial_viewports(&tab.doc, tab.current_page).unwrap_or_default();
                        if !viewports.is_empty() {
                            ui.label(RichText::new("🌐 Geospatial PDF (WGS84)").color(Color32::from_rgb(0, 180, 216)).size(11.0));
                            ui.separator();
                        }
                        if self.active_tool == ActiveTool::MeasureDistance || self.active_tool == ActiveTool::MeasureArea {
                            ui.label(RichText::new(format!("Scale: {}", self.measure_scale.to_display_string())).size(11.0).color(Color32::from_rgb(100, 220, 255)));
                            ui.separator();
                        }
                        ui.label(format!("Tool: {:?}", self.active_tool));
                    }
                });
            });
        });
    }

    /// Renders central canvas area with PDF rendering, interactive annotations, and form widgets (EI-01.3).
    fn render_canvas(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            if self.tabs.is_empty() || self.active_tab().is_none() {
                self.render_welcome_screen(ui);
                return;
            }

            // Ensure current active tab page texture is rendered
            if let Some(tab) = self.active_tab_mut() {
                tab.ensure_page_rendered(ctx);
            }

            // Read properties of current active page
            let (page_size, page_h, zoom) = {
                let tab = self.active_tab().unwrap();
                let page = tab.doc.get_page(tab.current_page);
                let page_h = page
                    .as_ref()
                    .map(|p| (p.crop_box[3] - p.crop_box[1]).abs() as f32)
                    .unwrap_or(792.0);
                let tex_size = tab.page_texture.as_ref().map(|t| t.size_vec2()).unwrap_or(Vec2::ZERO);
                (tex_size, page_h, tab.zoom)
            };

            ScrollArea::both()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    if page_size.x <= 0.0 || page_size.y <= 0.0 {
                        return;
                    }

                    let (rect, response) =
                        ui.allocate_exact_size(page_size, egui::Sense::click_and_drag());

                    // 1. Draw page background and shadow
                    ui.painter().rect_filled(
                        rect.expand(2.0),
                        0.0,
                        Color32::from_black_alpha(40),
                    );
                    ui.painter().rect_filled(rect, 0.0, Color32::WHITE);

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
                                        _ => {}
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
                                            resp.on_hover_ui(|ui| {
                                                ui.label(format!("➔ Destination: {}", n));
                                            });
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

                    // 4. Annotation Drag-Drawing and Click Handlers (FR-08, FR-09)
                    if response.drag_started() {
                        if let Some(pos) = response.interact_pointer_pos() {
                            self.drag_start_pos = Some(pos);
                            self.drag_current_pos = Some(pos);
                        }
                    }

                    if response.dragged() {
                        if let Some(pos) = response.interact_pointer_pos() {
                            self.drag_current_pos = Some(pos);
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
                            ActiveTool::Rectangle => {
                                ui.painter().rect_stroke(
                                    drag_rect,
                                    0.0,
                                    Stroke::new(2.0_f32, Color32::from_rgb(220, 38, 38)),
                                );
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
                                    ActiveTool::Link => {
                                        self.link_rect = pdf_rect;
                                        let current_page = self.active_tab().map(|t| t.current_page).unwrap_or(0);
                                        let total_pages = self.active_tab().map(|t| t.doc.page_count()).unwrap_or(1);
                                        self.link_target_page = (current_page + 2).min(total_pages);
                                        self.show_link_dialog = true;
                                    }
                                    ActiveTool::Redact => {
                                        if let Some(tab) = self.active_tab_mut() {
                                            let curr_page = tab.current_page;
                                            let _ = redact_page_area(
                                                &mut tab.doc,
                                                curr_page,
                                                pdf_rect,
                                                Some("[REDACTED]"),
                                             );
                                            tab.is_modified = true;
                                            tab.invalidate_render();
                                            self.status_message = format!("Permanently redacted area on page {}", curr_page + 1);
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
                    }

                    // Click handlers for point-based tools
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
                                _ => {}
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

            painter.rect_stroke(
                rect,
                8.0,
                Stroke::new(1.5_f32, Color32::from_gray(100)),
            );

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
            egui::Window::new("Add Text Watermark (FR-18.2)")
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
                                let _ = apply_text_watermark(
                                    &mut tab.doc,
                                    &text,
                                    opacity,
                                    45.0,
                                    54.0,
                                );
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
            egui::Window::new("Apply Bates Numbering (FR-18.3)")
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
                                let _ = apply_bates_numbering(
                                    &mut tab.doc,
                                    &prefix,
                                    start,
                                    digits,
                                    "",
                                );
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
            egui::Window::new("Add Headers & Footers (FR-18.1)")
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
            egui::Window::new("Add Text Box Annotation (FR-08.1.4)")
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
            egui::Window::new("Combine / Merge PDF Files (FR-07.1)")
                .collapsible(false)
                .resizable(true)
                .default_size([480.0, 360.0])
                .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.label("Add PDF files in the desired order to merge them into a single document:");
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
                            ui.weak("(No files selected yet — click 'Add PDF Files' to select files)");
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
                                    if i > 0 && ui.small_button("▲").on_hover_text("Move up").clicked() {
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
                        if ui.add_enabled(can_combine, Button::new("Combine Files")).clicked() {
                            let mut docs = Vec::new();
                            let mut load_err = None;
                            for p in &self.combine_file_paths {
                                match PdfDocument::load_from_file(p) {
                                    Ok(d) => docs.push(d),
                                    Err(e) => {
                                        load_err = Some(format!("Failed to load {}: {}", p.display(), e));
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
                                        let title = format!("Merged Document ({} files).pdf", docs.len());
                                        let mut tab = DocumentTab::new(tab_id, title, merged, None);
                                        tab.is_modified = true;
                                        self.tabs.push(tab);
                                        self.active_tab_idx = Some(self.tabs.len() - 1);
                                        self.status_message = format!("Combined {} documents successfully", docs.len());
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
            egui::Window::new("Split Document (FR-07.3)")
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
                                let ranges = parse_page_ranges(&self.split_ranges_input, total_pages);
                                if ranges.is_empty() {
                                    self.status_message = "Invalid or empty page ranges specified".into();
                                } else if let Some(folder) = rfd::FileDialog::new().pick_folder() {
                                    match split_document(&tab.doc, &ranges) {
                                        Ok(docs) => {
                                            let mut count = 0;
                                            for (idx, doc) in docs.iter().enumerate() {
                                                let (s, e) = ranges[idx];
                                                let out_name = format!("split_pages_{}_to_{}.pdf", s + 1, e + 1);
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
            egui::Window::new("Document Properties (FR-33)")
                .collapsible(false)
                .resizable(true)
                .default_size([500.0, 420.0])
                .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.heading("General Metadata");
                    ui.add_space(4.0);
                    egui::Grid::new("prop_grid").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
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
                        let path_str = tab.file_path.as_ref().map(|p| p.display().to_string()).unwrap_or_else(|| "<Unsaved Document>".into());
                        ui.label(format!("File Path: {}", path_str));
                        ui.label(format!("Page Count: {}", tab.doc.page_count()));
                        ui.label(format!("PDF Version: %PDF-{}.{}", tab.doc.version.0, tab.doc.version.1));
                        if let Some(page) = tab.doc.get_page(tab.current_page) {
                            let w_pt = (page.crop_box[2] - page.crop_box[0]).abs();
                            let h_pt = (page.crop_box[3] - page.crop_box[1]).abs();
                            ui.label(format!("Page {} Size: {:.1} x {:.1} pt ({:.2}\" x {:.2}\")", tab.current_page + 1, w_pt, h_pt, w_pt / 72.0, h_pt / 72.0));
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
                    title: if self.prop_title.trim().is_empty() { None } else { Some(self.prop_title.clone()) },
                    author: if self.prop_author.trim().is_empty() { None } else { Some(self.prop_author.clone()) },
                    subject: if self.prop_subject.trim().is_empty() { None } else { Some(self.prop_subject.clone()) },
                    keywords: if self.prop_keywords.trim().is_empty() { None } else { Some(self.prop_keywords.clone()) },
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
            egui::Window::new("Sanitize & Redact Patterns (FR-15.4)")
                .collapsible(false)
                .resizable(false)
                .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.label("Permanently search and sanitize sensitive patterns across the document:");
                    ui.add_space(6.0);

                    ui.label(RichText::new("Quick Presets:").strong());
                    ui.horizontal(|ui| {
                        if ui.button("SSN").on_hover_text(r"\d{3}-\d{2}-\d{4}").clicked() {
                            self.redact_pattern_input = r"\d{3}-\d{2}-\d{4}".into();
                        }
                        if ui.button("Email").on_hover_text(r"[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}").clicked() {
                            self.redact_pattern_input = r"[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}".into();
                        }
                        if ui.button("Phone").on_hover_text(r"\d{3}[-.]?\d{3}[-.]?\d{4}").clicked() {
                            self.redact_pattern_input = r"\d{3}[-.]?\d{3}[-.]?\d{4}".into();
                        }
                        if ui.button("Credit Card").on_hover_text(r"\b(?:\d{4}[ -]?){3}\d{4}\b").clicked() {
                            self.redact_pattern_input = r"\b(?:\d{4}[ -]?){3}\d{4}\b".into();
                        }
                    });

                    ui.add_space(8.0);
                    egui::Grid::new("redact_grid").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
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
                        if ui.add_enabled(can_redact, Button::new("✂ Redact All Occurrences")).clicked() {
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
                            self.status_message = format!("Redacted {} occurrences of pattern '{}'", count, pattern);
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
            egui::Window::new("Password Protect & Encrypt (FR-14)")
                .collapsible(false)
                .resizable(false)
                .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.label("Encrypts the document using standard AES-128 security handler.");
                    ui.add_space(8.0);

                    egui::Grid::new("enc_grid").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
                        ui.label("User Password (to open):");
                        ui.add(egui::TextEdit::singleline(&mut self.encrypt_user_pass).password(true));
                        ui.end_row();

                        ui.label("Owner Password (permissions):");
                        ui.add(egui::TextEdit::singleline(&mut self.encrypt_owner_pass).password(true));
                        ui.end_row();
                    });

                    ui.add_space(8.0);
                    ui.heading("Permissions (FR-14.2)");
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
                                    match encrypt_document(&mut enc_doc, &self.encrypt_user_pass, &self.encrypt_owner_pass, perms) {
                                        Ok(()) => {
                                            if let Err(e) = enc_doc.save_to_file(&path) {
                                                self.status_message = format!("Save error: {}", e);
                                            } else {
                                                self.status_message = format!("Encrypted PDF saved to '{}'", path.display());
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
            egui::Window::new("Reduce File Size & Optimize (FR-25.1)")
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
                            self.status_message = format!("Optimization complete: {:.1}% reduction", stats.reduction_percent());
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
            egui::Window::new("Audit Space Usage (FR-25.9)")
                .collapsible(false)
                .resizable(false)
                .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    if let Some(ref audit) = self.space_audit {
                        ui.heading("Space Breakdown by Component");
                        ui.add_space(6.0);

                        egui::Grid::new("audit_grid").num_columns(3).spacing([16.0, 8.0]).show(ui, |ui| {
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
                            ui.label(RichText::new(format!("{} bytes", audit.total_bytes)).strong());
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
            egui::Window::new("Compare Two Documents (FR-17)")
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
                    match (PdfDocument::load_from_file(pa), PdfDocument::load_from_file(pb)) {
                        (Ok(da), Ok(db)) => {
                            self.comparison_report = Some(compare_documents(&da, &db));
                            self.status_message = "Comparison generated successfully".into();
                        }
                        (Err(e), _) => self.status_message = format!("Failed to load Document A: {}", e),
                        (_, Err(e)) => self.status_message = format!("Failed to load Document B: {}", e),
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
            egui::Window::new("Preflight & Standards Validation (FR-22, FR-23)")
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
                    }

                    ui.add_space(10.0);
                    if ui.button("Close").clicked() {
                        close = true;
                    }
                });

            if run_check {
                if let Some(tab) = self.active_tab() {
                    self.preflight_report = Some(run_preflight(&tab.doc, self.preflight_profile));
                    self.status_message = format!("Preflight audit completed for {}", self.preflight_profile.display_name());
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
            egui::Window::new("Action Wizard & Batch Processing (FR-30)")
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
                            if let Some(files) = rfd::FileDialog::new().add_filter("PDF files", &["pdf"]).pick_files() {
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
                        let out_name = self.batch_output_dir.as_ref()
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
                    ui.heading("Action Sequence to Apply (FR-30.4):");
                    ui.checkbox(&mut self.batch_action_optimize, "Optimize & Flate Compress (FR-25)");
                    ui.checkbox(&mut self.batch_action_flatten, "Flatten All Interactive Forms (FR-11)");

                    ui.horizontal(|ui| {
                        ui.checkbox(&mut self.batch_action_watermark, "Add Watermark (FR-18.2):");
                        if self.batch_action_watermark {
                            ui.text_edit_singleline(&mut self.batch_watermark_text);
                        }
                    });

                    ui.horizontal(|ui| {
                        ui.checkbox(&mut self.batch_action_bates, "Apply Bates Numbering (FR-18.3):");
                        if self.batch_action_bates {
                            ui.label("Prefix:");
                            ui.text_edit_singleline(&mut self.batch_bates_prefix);
                        }
                    });

                    ui.add_space(8.0);
                    let can_run = !self.batch_input_files.is_empty() && self.batch_output_dir.is_some();
                    if ui.add_enabled(can_run, Button::new("▶ Execute Batch Sequence")).clicked() {
                        run_batch = true;
                    }

                    if let Some(ref res) = self.batch_result {
                        ui.separator();
                        ui.heading("Execution Results:");
                        ui.label(format!("Total: {}, Succeeded: {}, Failed: {}", res.total_files, res.successful_files, res.failed_files));

                        ScrollArea::vertical().max_height(160.0).show(ui, |ui| {
                            for f in &res.details {
                                ui.horizontal(|ui| {
                                    let fname = f.path.file_name().and_then(|n| n.to_str()).unwrap_or("?");
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

                    let job = BatchJob {
                        input_files: self.batch_input_files.clone(),
                        output_directory: out_dir.clone(),
                        actions,
                    };
                    let result = execute_batch(&job);
                    self.status_message = format!("Batch execution finished: {}/{} succeeded", result.successful_files, result.total_files);
                    self.batch_result = Some(result);
                }
            }
            if close {
                self.show_batch_dialog = false;
            }
        }

        // Crop Page Dialog (FR-06.8)
        if self.show_crop_dialog {
            let mut close = false;
            let mut apply = false;
            egui::Window::new(format!("Crop Page {} (FR-06.8)", self.crop_target_page + 1))
                .collapsible(false)
                .resizable(false)
                .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.label("Specify new CropBox boundary coordinates (points):");
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
                    ui.weak(format!("Resulting dimensions: {:.1} × {:.1} pt ({:.2} × {:.2} in)", width, height, width / 72.0, height / 72.0));

                    ui.checkbox(&mut self.crop_all_pages, "Apply crop to all pages in document");

                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        if ui.button("✂ Apply Crop").clicked() {
                            apply = true;
                            close = true;
                        }
                        if ui.button("Cancel").clicked() {
                            close = true;
                        }
                    });
                });

            if apply {
                let crop_box = [self.crop_left, self.crop_bottom, self.crop_right, self.crop_top];
                let all_pages = self.crop_all_pages;
                let target_page = self.crop_target_page;
                let mut msg = String::new();

                if let Some(tab) = self.active_tab_mut() {
                    if all_pages {
                        let total = tab.doc.page_count();
                        for i in 0..total {
                            let _ = crop_page(&mut tab.doc, i, crop_box);
                        }
                        msg = format!("Cropped all {} pages", total);
                    } else {
                        let _ = crop_page(&mut tab.doc, target_page, crop_box);
                        msg = format!("Cropped page {}", target_page + 1);
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
            let mut create_sig = false;
            egui::Window::new("Add Digital Signature Field (FR-13.1)")
                .collapsible(false)
                .resizable(false)
                .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
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
                    ui.checkbox(&mut self.sig_is_visible, "Visible Signature Appearance Box (FR-13.10)");

                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        if ui.button("🔏 Sign & Place Field").clicked() {
                            create_sig = true;
                            close = true;
                        }
                        if ui.button("Cancel").clicked() {
                            close = true;
                        }
                    });
                });

            if create_sig {
                let target_page = self.sig_target_page;
                let field_name = self.sig_field_name.clone();
                let signer = self.sig_signer_name.clone();
                let reason = self.sig_reason.clone();
                let location = self.sig_location.clone();
                let is_visible = self.sig_is_visible;
                let mut status = None;

                if let Some(tab) = self.active_tab_mut() {
                    let curr = target_page.min(tab.doc.page_count().saturating_sub(1));
                    let rect = if is_visible {
                        Some([72.0, 72.0, 260.0, 130.0])
                    } else {
                        None
                    };

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
                                format!("Created digital signature field '{}' on page {}", field_name, curr + 1),
                                true,
                            ));
                        }
                        Err(e) => {
                            status = Some((format!("Failed to create signature: {}", e), false));
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
            egui::Window::new("Page Numbering & Labels (FR-06.9)")
                .collapsible(false)
                .resizable(false)
                .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.label("Configure ISO 32000-1 Section 8.3.1 page numbering schemes:");
                    ui.add_space(4.0);

                    ui.horizontal(|ui| {
                        ui.label("Starting at page:");
                        ui.add(egui::DragValue::new(&mut self.page_labels_start_page).range(1..=9999));
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
                                ui.selectable_value(&mut self.page_labels_style_idx, 0, "1, 2, 3, ... (Decimal)");
                                ui.selectable_value(&mut self.page_labels_style_idx, 1, "i, ii, iii, ... (Lowercase Roman)");
                                ui.selectable_value(&mut self.page_labels_style_idx, 2, "I, II, III, ... (Uppercase Roman)");
                                ui.selectable_value(&mut self.page_labels_style_idx, 3, "a, b, c, ... (Lowercase Letters)");
                                ui.selectable_value(&mut self.page_labels_style_idx, 4, "A, B, C, ... (Uppercase Letters)");
                                ui.selectable_value(&mut self.page_labels_style_idx, 5, "None (Prefix only)");
                            });
                    });

                    ui.horizontal(|ui| {
                        ui.label("Prefix:");
                        ui.text_edit_singleline(&mut self.page_labels_prefix);
                    });

                    ui.horizontal(|ui| {
                        ui.label("Start numbering at:");
                        ui.add(egui::DragValue::new(&mut self.page_labels_start_num).range(1..=9999));
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
            egui::Window::new("Add Hyperlink (FR-20.1)")
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
                            ui.add(egui::DragValue::new(&mut self.link_target_page).range(1..=9999));
                        });
                    }

                    ui.horizontal(|ui| {
                        ui.label("Border Width (0 = invisible hotspot):");
                        ui.add(egui::DragValue::new(&mut self.link_border_width).range(0.0..=5.0).speed(0.5));
                    });

                    ui.weak(format!("Region: [{:.1}, {:.1}, {:.1}, {:.1}] pt", self.link_rect[0], self.link_rect[1], self.link_rect[2], self.link_rect[3]));

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
                    if let AnnotationType::Link { border_width: ref mut bw, .. } = a.annot_type {
                        *bw = border_width;
                    }
                    a
                } else {
                    let mut a = Annotation::new_link_goto(rect, target_p);
                    if let AnnotationType::Link { border_width: ref mut bw, .. } = a.annot_type {
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
            egui::Window::new("Add Interactive Form Field (FR-11.1)")
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
                                2 => "▾ Dropdown Choice (Ch)",
                                _ => "🔘 Push Button (Btn)",
                            })
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut self.form_field_type_idx, 0, "📝 Text Field (Tx)");
                                ui.selectable_value(&mut self.form_field_type_idx, 1, "☑ Checkbox (Btn)");
                                ui.selectable_value(&mut self.form_field_type_idx, 2, "▾ Dropdown Choice (Ch)");
                                ui.selectable_value(&mut self.form_field_type_idx, 3, "🔘 Push Button (Btn)");
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
                                self.form_field_value = if checked { "Yes".into() } else { "Off".into() };
                            }
                        }
                        2 => {
                            ui.horizontal(|ui| {
                                ui.label("Options (comma-separated):");
                                ui.text_edit_singleline(&mut self.form_field_options_csv);
                            });
                            ui.horizontal(|ui| {
                                ui.label("Default Selected:");
                                ui.text_edit_singleline(&mut self.form_field_value);
                            });
                        }
                        _ => {
                            ui.horizontal(|ui| {
                                ui.label("Button Label:");
                                ui.text_edit_singleline(&mut self.form_field_value);
                            });
                        }
                    }

                    ui.add_space(4.0);
                    ui.weak(format!("Dimensions: [{:.1}, {:.1}, {:.1}, {:.1}] pt", self.form_field_rect[0], self.form_field_rect[1], self.form_field_rect[2], self.form_field_rect[3]));

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
                    _ => {
                        let label = if val_str.is_empty() { "Button" } else { &val_str };
                        FormField::new_pushbutton(name, rect, label)
                    }
                };

                let mut msg = None;
                if let Some(tab) = self.active_tab_mut() {
                    let curr = tab.current_page;
                    match add_form_field(&mut tab.doc, curr, &field) {
                        Ok(_) => {
                            tab.is_modified = true;
                            tab.invalidate_render();
                            msg = Some(format!("Created form field '{}' on page {}", field.name, curr + 1));
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

            egui::Window::new("Accessibility Checker & PDF/UA Remediation (FR-21)")
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
                            if ui.button("⚡ Remediate All Automatically (FR-21.3-10)").on_hover_text("Fix title, display preference, natural language, tags, and structure tab order in one click").clicked() {
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
                    self.status_message = "Completed accessibility remediation (FR-21)".into();
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

            egui::Window::new("Export Document (FR-03.2)")
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
                                if let Ok(html) = export_to_html(&tab.doc, page_indices.as_deref()) {
                                    if std::fs::write(&dest, html).is_ok() {
                                        self.status_message = format!("Exported HTML to '{}'", dest.display());
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
                                if let Ok(txt) = export_to_plain_text(&tab.doc, page_indices.as_deref()) {
                                    if std::fs::write(&dest, txt).is_ok() {
                                        self.status_message = format!("Exported text to '{}'", dest.display());
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
                            let ext = if self.export_format_idx == 2 { "png" } else { "jpg" };

                            if let Some(folder) = rfd::FileDialog::new().pick_folder() {
                                let targets: Vec<usize> = page_indices.unwrap_or_else(|| (0..total_pages).collect());
                                let mut count = 0;
                                for p in &targets {
                                    if let Ok(bytes) = export_page_to_image(&tab.doc, *p, self.export_dpi, fmt) {
                                        let out_path = folder.join(format!("page_{}.{}", p + 1, ext));
                                        if std::fs::write(&out_path, bytes).is_ok() {
                                            count += 1;
                                        }
                                    }
                                }
                                self.status_message = format!("Exported {} images to '{}'", count, folder.display());
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

            egui::Window::new("Comments & Review Summary (FR-31.5)")
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
                            if ui.button("📥 Export Comments as CSV (FR-31.5)").clicked() {
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
                        self.status_message = format!("Exported comments CSV to '{}'", dest.display());
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

            egui::Window::new("Merge Reviewer Comments (FR-31.3)")
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

            egui::Window::new("Measurement Scale & Calibration (FR-26.5)")
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
                        ui.label(RichText::new("Reference Calibration (FR-26.5)").strong());
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
                        ui.label(RichText::new("Snapping & Annotations (FR-26.4, FR-26.6)").strong());
                        ui.horizontal(|ui| {
                            ui.checkbox(&mut self.measure_snap.snap_endpoints, "Endpoints");
                            ui.checkbox(&mut self.measure_snap.snap_midpoints, "Midpoints");
                            ui.checkbox(&mut self.measure_snap.snap_intersections, "Intersections");
                        });
                        ui.add_space(2.0);
                        ui.checkbox(&mut self.measure_bake_as_annot, "Bake dimension lines into PDF as annotations (FR-26.4)");
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

            egui::Window::new("Create PDF Portfolio (FR-29.2.1)")
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
                                self.status_message = format!("Created PDF Portfolio '{}' with {} files", out_path.display(), items.len());
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

            egui::Window::new("Linearize for Fast Web View (FR-02.8, FR-25.8)")
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
                                    self.status_message = format!("Linearized PDF saved to '{}'", dest.display());
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
        self.render_menu_bar(ctx);
        self.render_toolbar(ctx);
        self.render_tab_bar(ctx);

        if self.search_bar_open && self.active_tab().is_some() {
            self.render_search_bar(ctx);
        }

        self.render_status_bar(ctx);
        self.render_sidebar(ctx);
        self.render_canvas(ctx);

        // 4. Render Modals (Watermark, Bates, Headers & Footers, Text Box)
        self.render_dialogs(ctx);
    }
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
        Box::new(|_cc| Ok(Box::new(initial_app))),
    )
}
