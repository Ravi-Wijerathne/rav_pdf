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
use ravpdf_annot::{add_annotation_to_page, get_page_annotations, Annotation, AnnotationType};
use ravpdf_core::document::{paper_sizes, PdfDocument};
use ravpdf_core::outline::{add_bookmark, Bookmark};
use ravpdf_edit::{
    create_pdf_from_images, create_pdf_from_text, delete_page, extract_pages, insert_blank_page,
    merge_documents, reorder_pages, rotate_page, split_document, ImagePdfOptions, TextPdfOptions,
};
use ravpdf_crypto::{encrypt_document, Permissions};
use ravpdf_forms::{
    flatten_form, get_page_form_fields, set_form_field_value, FormFieldType,
};
use ravpdf_render::{pixmap_to_png, render_page, RenderOptions};
use ravpdf_tools::{
    apply_bates_numbering, apply_headers_and_footers, apply_text_watermark, audit_space_usage,
    compare_documents, extract_all_text, optimize_document, redact_page_area, redact_pattern,
    search_document, set_document_info, ComparisonReport, HeaderFooterConfig, OptimizationStats,
    SearchMatch, SpaceAudit,
};
use std::path::PathBuf;
use tab::DocumentTab;

/// Action triggered on a page from the thumbnail sidebar (FR-01.2.3, FR-06).
#[derive(Debug, Clone, Copy)]
enum PageAction {
    Select(usize),
    Rotate(usize),
    Delete(usize),
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
    Properties,
    Search,
}

#[derive(Debug, PartialEq, Clone, Copy)]
enum ActiveTool {
    Select,
    Hand,
    StickyNote,
    Highlight,
    Rectangle,
    Text,
    Redact,
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

            new_bookmark_title: String::new(),

            drag_start_pos: None,
            drag_current_pos: None,
            show_free_text_dialog: false,
            free_text_content: String::new(),
            free_text_coords: None,

            form_field_text_buffers: std::collections::HashMap::new(),
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
                if let Ok(matches) = search_document(&tab.doc, &self.search_query, false, false) {
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

                // Go to page quick input box (FR-01.2.6)
                if has_doc {
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
                    ui.label(format!("of {}", total_pages));
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

                // Find in document trigger
                if ui.add_enabled(has_doc, Button::new("🔍 Find")).clicked() {
                    self.search_bar_open = !self.search_bar_open;
                }

                ui.separator();

                // Tool Selector Buttons (FR-08, FR-09)
                ui.add_enabled_ui(has_doc, |ui| {
                    ui.selectable_value(&mut self.active_tool, ActiveTool::Select, "Select");
                    ui.selectable_value(&mut self.active_tool, ActiveTool::Hand, "Pan");
                    ui.selectable_value(&mut self.active_tool, ActiveTool::StickyNote, "Note");
                    ui.selectable_value(&mut self.active_tool, ActiveTool::Highlight, "Highlight");
                    ui.selectable_value(&mut self.active_tool, ActiveTool::Rectangle, "Shape");
                    ui.selectable_value(&mut self.active_tool, ActiveTool::Text, "Text");
                    ui.selectable_value(&mut self.active_tool, ActiveTool::Redact, "Redact");
                });
            });
        });
    }

    /// Renders search bar panel (FR-34.1).
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

                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui.button("✕ Close").clicked() {
                        self.search_bar_open = false;
                        self.search_results.clear();
                    }
                });
            });
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
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut self.sidebar_tab, SidebarTab::Thumbnails, "Pages");
                    ui.selectable_value(&mut self.sidebar_tab, SidebarTab::Bookmarks, "Outline");
                    ui.selectable_value(&mut self.sidebar_tab, SidebarTab::Annotations, "Notes");
                    ui.selectable_value(&mut self.sidebar_tab, SidebarTab::Properties, "Info");
                    ui.selectable_value(&mut self.sidebar_tab, SidebarTab::Search, "Results");
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
                                                ui.label(
                                                    RichText::new(format!("{}", i + 1))
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
                                        _ => {}
                                    }
                                }
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
