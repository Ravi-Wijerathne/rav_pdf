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
use ravpdf_tools::{
    apply_plate_separations, apply_soft_proof, get_document_info, DocumentInfo,
    PlateSeparationConfig, SeparationAnalysis, SoftProofConfig,
};
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;

/// Represents an Optional Content Group (OCG) / Layer conforming to ISO 32000-1 Section 8.11 (EI-01.4, FR-35.5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PdfLayer {
    pub id: String,
    pub name: String,
    pub visible: bool,
}

/// Represents an individual opened PDF document in a tab.
pub struct DocumentTab {
    pub id: usize,
    pub title: String,
    pub doc: PdfDocument,
    pub file_path: Option<PathBuf>,
    pub current_page: usize,
    pub page_input_buffer: String,
    pub zoom: f32,
    pub target_zoom: f32,
    pub rotation: i64,
    pub is_modified: bool,
    pub metadata: Option<DocumentInfo>,
    pub bookmarks: Vec<Bookmark>,
    pub page_texture: Option<egui::TextureHandle>,
    pub thumbnail_textures: HashMap<usize, egui::TextureHandle>,
    pub continuous_page_textures: HashMap<usize, egui::TextureHandle>,
    pub continuous_page_order: VecDeque<usize>,
    pub page_dimensions: Vec<(f32, f32)>,
    pub cached_page_index: Option<usize>,
    pub cached_zoom: Option<f32>,
    pub cached_render_zoom: Option<f32>,
    pub cached_continuous_render_zoom: Option<f32>,
    pub cached_rotation: Option<i64>,
    pub cached_continuous_rotation: Option<i64>,
    pub cached_plate_config: Option<PlateSeparationConfig>,
    pub cached_soft_proof: Option<SoftProofConfig>,
    pub raw_file_bytes: Option<Vec<u8>>,
    pub hidden_ocgs: HashSet<String>,
}

impl DocumentTab {
    /// Computes dimensions (width, height in pt) for every page in the document in a single pass.
    pub fn compute_page_dimensions(doc: &PdfDocument) -> Vec<(f32, f32)> {
        let page_ids = doc.page_ids();
        let mut dims = Vec::with_capacity(page_ids.len());
        for &id in &page_ids {
            if let Some(dict) = doc.get_object(id).and_then(|o| o.as_dict()) {
                let media_box = doc
                    .resolve_box(dict, "MediaBox")
                    .unwrap_or([0.0, 0.0, 612.0, 792.0]);
                let crop_box = doc.resolve_box(dict, "CropBox").unwrap_or(media_box);
                let mut w = (crop_box[2] - crop_box[0]).abs() as f32;
                let mut h = (crop_box[3] - crop_box[1]).abs() as f32;
                let rot = doc.resolve_i64(dict, "Rotate").unwrap_or(0).rem_euclid(360);
                if rot == 90 || rot == 270 {
                    std::mem::swap(&mut w, &mut h);
                }
                dims.push((w, h));
            } else {
                dims.push((612.0, 792.0));
            }
        }
        dims
    }

    /// Fast O(1) dimension lookup for any page in points, taking view display rotation into account.
    pub fn get_page_dim(&self, page_idx: usize) -> (f32, f32) {
        let (mut w, mut h) = self.page_dimensions.get(page_idx).copied().unwrap_or((612.0, 792.0));
        let view_rot = self.rotation.rem_euclid(360);
        if view_rot == 90 || view_rot == 270 {
            std::mem::swap(&mut w, &mut h);
        }
        (w, h)
    }

    /// Creates a new document tab with the provided document.
    pub fn new(id: usize, title: String, doc: PdfDocument, file_path: Option<PathBuf>) -> Self {
        let bookmarks = get_document_bookmarks(&doc).unwrap_or_default();
        let metadata = get_document_info(&doc).ok();
        let page_dimensions = Self::compute_page_dimensions(&doc);

        Self {
            id,
            title,
            doc,
            file_path,
            current_page: 0,
            page_input_buffer: "1".into(),
            zoom: 1.0,
            target_zoom: 1.0,
            rotation: 0,
            is_modified: false,
            metadata,
            bookmarks,
            page_texture: None,
            thumbnail_textures: HashMap::new(),
            continuous_page_textures: HashMap::new(),
            continuous_page_order: VecDeque::new(),
            page_dimensions,
            cached_page_index: None,
            cached_zoom: None,
            cached_render_zoom: None,
            cached_continuous_render_zoom: None,
            cached_rotation: None,
            cached_continuous_rotation: None,
            cached_plate_config: None,
            cached_soft_proof: None,
            raw_file_bytes: None,
            hidden_ocgs: HashSet::new(),
        }
    }

    /// Invalidates rendered textures so they are re-rasterized on the next frame.
    pub fn invalidate_render(&mut self) {
        self.page_texture = None;
        self.thumbnail_textures.clear();
        self.continuous_page_textures.clear();
        self.continuous_page_order.clear();
        self.cached_zoom = None;
        self.cached_render_zoom = None;
        self.cached_continuous_render_zoom = None;
        self.cached_continuous_rotation = None;
        self.page_dimensions = Self::compute_page_dimensions(&self.doc);
    }

    /// Invalidates only the single-page texture when navigating pages (preserving continuous & thumbnail caches).
    pub fn invalidate_page_texture(&mut self) {
        self.page_texture = None;
    }

    /// Retrieves an existing page texture or rasterizes it on demand for continuous scrolling with bounded LRU cache (FR-01.2.2, NFR-01.3, NFR-01.8).
    pub fn get_or_render_continuous_page(
        &mut self,
        ctx: &egui::Context,
        page_idx: usize,
    ) -> Option<egui::TextureHandle> {
        let ppp = ctx.pixels_per_point();
        let render_zoom = self.zoom * ppp;

        // If zoom or rotation changed, invalidate continuous cache so pages render at the new resolution
        if self.cached_continuous_render_zoom != Some(render_zoom)
            || self.cached_continuous_rotation != Some(self.rotation)
        {
            self.continuous_page_textures.clear();
            self.continuous_page_order.clear();
            self.cached_continuous_render_zoom = Some(render_zoom);
            self.cached_continuous_rotation = Some(self.rotation);
        }

        if let Some(tex) = self.continuous_page_textures.get(&page_idx) {
            if let Some(pos) = self.continuous_page_order.iter().position(|&p| p == page_idx) {
                self.continuous_page_order.remove(pos);
            }
            self.continuous_page_order.push_back(page_idx);
            return Some(tex.clone());
        }
        if page_idx >= self.doc.page_count() {
            return None;
        }

        // Bounded LRU cache eviction (NFR-01.8: max 32 cached continuous pages)
        const MAX_CACHED_PAGES: usize = 32;
        while self.continuous_page_order.len() >= MAX_CACHED_PAGES {
            if let Some(oldest) = self.continuous_page_order.pop_front() {
                self.continuous_page_textures.remove(&oldest);
            }
        }

        let options = RenderOptions {
            zoom: render_zoom,
            rotation: self.rotation,
            background_color: Color::WHITE,
            hidden_ocgs: self.hidden_ocgs.clone(),
        };

        if let Ok(pixmap) = render_page(&self.doc, page_idx, &options) {
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
                format!(
                    "cont_tab_{}_{}_z{:.2}_p{:.1}_r{}",
                    self.id, page_idx, self.zoom, ppp, self.rotation
                ),
                color_image,
                TextureOptions::LINEAR,
            );
            self.continuous_page_textures.insert(page_idx, tex.clone());
            self.continuous_page_order.push_back(page_idx);
            Some(tex)
        } else {
            None
        }
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
    #[allow(dead_code)]
    pub fn ensure_page_rendered(&mut self, ctx: &egui::Context) {
        let _ = self.ensure_page_rendered_with_plate(ctx, None, None);
    }

    /// Ensures that the current page texture is rendered, optionally applying CMYK plate separations, TAC warnings (FR-23.2), and Soft-Proofing simulation (FR-24.5).
    pub fn ensure_page_rendered_with_plate(
        &mut self,
        ctx: &egui::Context,
        plate_config: Option<&PlateSeparationConfig>,
        soft_proof_config: Option<&SoftProofConfig>,
    ) -> Option<SeparationAnalysis> {
        let ppp = ctx.pixels_per_point();
        let render_zoom = self.zoom * ppp;
        let needs_render = self.page_texture.is_none()
            || self.cached_page_index != Some(self.current_page)
            || self.cached_render_zoom != Some(render_zoom)
            || self.cached_rotation != Some(self.rotation)
            || self.cached_plate_config.as_ref() != plate_config
            || self.cached_soft_proof.as_ref() != soft_proof_config;

        let mut analysis = None;

        if needs_render && self.current_page < self.doc.page_count() {
            let options = RenderOptions {
                zoom: render_zoom,
                rotation: self.rotation,
                background_color: Color::WHITE,
                hidden_ocgs: self.hidden_ocgs.clone(),
            };

            if let Ok(pixmap) = render_page(&self.doc, self.current_page, &options) {
                let width = pixmap.width() as usize;
                let height = pixmap.height() as usize;

                let (mut processed_data, ana) = if let Some(cfg) = plate_config {
                    let (data, a) = apply_plate_separations(
                        pixmap.data(),
                        pixmap.width(),
                        pixmap.height(),
                        cfg,
                    );
                    (data, Some(a))
                } else {
                    (pixmap.data().to_vec(), None)
                };
                analysis = ana;

                // Apply soft-proofing simulation if enabled (FR-24.5)
                if let Some(sp_cfg) = soft_proof_config {
                    processed_data =
                        apply_soft_proof(&processed_data, width as u32, height as u32, sp_cfg);
                }

                let pixels: Vec<Color32> = processed_data
                    .chunks_exact(4)
                    .map(|rgba| Color32::from_rgba_unmultiplied(rgba[0], rgba[1], rgba[2], rgba[3]))
                    .collect();

                let color_image = egui::ColorImage {
                    size: [width, height],
                    pixels,
                };

                let tex = ctx.load_texture(
                    format!("page_view_tab_{}_{}_p{:.1}", self.id, self.current_page, ppp),
                    color_image,
                    TextureOptions::LINEAR,
                );
                self.page_texture = Some(tex);
                self.cached_page_index = Some(self.current_page);
                self.cached_zoom = Some(self.zoom);
                self.cached_render_zoom = Some(render_zoom);
                self.cached_rotation = Some(self.rotation);
                self.cached_plate_config = plate_config.cloned();
                self.cached_soft_proof = soft_proof_config.cloned();
            }
        }

        analysis
    }

    /// Fits current page to the view width (FR-01.3.2).
    pub fn fit_to_width(&mut self, avail_w: f32) {
        if let Some(page) = self.doc.get_page(self.current_page) {
            let page_w = (page.crop_box[2] - page.crop_box[0]).abs() as f32;
            if page_w > 0.0 && avail_w > 0.0 {
                self.zoom = (avail_w / page_w).clamp(0.2, 5.0);
                self.target_zoom = self.zoom;
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
                self.target_zoom = self.zoom;
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

    /// Extracts Optional Content Groups (OCGs) / Layers defined in the document catalog (EI-01.4, FR-35.5).
    pub fn get_document_layers(&self) -> Vec<PdfLayer> {
        let catalog = match self.doc.catalog() {
            Some(c) => c,
            None => return Vec::new(),
        };

        let oc_props = match catalog.get("OCProperties").and_then(|obj| {
            match obj {
                ravpdf_core::object::Object::Dictionary(d) => Some(d.clone()),
                ravpdf_core::object::Object::Reference(id) => {
                    self.doc.get_object(*id).and_then(|o| o.as_dict().cloned())
                }
                _ => None,
            }
        }) {
            Some(p) => p,
            None => return Vec::new(),
        };

        // Extract default OFF state from /D dictionary
        let mut default_off_ids = HashSet::new();
        if let Some(d_obj) = oc_props.get("D") {
            let d_dict = match d_obj {
                ravpdf_core::object::Object::Dictionary(d) => Some(d),
                ravpdf_core::object::Object::Reference(id) => self.doc.get_object(*id).and_then(|o| o.as_dict()),
                _ => None,
            };
            if let Some(d) = d_dict {
                if let Some(off_arr) = d.get_array("OFF") {
                    for off_item in off_arr {
                        if let ravpdf_core::object::Object::Reference(id) = off_item {
                            default_off_ids.insert(format!("{}_{}", id.number, id.generation));
                        }
                    }
                }
            }
        }

        let mut layers = Vec::new();
        if let Some(ocgs_arr) = oc_props.get_array("OCGs") {
            for ocg_ref in ocgs_arr {
                let (ocg_id_str, ocg_dict) = match ocg_ref {
                    ravpdf_core::object::Object::Reference(id) => {
                        let id_str = format!("{}_{}", id.number, id.generation);
                        let dict = self.doc.get_object(*id).and_then(|o| o.as_dict().cloned());
                        (id_str, dict)
                    }
                    ravpdf_core::object::Object::Dictionary(d) => {
                        ("inline_ocg".to_string(), Some(d.clone()))
                    }
                    _ => continue,
                };

                if let Some(dict) = ocg_dict {
                    let name = dict.get_str("Name").unwrap_or_else(|| "Unnamed Layer".to_string());
                    let is_default_off = default_off_ids.contains(&ocg_id_str);
                    let is_hidden_by_user = self.hidden_ocgs.contains(&ocg_id_str) || self.hidden_ocgs.contains(&name);
                    let visible = !is_hidden_by_user && !is_default_off;

                    layers.push(PdfLayer {
                        id: ocg_id_str,
                        name,
                        visible,
                    });
                }
            }
        }

        layers
    }

    /// Toggles the visibility of a layer by its ID and Name (EI-01.4).
    pub fn toggle_layer_visibility(&mut self, layer_id: &str, layer_name: &str) {
        let is_hidden = self.hidden_ocgs.contains(layer_id) || self.hidden_ocgs.contains(layer_name);
        if is_hidden {
            self.hidden_ocgs.remove(layer_id);
            self.hidden_ocgs.remove(layer_name);
        } else {
            self.hidden_ocgs.insert(layer_id.to_string());
            self.hidden_ocgs.insert(layer_name.to_string());
        }
        self.invalidate_render();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ravpdf_core::object::{Object, PdfDictionary, PdfName, PdfString};

    #[test]
    fn test_layer_parsing_and_toggle() {
        let mut doc = PdfDocument::new();
        doc.add_blank_page(612.0, 792.0);

        // Add 2 OCGs to document
        let ocg1_id = doc.next_object_id();
        let mut ocg1_dict = PdfDictionary::new();
        ocg1_dict.insert("Type", PdfName::new("OCG"));
        ocg1_dict.insert("Name", PdfString::from_str("Architectural Plans"));
        doc.objects.insert(ocg1_id, Object::Dictionary(ocg1_dict));

        let ocg2_id = doc.next_object_id();
        let mut ocg2_dict = PdfDictionary::new();
        ocg2_dict.insert("Type", PdfName::new("OCG"));
        ocg2_dict.insert("Name", PdfString::from_str("Electrical Wiring"));
        doc.objects.insert(ocg2_id, Object::Dictionary(ocg2_dict));

        // Add OCProperties to Catalog
        let mut oc_props = PdfDictionary::new();
        oc_props.insert(
            "OCGs",
            Object::Array(vec![
                Object::Reference(ocg1_id),
                Object::Reference(ocg2_id),
            ]),
        );

        let mut d_dict = PdfDictionary::new();
        d_dict.insert("ON", Object::Array(vec![Object::Reference(ocg1_id)]));
        d_dict.insert("OFF", Object::Array(vec![Object::Reference(ocg2_id)]));
        oc_props.insert("D", Object::Dictionary(d_dict));

        let catalog_id = doc.root_id;
        if let Some(Object::Dictionary(ref mut cat)) = doc.objects.get_mut(&catalog_id) {
            cat.insert("OCProperties", Object::Dictionary(oc_props));
        }

        let mut tab = DocumentTab::new(1, "Test Doc".to_string(), doc, None);
        let layers = tab.get_document_layers();
        assert_eq!(layers.len(), 2);
        assert_eq!(layers[0].name, "Architectural Plans");
        assert!(layers[0].visible);
        assert_eq!(layers[1].name, "Electrical Wiring");
        assert!(!layers[1].visible); // Default OFF

        // Toggle first layer to hidden
        tab.toggle_layer_visibility(&layers[0].id, &layers[0].name);
        let updated = tab.get_document_layers();
        assert!(!updated[0].visible);

        // Toggle back to visible
        tab.toggle_layer_visibility(&layers[0].id, &layers[0].name);
        let updated2 = tab.get_document_layers();
        assert!(updated2[0].visible);
    }

    #[test]
    fn test_bounded_lru_texture_order() {
        let mut order = VecDeque::new();
        const MAX_PAGES: usize = 5;

        for page in 0..10 {
            if order.len() >= MAX_PAGES {
                order.pop_front();
            }
            order.push_back(page);
        }

        assert_eq!(order.len(), MAX_PAGES);
        assert_eq!(order.into_iter().collect::<Vec<_>>(), vec![5, 6, 7, 8, 9]);
    }
}
