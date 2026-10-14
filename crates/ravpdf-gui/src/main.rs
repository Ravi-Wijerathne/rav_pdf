//! RavPDF Desktop GUI Application.
//!
//! Traceability:
//! - FR-01 (PDF Viewing and Navigation: continuous/fit modes, keyboard navigation shortcuts)
//! - FR-01.2.1 (Page navigation: Page Up/Down, Home, End, Arrows)
//! - FR-01.2.6 (Go to Page)
//! - FR-01.3.2 (Fit to Page, Fit to Width, 100% actual size)
//! - FR-34.1 (Find & Search Ctrl+F with match navigation)
//! - EI-01.1 - EI-01.9 (Main menu bar, toolbar, canvas, side panels, status bar)
//! - NFR-05.1 (Usability, modern UI conventions)

use eframe::egui::{
    self, Align, Align2, Button, Color32, FontId, Key, Layout, Pos2, Rect, RichText, ScrollArea,
    Stroke, TextureOptions, Vec2,
};
use ravpdf_annot::{add_annotation_to_page, get_page_annotations, Annotation, AnnotationType};
use ravpdf_core::document::{paper_sizes, PdfDocument};
use ravpdf_edit::{delete_page, insert_blank_page, rotate_page};
use ravpdf_render::{render_page, Color, RenderOptions};
use ravpdf_tools::{
    apply_bates_numbering, apply_text_watermark, get_document_info, search_document, DocumentInfo,
    SearchMatch,
};
use std::path::PathBuf;

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
}

struct RavPdfApp {
    doc: Option<PdfDocument>,
    file_path: Option<PathBuf>,
    current_page: usize,
    page_input_buffer: String,
    zoom: f32,
    rotation: i64,
    sidebar_tab: SidebarTab,
    sidebar_open: bool,
    active_tool: ActiveTool,
    page_texture: Option<egui::TextureHandle>,
    cached_page_index: Option<usize>,
    cached_zoom: Option<f32>,
    cached_rotation: Option<i64>,
    status_message: String,
    metadata: Option<DocumentInfo>,

    // Search bar state (FR-34)
    search_bar_open: bool,
    search_query: String,
    search_results: Vec<SearchMatch>,
    current_search_idx: usize,

    // Dialog modals
    show_watermark_dialog: bool,
    watermark_text: String,
    watermark_opacity: f32,
    show_bates_dialog: bool,
    bates_prefix: String,
    bates_start: u32,
    bates_digits: usize,
}

impl Default for RavPdfApp {
    fn default() -> Self {
        Self {
            doc: None,
            file_path: None,
            current_page: 0,
            page_input_buffer: "1".into(),
            zoom: 1.0,
            rotation: 0,
            sidebar_tab: SidebarTab::Thumbnails,
            sidebar_open: true,
            active_tool: ActiveTool::Select,
            page_texture: None,
            cached_page_index: None,
            cached_zoom: None,
            cached_rotation: None,
            status_message: "Ready • No document open".into(),
            metadata: None,
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
        }
    }
}

impl eframe::App for RavPdfApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // 1. Handle Drag-and-Drop files onto the window (EI-01.7)
        let dropped_files = ctx.input(|i| i.raw.dropped_files.clone());
        for dropped in dropped_files {
            if let Some(path) = dropped.path {
                if path
                    .extension()
                    .map_or(false, |ext| ext.eq_ignore_ascii_case("pdf"))
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
        if self.search_bar_open && self.doc.is_some() {
            self.render_search_bar(ctx);
        }
        self.render_status_bar(ctx);
        self.render_sidebar(ctx);
        self.render_canvas(ctx);

        // 4. Render Modals (Watermark, Bates)
        self.render_dialogs(ctx);
    }
}

impl RavPdfApp {
    /// Handles keyboard shortcuts for rapid navigation and productivity.
    fn handle_keyboard_shortcuts(&mut self, ctx: &egui::Context) {
        let has_doc = self.doc.is_some();
        let total_pages = self.doc.as_ref().map(|d| d.page_count()).unwrap_or(0);

        ctx.input(|i| {
            // Open / New / Save
            if i.modifiers.ctrl && i.key_pressed(Key::O) {
                self.open_file_dialog();
            }
            if i.modifiers.ctrl && i.key_pressed(Key::N) {
                self.create_new_blank_document();
            }
            if i.modifiers.ctrl && i.key_pressed(Key::S) && has_doc {
                if let Some(ref path) = self.file_path.clone() {
                    if let Some(ref doc) = self.doc {
                        let _ = doc.save_to_file(path);
                        self.status_message = format!("Saved: {}", path.display());
                    }
                } else {
                    self.save_file_dialog();
                }
            }
            if i.modifiers.ctrl && i.key_pressed(Key::W) && has_doc {
                self.close_document();
            }

            // Find (Ctrl+F)
            if i.modifiers.ctrl && i.key_pressed(Key::F) && has_doc {
                self.search_bar_open = !self.search_bar_open;
            }

            // Zoom shortcuts
            if i.modifiers.ctrl && (i.key_pressed(Key::Plus) || i.key_pressed(Key::Equals)) && has_doc {
                self.zoom = (self.zoom * 1.25).min(64.0);
                self.invalidate_render();
            }
            if i.modifiers.ctrl && i.key_pressed(Key::Minus) && has_doc {
                self.zoom = (self.zoom / 1.25).max(0.1);
                self.invalidate_render();
            }
            if i.modifiers.ctrl && i.key_pressed(Key::Num0) && has_doc {
                self.fit_to_page(ctx);
            }
            if i.modifiers.ctrl && i.key_pressed(Key::Num9) && has_doc {
                self.fit_to_width(ctx);
            }

            // Navigation shortcuts (FR-01.2.1)
            if has_doc && total_pages > 0 {
                if i.key_pressed(Key::PageDown) || i.key_pressed(Key::ArrowRight) {
                    if self.current_page + 1 < total_pages {
                        self.current_page += 1;
                        self.page_input_buffer = (self.current_page + 1).to_string();
                        self.invalidate_render();
                    }
                }
                if i.key_pressed(Key::PageUp) || i.key_pressed(Key::ArrowLeft) {
                    if self.current_page > 0 {
                        self.current_page -= 1;
                        self.page_input_buffer = (self.current_page + 1).to_string();
                        self.invalidate_render();
                    }
                }
                if i.key_pressed(Key::Home) {
                    self.current_page = 0;
                    self.page_input_buffer = "1".into();
                    self.invalidate_render();
                }
                if i.key_pressed(Key::End) {
                    self.current_page = total_pages.saturating_sub(1);
                    self.page_input_buffer = (self.current_page + 1).to_string();
                    self.invalidate_render();
                }
            }
        });
    }

    /// Fits page to window width (FR-01.3.2).
    fn fit_to_width(&mut self, ctx: &egui::Context) {
        if let Some(ref doc) = self.doc {
            if let Some(page) = doc.get_page(self.current_page) {
                let page_w = (page.crop_box[2] - page.crop_box[0]).abs() as f32;
                let avail_w = ctx.screen_rect().width() - (if self.sidebar_open { 240.0 } else { 40.0 });
                if page_w > 0.0 && avail_w > 0.0 {
                    self.zoom = (avail_w / page_w).clamp(0.2, 5.0);
                    self.invalidate_render();
                    self.status_message = format!("Fit to width ({:.0}%)", self.zoom * 100.0);
                }
            }
        }
    }

    /// Fits entire page into view (FR-01.3.2).
    fn fit_to_page(&mut self, ctx: &egui::Context) {
        if let Some(ref doc) = self.doc {
            if let Some(page) = doc.get_page(self.current_page) {
                let page_w = (page.crop_box[2] - page.crop_box[0]).abs() as f32;
                let page_h = (page.crop_box[3] - page.crop_box[1]).abs() as f32;
                let avail_w = ctx.screen_rect().width() - (if self.sidebar_open { 260.0 } else { 60.0 });
                let avail_h = ctx.screen_rect().height() - 140.0;

                if page_w > 0.0 && page_h > 0.0 && avail_w > 0.0 && avail_h > 0.0 {
                    let zoom_w = avail_w / page_w;
                    let zoom_h = avail_h / page_h;
                    self.zoom = zoom_w.min(zoom_h).clamp(0.2, 5.0);
                    self.invalidate_render();
                    self.status_message = format!("Fit to page ({:.0}%)", self.zoom * 100.0);
                }
            }
        }
    }

    /// Opens a PDF document from a path on disk.
    fn open_file(&mut self, path: PathBuf) {
        match PdfDocument::load_from_file(&path) {
            Ok(doc) => {
                let name = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("Document");
                self.status_message = format!("Opened: {}", name);
                self.metadata = get_document_info(&doc).ok();
                self.doc = Some(doc);
                self.file_path = Some(path);
                self.current_page = 0;
                self.page_input_buffer = "1".into();
                self.zoom = 1.0;
                self.rotation = 0;
                self.search_results.clear();
                self.current_search_idx = 0;
                self.invalidate_render();
            }
            Err(err) => {
                self.status_message = format!("Failed to open file: {}", err);
            }
        }
    }

    /// Spawns the native file picker to open a PDF.
    fn open_file_dialog(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("PDF Documents (*.pdf)", &["pdf"])
            .pick_file()
        {
            self.open_file(path);
        }
    }

    /// Spawns the native file picker to save the current document.
    fn save_file_dialog(&mut self) {
        if let Some(ref doc) = self.doc {
            let mut dialog = rfd::FileDialog::new().add_filter("PDF Documents (*.pdf)", &["pdf"]);
            if let Some(ref current_path) = self.file_path {
                if let Some(name) = current_path.file_name().and_then(|n| n.to_str()) {
                    dialog = dialog.set_file_name(name);
                }
            } else {
                dialog = dialog.set_file_name("document.pdf");
            }

            if let Some(path) = dialog.save_file() {
                match doc.save_to_file(&path) {
                    Ok(()) => {
                        self.file_path = Some(path.clone());
                        self.status_message = format!("Saved: {}", path.display());
                    }
                    Err(e) => {
                        self.status_message = format!("Save failed: {}", e);
                    }
                }
            }
        }
    }

    /// Closes the currently active document.
    fn close_document(&mut self) {
        self.doc = None;
        self.file_path = None;
        self.current_page = 0;
        self.page_input_buffer = "1".into();
        self.metadata = None;
        self.search_results.clear();
        self.current_search_idx = 0;
        self.search_bar_open = false;
        self.invalidate_render();
        self.status_message = "Ready • No document open".into();
    }

    /// Creates a brand new empty PDF document with one blank Letter page.
    fn create_new_blank_document(&mut self) {
        let mut doc = PdfDocument::new();
        doc.add_blank_page(paper_sizes::LETTER.0, paper_sizes::LETTER.1);
        self.doc = Some(doc);
        self.file_path = None;
        self.current_page = 0;
        self.page_input_buffer = "1".into();
        self.zoom = 1.0;
        self.rotation = 0;
        self.metadata = None;
        self.search_results.clear();
        self.invalidate_render();
        self.status_message = "New blank document created (Letter 612x792 pt)".into();
    }

    /// Executes search across document.
    fn execute_search(&mut self) {
        if let Some(ref doc) = self.doc {
            if !self.search_query.trim().is_empty() {
                match search_document(doc, &self.search_query, false, false) {
                    Ok(results) => {
                        self.search_results = results;
                        self.current_search_idx = 0;
                        if let Some(first_match) = self.search_results.first() {
                            self.current_page = first_match.page_index;
                            self.page_input_buffer = (self.current_page + 1).to_string();
                            self.invalidate_render();
                            self.status_message = format!(
                                "Found {} matches for '{}'",
                                self.search_results.len(),
                                self.search_query
                            );
                        } else {
                            self.status_message = format!("No matches found for '{}'", self.search_query);
                        }
                    }
                    Err(e) => {
                        self.status_message = format!("Search error: {}", e);
                    }
                }
            }
        }
    }

    fn render_menu_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Open Document... (Ctrl+O)").clicked() {
                        self.open_file_dialog();
                        ui.close_menu();
                    }
                    if ui.button("New Blank Document (Ctrl+N)").clicked() {
                        self.create_new_blank_document();
                        ui.close_menu();
                    }

                    ui.separator();

                    let has_doc = self.doc.is_some();
                    if ui
                        .add_enabled(has_doc, Button::new("Save (Ctrl+S)"))
                        .clicked()
                    {
                        if let Some(ref path) = self.file_path.clone() {
                            if let Some(ref doc) = self.doc {
                                let _ = doc.save_to_file(path);
                                self.status_message = format!("Saved: {}", path.display());
                            }
                        } else {
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
                    if ui
                        .add_enabled(has_doc, Button::new("Close Document (Ctrl+W)"))
                        .clicked()
                    {
                        self.close_document();
                        ui.close_menu();
                    }

                    ui.separator();
                    if ui.button("Exit").clicked() {
                        std::process::exit(0);
                    }
                });

                let has_doc = self.doc.is_some();

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
                        if let Some(ref mut doc) = self.doc {
                            let _ = insert_blank_page(
                                doc,
                                self.current_page + 1,
                                paper_sizes::LETTER.0,
                                paper_sizes::LETTER.1,
                            );
                            self.current_page += 1;
                            self.page_input_buffer = (self.current_page + 1).to_string();
                            self.invalidate_render();
                            self.status_message = "Inserted new page".into();
                        }
                        ui.close_menu();
                    }
                    if ui
                        .add_enabled(has_doc, Button::new("Delete Current Page"))
                        .clicked()
                    {
                        if let Some(ref mut doc) = self.doc {
                            if doc.page_count() > 1 {
                                let _ = delete_page(doc, self.current_page);
                                if self.current_page >= doc.page_count() {
                                    self.current_page = doc.page_count().saturating_sub(1);
                                }
                                self.page_input_buffer = (self.current_page + 1).to_string();
                                self.invalidate_render();
                                self.status_message = "Deleted current page".into();
                            }
                        }
                        ui.close_menu();
                    }
                });

                ui.menu_button("View", |ui| {
                    if ui
                        .add_enabled(has_doc, Button::new("Fit to Page (Ctrl+0)"))
                        .clicked()
                    {
                        self.fit_to_page(ctx);
                        ui.close_menu();
                    }
                    if ui
                        .add_enabled(has_doc, Button::new("Fit to Width (Ctrl+9)"))
                        .clicked()
                    {
                        self.fit_to_width(ctx);
                        ui.close_menu();
                    }
                    if ui
                        .add_enabled(has_doc, Button::new("Actual Size (100%)"))
                        .clicked()
                    {
                        self.zoom = 1.0;
                        self.invalidate_render();
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui
                        .add_enabled(has_doc, Button::new("Rotate Clockwise 90°"))
                        .clicked()
                    {
                        if let Some(ref mut doc) = self.doc {
                            let _ = rotate_page(doc, self.current_page, 90);
                            self.invalidate_render();
                        }
                        ui.close_menu();
                    }
                });

                ui.menu_button("Tools", |ui| {
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
                    ui.separator();
                    if ui
                        .add_enabled(has_doc, Button::new("Sticky Note Tool"))
                        .clicked()
                    {
                        self.active_tool = ActiveTool::StickyNote;
                        ui.close_menu();
                    }
                    if ui
                        .add_enabled(has_doc, Button::new("Highlight Tool"))
                        .clicked()
                    {
                        self.active_tool = ActiveTool::Highlight;
                        ui.close_menu();
                    }
                });

                ui.menu_button("Help", |ui| {
                    if ui.button("About RavPDF").clicked() {
                        self.status_message =
                            "RavPDF 1.0 — High Performance Rust PDF Editor (ISO 32000 Conformance)".into();
                        ui.close_menu();
                    }
                });
            });
        });
    }

    fn render_toolbar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("toolbar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui.button("📂 Open").clicked() {
                    self.open_file_dialog();
                }

                let has_doc = self.doc.is_some();

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

                let total_pages = self.doc.as_ref().map(|d| d.page_count()).unwrap_or(0);
                let can_prev = has_doc && self.current_page > 0;
                let can_next = has_doc && self.current_page + 1 < total_pages;

                if ui.add_enabled(can_prev, Button::new("⏮")).clicked() {
                    self.current_page = 0;
                    self.page_input_buffer = "1".into();
                    self.invalidate_render();
                }
                if ui.add_enabled(can_prev, Button::new("◀")).clicked() {
                    self.current_page -= 1;
                    self.page_input_buffer = (self.current_page + 1).to_string();
                    self.invalidate_render();
                }

                // Go to page quick input box (FR-01.2.6)
                if has_doc {
                    ui.label("Page");
                    let text_edit = ui.add(
                        egui::TextEdit::singleline(&mut self.page_input_buffer)
                            .desired_width(32.0),
                    );
                    if text_edit.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                        if let Ok(p) = self.page_input_buffer.trim().parse::<usize>() {
                            if p >= 1 && p <= total_pages {
                                self.current_page = p - 1;
                                self.invalidate_render();
                            } else {
                                self.page_input_buffer = (self.current_page + 1).to_string();
                            }
                        }
                    }
                    ui.label(format!("of {}", total_pages));
                } else {
                    ui.label("Page 0 of 0");
                }

                if ui.add_enabled(can_next, Button::new("▶")).clicked() {
                    self.current_page += 1;
                    self.page_input_buffer = (self.current_page + 1).to_string();
                    self.invalidate_render();
                }
                if ui.add_enabled(can_next, Button::new("⏭")).clicked() {
                    self.current_page = total_pages.saturating_sub(1);
                    self.page_input_buffer = (self.current_page + 1).to_string();
                    self.invalidate_render();
                }

                ui.separator();

                // Zoom controls & fit modes (FR-01.3.2)
                if ui.add_enabled(has_doc, Button::new("➖")).clicked() {
                    self.zoom = (self.zoom / 1.2).max(0.1);
                    self.invalidate_render();
                }
                ui.label(format!("{:.0}%", self.zoom * 100.0));
                if ui.add_enabled(has_doc, Button::new("➕")).clicked() {
                    self.zoom = (self.zoom * 1.2).min(64.0);
                    self.invalidate_render();
                }
                if ui.add_enabled(has_doc, Button::new("Fit Page")).clicked() {
                    self.fit_to_page(ctx);
                }
                if ui.add_enabled(has_doc, Button::new("Fit Width")).clicked() {
                    self.fit_to_width(ctx);
                }

                ui.separator();

                if ui.add_enabled(has_doc, Button::new("⟳ Rotate")).clicked() {
                    if let Some(ref mut doc) = self.doc {
                        let _ = rotate_page(doc, self.current_page, 90);
                        self.invalidate_render();
                    }
                }

                ui.separator();

                // Find in document trigger
                if ui.add_enabled(has_doc, Button::new("🔍 Find")).clicked() {
                    self.search_bar_open = !self.search_bar_open;
                }

                ui.separator();

                // Tool Selector Buttons
                ui.add_enabled_ui(has_doc, |ui| {
                    ui.selectable_value(&mut self.active_tool, ActiveTool::Select, "Select");
                    ui.selectable_value(&mut self.active_tool, ActiveTool::Hand, "Pan");
                    ui.selectable_value(&mut self.active_tool, ActiveTool::StickyNote, "Note");
                    ui.selectable_value(&mut self.active_tool, ActiveTool::Highlight, "Highlight");
                    ui.selectable_value(&mut self.active_tool, ActiveTool::Rectangle, "Shape");
                    ui.selectable_value(&mut self.active_tool, ActiveTool::Text, "Text");
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
                        self.current_page = self.search_results[self.current_search_idx].page_index;
                        self.page_input_buffer = (self.current_page + 1).to_string();
                        self.invalidate_render();
                    }
                    if ui.button("▼ Next").clicked() && self.current_search_idx + 1 < self.search_results.len() {
                        self.current_search_idx += 1;
                        self.current_page = self.search_results[self.current_search_idx].page_index;
                        self.page_input_buffer = (self.current_page + 1).to_string();
                        self.invalidate_render();
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

    fn render_sidebar(&mut self, ctx: &egui::Context) {
        if !self.sidebar_open {
            return;
        }

        egui::SidePanel::left("left_sidebar")
            .default_width(220.0)
            .min_width(180.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut self.sidebar_tab, SidebarTab::Thumbnails, "Pages");
                    ui.selectable_value(&mut self.sidebar_tab, SidebarTab::Bookmarks, "Outline");
                    ui.selectable_value(&mut self.sidebar_tab, SidebarTab::Annotations, "Notes");
                    ui.selectable_value(&mut self.sidebar_tab, SidebarTab::Properties, "Info");
                    ui.selectable_value(&mut self.sidebar_tab, SidebarTab::Search, "Results");
                });

                ui.separator();

                if self.doc.is_none() {
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
                        ScrollArea::vertical().show(ui, |ui| {
                            let total = self.doc.as_ref().map(|d| d.page_count()).unwrap_or(0);
                            for i in 0..total {
                                let is_selected = self.current_page == i;
                                ui.horizontal(|ui| {
                                    if ui
                                        .selectable_label(is_selected, format!("Page {}", i + 1))
                                        .clicked()
                                    {
                                        self.current_page = i;
                                        self.page_input_buffer = (self.current_page + 1).to_string();
                                        self.invalidate_render();
                                    }
                                });
                            }
                        });
                    }
                    SidebarTab::Bookmarks => {
                        ui.label("Document Outlines");
                        ui.weak("(No bookmarks defined)");
                    }
                    SidebarTab::Annotations => {
                        ui.label("Annotations on current page:");
                        if let Some(ref doc) = self.doc {
                            if let Ok(annots) = get_page_annotations(doc, self.current_page) {
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
                        ui.label("Document Metadata:");
                        if let Some(ref info) = self.metadata {
                            ui.label(format!("Title: {}", info.title.as_deref().unwrap_or("None")));
                            ui.label(format!(
                                "Author: {}",
                                info.author.as_deref().unwrap_or("None")
                            ));
                            ui.label(format!(
                                "Creator: {}",
                                info.creator.as_deref().unwrap_or("None")
                            ));
                            ui.label(format!(
                                "Producer: {}",
                                info.producer.as_deref().unwrap_or("None")
                            ));
                        } else {
                            ui.weak("No metadata available");
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
                            self.current_page = p_idx;
                            self.page_input_buffer = (self.current_page + 1).to_string();
                            self.invalidate_render();
                        }
                    }
                }
            });
    }

    fn render_status_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(&self.status_message);
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if let Some(ref doc) = self.doc {
                        ui.label(format!("Zoom: {:.0}%", self.zoom * 100.0));
                        ui.separator();
                        if let Some(p) = doc.get_page(self.current_page) {
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

    fn render_canvas(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            if self.doc.is_none() {
                // Professional Welcome / Empty State View
                self.render_welcome_screen(ui);
                return;
            }

            self.ensure_page_rendered(ctx);

            ScrollArea::both()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    if let Some(ref tex) = self.page_texture {
                        let size = tex.size_vec2();
                        let (rect, response) =
                            ui.allocate_exact_size(size, egui::Sense::click_and_drag());

                        // Draw page shadow and white background
                        let painter = ui.painter();
                        painter.rect_filled(
                            rect.expand(2.0),
                            0.0,
                            Color32::from_black_alpha(40),
                        );
                        painter.rect_filled(rect, 0.0, Color32::WHITE);

                        // Paint page image texture
                        painter.image(
                            tex.id(),
                            rect,
                            Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                            Color32::WHITE,
                        );

                        // Handle interactions
                        if response.clicked() {
                            if let Some(pos) = response.interact_pointer_pos() {
                                let local_x = (pos.x - rect.min.x) / self.zoom;
                                let local_y = (rect.max.y - pos.y) / self.zoom; // Invert to PDF space

                                if self.active_tool == ActiveTool::StickyNote {
                                    if let Some(ref mut doc) = self.doc {
                                        let annot = Annotation::new(
                                            AnnotationType::Text,
                                            [
                                                local_x as f64,
                                                local_y as f64,
                                                (local_x + 30.0) as f64,
                                                (local_y + 30.0) as f64,
                                            ],
                                        );
                                        let _ = add_annotation_to_page(
                                            doc,
                                            self.current_page,
                                            &annot,
                                        );
                                        self.status_message = "Added sticky note".into();
                                        self.invalidate_render();
                                    }
                                }
                            }
                        }
                    }
                });
        });
    }

    /// Renders a modern, rich welcome screen when no document is loaded.
    fn render_welcome_screen(&mut self, ui: &mut egui::Ui) {
        ui.vertical_centered(|ui| {
            ui.add_space(80.0);

            // Title
            ui.heading(
                RichText::new("RavPDF")
                    .size(36.0)
                    .strong()
                    .color(Color32::from_rgb(60, 130, 246)),
            );
            ui.label(
                RichText::new("Professional, Memory-Safe Rust PDF Editor")
                    .size(16.0)
                    .color(Color32::GRAY),
            );

            ui.add_space(40.0);

            // Primary Action Buttons
            ui.horizontal(|ui| {
                ui.add_space((ui.available_width() - 360.0).max(0.0) * 0.5);

                let open_btn = Button::new(RichText::new("📂  Open PDF File").size(15.0).strong())
                    .min_size(Vec2::new(170.0, 48.0));
                if ui.add(open_btn).clicked() {
                    self.open_file_dialog();
                }

                ui.add_space(16.0);

                let new_btn = Button::new(RichText::new("📄  New Blank PDF").size(15.0))
                    .min_size(Vec2::new(170.0, 48.0));
                if ui.add(new_btn).clicked() {
                    self.create_new_blank_document();
                }
            });

            ui.add_space(40.0);

            // Drag and drop target box
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

    /// Renders modal dialogs for tools like Watermarking and Bates Numbering.
    fn render_dialogs(&mut self, ctx: &egui::Context) {
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
                        ui.label("Opacity (0.0 - 1.0):");
                        ui.add(egui::Slider::new(&mut self.watermark_opacity, 0.05..=1.0));
                    });
                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        if ui.button("Apply Watermark").clicked() {
                            if let Some(ref mut doc) = self.doc {
                                let _ = apply_text_watermark(
                                    doc,
                                    &self.watermark_text,
                                    self.watermark_opacity,
                                    45.0,
                                    54.0,
                                );
                                self.status_message = "Applied watermark to document".into();
                                self.invalidate_render();
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
                            if let Some(ref mut doc) = self.doc {
                                let _ = apply_bates_numbering(
                                    doc,
                                    &self.bates_prefix,
                                    self.bates_start,
                                    self.bates_digits,
                                    "",
                                );
                                self.status_message = "Applied Bates numbering".into();
                                self.invalidate_render();
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
    }

    fn ensure_page_rendered(&mut self, ctx: &egui::Context) {
        let needs_render = self.page_texture.is_none()
            || self.cached_page_index != Some(self.current_page)
            || self.cached_zoom != Some(self.zoom)
            || self.cached_rotation != Some(self.rotation);

        if needs_render {
            if let Some(ref doc) = self.doc {
                if self.current_page < doc.page_count() {
                    let options = RenderOptions {
                        zoom: self.zoom,
                        rotation: self.rotation,
                        background_color: Color::WHITE,
                    };

                    if let Ok(pixmap) = render_page(doc, self.current_page, &options) {
                        let width = pixmap.width() as usize;
                        let height = pixmap.height() as usize;
                        let pixels: Vec<Color32> = pixmap
                            .data()
                            .chunks_exact(4)
                            .map(|rgba| {
                                Color32::from_rgba_unmultiplied(
                                    rgba[0], rgba[1], rgba[2], rgba[3],
                                )
                            })
                            .collect();

                        let color_image = egui::ColorImage {
                            size: [width, height],
                            pixels,
                        };

                        let tex = ctx.load_texture(
                            "page_view",
                            color_image,
                            TextureOptions::LINEAR,
                        );
                        self.page_texture = Some(tex);
                        self.cached_page_index = Some(self.current_page);
                        self.cached_zoom = Some(self.zoom);
                        self.cached_rotation = Some(self.rotation);
                    }
                }
            }
        }
    }

    fn invalidate_render(&mut self) {
        self.page_texture = None;
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
            .with_title("RavPDF — Rust PDF Editor")
            .with_inner_size([1200.0, 800.0])
            .with_min_inner_size([800.0, 600.0]),
        ..Default::default()
    };

    eframe::run_native(
        "RavPDF",
        options,
        Box::new(|_cc| Ok(Box::new(initial_app))),
    )
}
