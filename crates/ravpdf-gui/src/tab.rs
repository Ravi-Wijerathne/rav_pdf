//! Document Tab Model conforming to ISO 32000-1 and FR-01.4 (Multi-Tab Interface).
//!
//! Traceability:
//! - FR-01.4.1 (Multiple PDF documents open in separate tabs)
//! - FR-01.4.3 (Display document title or filename in each tab)
//! - FR-01.3.2 (Fit to page, fit to width)
//! - FR-19.1 (Bookmarks outline model)

use eframe::egui::{self, Color32, TextureOptions};
use ravpdf_core::document::PdfDocument;
use ravpdf_core::outline::{get_document_bookmarks, Bookmark};
use ravpdf_render::{render_page, Color, RenderOptions};
use ravpdf_tools::{get_document_info, DocumentInfo};
use std::path::PathBuf;

/// Represents an individual opened PDF document in a tab.
pub struct DocumentTab {
    pub id: usize,
    pub title: String,
    pub doc: PdfDocument,
    pub file_path: Option<PathBuf>,
    pub current_page: usize,
    pub page_input_buffer: String,
    pub zoom: f32,
    pub rotation: i64,
    pub is_modified: bool,
    pub metadata: Option<DocumentInfo>,
    pub bookmarks: Vec<Bookmark>,
    pub page_texture: Option<egui::TextureHandle>,
    pub thumbnail_textures: std::collections::HashMap<usize, egui::TextureHandle>,
    pub cached_page_index: Option<usize>,
    pub cached_zoom: Option<f32>,
    pub cached_rotation: Option<i64>,
}

impl DocumentTab {
    /// Creates a new document tab with the provided document.
    pub fn new(id: usize, title: String, doc: PdfDocument, file_path: Option<PathBuf>) -> Self {
        let bookmarks = get_document_bookmarks(&doc).unwrap_or_default();
        let metadata = get_document_info(&doc).ok();

        Self {
            id,
            title,
            doc,
            file_path,
            current_page: 0,
            page_input_buffer: "1".into(),
            zoom: 1.0,
            rotation: 0,
            is_modified: false,
            metadata,
            bookmarks,
            page_texture: None,
            thumbnail_textures: std::collections::HashMap::new(),
            cached_page_index: None,
            cached_zoom: None,
            cached_rotation: None,
        }
    }

    /// Invalidates rendered textures so they are re-rasterized on the next frame.
    pub fn invalidate_render(&mut self) {
        self.page_texture = None;
        self.thumbnail_textures.clear();
    }

    /// Retrieves an existing thumbnail texture or rasterizes it on demand (FR-01.2.3).
    pub fn get_or_render_thumbnail(
        &mut self,
        ctx: &egui::Context,
        page_idx: usize,
    ) -> Option<egui::TextureHandle> {
        if let Some(tex) = self.thumbnail_textures.get(&page_idx) {
            return Some(tex.clone());
        }
        if page_idx >= self.doc.page_count() {
            return None;
        }

        if let Ok(pixmap) = ravpdf_render::render_thumbnail(&self.doc, page_idx, 160) {
            let width = pixmap.width() as usize;
            let height = pixmap.height() as usize;
            let pixels: Vec<Color32> = pixmap
                .data()
                .chunks_exact(4)
                .map(|rgba| Color32::from_rgba_unmultiplied(rgba[0], rgba[1], rgba[2], rgba[3]))
                .collect();

            let color_image = egui::ColorImage {
                size: [width, height],
                pixels,
            };

            let tex = ctx.load_texture(
                format!("thumb_tab_{}_{}", self.id, page_idx),
                color_image,
                TextureOptions::LINEAR,
            );
            self.thumbnail_textures.insert(page_idx, tex.clone());
            Some(tex)
        } else {
            None
        }
    }

    /// Refreshes bookmarks from the underlying PDF document.
    pub fn reload_bookmarks(&mut self) {
        self.bookmarks = get_document_bookmarks(&self.doc).unwrap_or_default();
    }

    /// Refreshes document metadata.
    #[allow(dead_code)]
    pub fn reload_metadata(&mut self) {
        self.metadata = get_document_info(&self.doc).ok();
    }

    /// Ensures that the current page raster texture is up to date in the egui context.
    pub fn ensure_page_rendered(&mut self, ctx: &egui::Context) {
        let needs_render = self.page_texture.is_none()
            || self.cached_page_index != Some(self.current_page)
            || self.cached_zoom != Some(self.zoom)
            || self.cached_rotation != Some(self.rotation);

        if needs_render && self.current_page < self.doc.page_count() {
            let options = RenderOptions {
                zoom: self.zoom,
                rotation: self.rotation,
                background_color: Color::WHITE,
            };

            if let Ok(pixmap) = render_page(&self.doc, self.current_page, &options) {
                let width = pixmap.width() as usize;
                let height = pixmap.height() as usize;
                let pixels: Vec<Color32> = pixmap
                    .data()
                    .chunks_exact(4)
                    .map(|rgba| Color32::from_rgba_unmultiplied(rgba[0], rgba[1], rgba[2], rgba[3]))
                    .collect();

                let color_image = egui::ColorImage {
                    size: [width, height],
                    pixels,
                };

                let tex = ctx.load_texture(
                    format!("page_view_tab_{}_{}", self.id, self.current_page),
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

    /// Fits current page to the view width (FR-01.3.2).
    pub fn fit_to_width(&mut self, avail_w: f32) {
        if let Some(page) = self.doc.get_page(self.current_page) {
            let page_w = (page.crop_box[2] - page.crop_box[0]).abs() as f32;
            if page_w > 0.0 && avail_w > 0.0 {
                self.zoom = (avail_w / page_w).clamp(0.2, 5.0);
                self.invalidate_render();
            }
        }
    }

    /// Fits current page completely within the available canvas area (FR-01.3.2).
    pub fn fit_to_page(&mut self, avail_w: f32, avail_h: f32) {
        if let Some(page) = self.doc.get_page(self.current_page) {
            let page_w = (page.crop_box[2] - page.crop_box[0]).abs() as f32;
            let page_h = (page.crop_box[3] - page.crop_box[1]).abs() as f32;

            if page_w > 0.0 && page_h > 0.0 && avail_w > 0.0 && avail_h > 0.0 {
                let zoom_w = avail_w / page_w;
                let zoom_h = avail_h / page_h;
                self.zoom = zoom_w.min(zoom_h).clamp(0.2, 5.0);
                self.invalidate_render();
            }
        }
    }

    /// Saves document to its currently associated file path on disk.
    pub fn save(&mut self) -> Result<(), ravpdf_core::error::PdfError> {
        if let Some(ref path) = self.file_path {
            self.doc.save_to_file(path)?;
            self.is_modified = false;
            Ok(())
        } else {
            Err(ravpdf_core::error::PdfError::InvalidStructure(
                "No file path specified for tab".into(),
            ))
        }
    }
}
