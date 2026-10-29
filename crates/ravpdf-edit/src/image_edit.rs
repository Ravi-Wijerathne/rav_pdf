//! Advanced PDF Image Editing, Transform Pipeline, and Layout Management conforming to ISO 32000-1.
//!
//! Traceability:
//! - FR-05.1 (Insert images: JPEG, PNG, TIFF, BMP, GIF, WebP)
//! - FR-05.2 (Replace existing image with new file or stream)
//! - FR-05.3 (Delete selected image from page)
//! - FR-05.4 (Resize image with optional aspect ratio lock)
//! - FR-05.5 (Rotate image by arbitrary angles 0-360 degrees)
//! - FR-05.6 (Crop image using vector clipping path)
//! - FR-05.7 (Flip image horizontally and vertically)
//! - FR-05.8 (Reposition image via translation)
//! - FR-05.9 (Alignment guides and snapping to margins and sibling objects)
//! - FR-05.10 (Adjust image opacity/transparency via ExtGState)
//! - FR-05.11 (Z-order management: bring to front, send to back, move forward, move backward)
//! - FR-05.12 (Preserve image resolution and metadata during editing)

use ravpdf_core::document::PdfDocument;
use ravpdf_core::error::{PdfError, Result};
use ravpdf_core::lexer::{Lexer, Token};
use ravpdf_core::object::{Object, ObjectId, PdfDictionary, PdfName};
use std::path::{Path, PathBuf};

fn token_as_f64(tok: &Token) -> Option<f64> {
    match tok {
        Token::Integer(i) => Some(*i as f64),
        Token::Real(r) => Some(*r),
        _ => None,
    }
}

/// Extracted image metadata and spatial placement on a page.
///
/// Conforms to ISO 32000-1 Clause 8.9 (Images) & Clause 8.3 (Coordinate Systems). Traceability: FR-05.
#[derive(Debug, Clone, PartialEq)]
pub struct PageImage {
    /// The XObject resource name (e.g. "Im1", "Image1")
    pub name: String,
    /// Indirect object ID of the image XObject
    pub obj_id: ObjectId,
    /// Pixel width from `/Width`
    pub pixel_width: u32,
    /// Pixel height from `/Height`
    pub pixel_height: u32,
    /// Color space (e.g. "DeviceRGB", "DeviceGray", "DeviceCMYK")
    pub color_space: String,
    /// Bits per component (typically 8)
    pub bits_per_component: u8,
    /// Stream compression filter (e.g. "FlateDecode", "DCTDecode")
    pub filter: Option<String>,
    /// Active transformation matrix `[a, b, c, d, e, f]`
    pub matrix: [f64; 6],
    /// Display X coordinate (origin bottom-left) in PDF points
    pub display_x: f64,
    /// Display Y coordinate in PDF points
    pub display_y: f64,
    /// Rendered width in PDF points
    pub display_width: f64,
    /// Rendered height in PDF points
    pub display_height: f64,
    /// Rotation in degrees `[0.0, 360.0)`
    pub rotation_deg: f64,
    /// Whether image has horizontal reflection / flip
    pub flipped_h: bool,
    /// Whether image has vertical reflection / flip
    pub flipped_v: bool,
    /// Active alpha opacity `[0.0, 1.0]`
    pub opacity: f32,
    /// Optional vector clipping rectangle `[x, y, w, h]`
    pub clip_rect: Option<[f64; 4]>,
}

/// Options for transforming an image on a page.
///
/// Traceability: FR-05.4, FR-05.5, FR-05.7, FR-05.8.
#[derive(Debug, Clone, Default)]
pub struct ImageTransformOptions {
    /// New translation coordinates `(x, y)` in PDF points. If None, preserves existing position.
    pub position: Option<(f64, f64)>,
    /// New size `(width, height)` in PDF points. If None, preserves existing dimensions.
    pub size: Option<(f64, f64)>,
    /// Rotation angle in degrees `[0.0, 360.0)`. If None, preserves existing rotation.
    pub rotation_deg: Option<f64>,
    /// Flip horizontally.
    pub flip_h: bool,
    /// Flip vertically.
    pub flip_v: bool,
    /// Lock aspect ratio when resizing.
    pub lock_aspect_ratio: bool,
}

/// Z-order management actions for reordering page image content.
///
/// Traceability: FR-05.11.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZOrderAction {
    BringToFront,
    SendToBack,
    MoveForward,
    MoveBackward,
}

/// Orientation of a magnetic alignment guide line.
///
/// Traceability: FR-05.9.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapGuideOrientation {
    Vertical,
    Horizontal,
}

/// Alignment guide line details for magnetic canvas snapping.
///
/// Traceability: FR-05.9.
#[derive(Debug, Clone, PartialEq)]
pub struct SnapGuide {
    pub orientation: SnapGuideOrientation,
    /// Coordinate in PDF points (X for Vertical, Y for Horizontal)
    pub coordinate: f64,
    /// Human-readable label (e.g. "Center", "Left Margin", "Sibling Top")
    pub label: String,
}

/// Snapped position result with active magnetic guides.
///
/// Traceability: FR-05.9.
#[derive(Debug, Clone, PartialEq)]
pub struct SnappedPosition {
    pub snapped_x: f64,
    pub snapped_y: f64,
    pub active_guides: Vec<SnapGuide>,
}

/// Extracts all image XObjects and their active spatial placement and properties from a page.
///
/// Conforms to ISO 32000-1 Clause 8.9 and 8.3. Traceability: FR-05.1 - FR-05.12.
pub fn extract_page_images(doc: &PdfDocument, page_index: usize) -> Result<Vec<PageImage>> {
    let page = doc
        .get_page(page_index)
        .ok_or_else(|| PdfError::InvalidStructure(format!("Page {} out of range", page_index)))?;

    // Gather all XObjects registered in page resources
    let mut xobjects_map: std::collections::HashMap<String, ObjectId> =
        std::collections::HashMap::new();
    if let Some(ref res_dict) = page.resources {
        if let Some(Object::Dictionary(xobj_dict)) = res_dict.get("XObject") {
            for (name, obj) in &xobj_dict.map {
                if let Object::Reference(id) = obj {
                    xobjects_map.insert(name.as_str().to_string(), *id);
                }
            }
        }
    }

    if xobjects_map.is_empty() {
        return Ok(Vec::new());
    }

    // Inspect content stream tokens to find Do invocations and their preceding matrix and graphics states
    let content_bytes = doc.get_page_content(page_index).unwrap_or_default();
    let mut lexer = Lexer::new(&content_bytes);
    let mut tokens: Vec<Token> = Vec::new();
    while let Ok(tok) = lexer.next_token() {
        if tok == Token::Eof {
            break;
        }
        tokens.push(tok);
    }

    let mut images = Vec::new();

    // Stack to track graphics state matrices and opacity
    #[derive(Clone)]
    struct GState {
        ctm: [f64; 6],
        opacity: f32,
        clip_rect: Option<[f64; 4]>,
    }

    let identity = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];
    let mut cur_state = GState {
        ctm: identity,
        opacity: 1.0,
        clip_rect: None,
    };
    let mut state_stack: Vec<GState> = Vec::new();
    let mut token_idx = 0;

    while token_idx < tokens.len() {
        match &tokens[token_idx] {
            Token::Keyword(op) if op == "q" => {
                state_stack.push(cur_state.clone());
            }
            Token::Keyword(op) if op == "Q" => {
                if let Some(prev) = state_stack.pop() {
                    cur_state = prev;
                }
            }
            Token::Keyword(op) if op == "cm" => {
                // Preceding 6 tokens are [a, b, c, d, e, f]
                if token_idx >= 6 {
                    let a = token_as_f64(&tokens[token_idx - 6]).unwrap_or(1.0);
                    let b = token_as_f64(&tokens[token_idx - 5]).unwrap_or(0.0);
                    let c = token_as_f64(&tokens[token_idx - 4]).unwrap_or(0.0);
                    let d = token_as_f64(&tokens[token_idx - 3]).unwrap_or(1.0);
                    let e = token_as_f64(&tokens[token_idx - 2]).unwrap_or(0.0);
                    let f = token_as_f64(&tokens[token_idx - 1]).unwrap_or(0.0);
                    let new_m = [a, b, c, d, e, f];
                    cur_state.ctm = concat_matrices(&cur_state.ctm, &new_m);
                }
            }
            Token::Keyword(op) if op == "gs" => {
                // Preceding token is ExtGState resource name
                if token_idx >= 1 {
                    if let Token::Name(gs_name) = &tokens[token_idx - 1] {
                        if let Some(alpha) = resolve_extgstate_opacity(page.resources.as_ref(), gs_name, doc) {
                            cur_state.opacity = alpha;
                        }
                    }
                }
            }
            Token::Keyword(op) if op == "re" => {
                // Vector rectangle: x y w h re
                if token_idx >= 4 {
                    let rx = token_as_f64(&tokens[token_idx - 4]).unwrap_or(0.0);
                    let ry = token_as_f64(&tokens[token_idx - 3]).unwrap_or(0.0);
                    let rw = token_as_f64(&tokens[token_idx - 2]).unwrap_or(0.0);
                    let rh = token_as_f64(&tokens[token_idx - 1]).unwrap_or(0.0);
                    cur_state.clip_rect = Some([rx, ry, rw, rh]);
                }
            }
            Token::Keyword(op) if op == "Do" && token_idx >= 1 => {
                if let Token::Name(name) = &tokens[token_idx - 1] {
                        if let Some(&obj_id) = xobjects_map.get(name) {
                            if let Some(Object::Stream(stream)) = doc.objects.get(&obj_id) {
                                let subtype = stream.dict.get_name("Subtype").unwrap_or("");
                                if subtype == "Image" {
                                    let pixel_w = stream.dict.get_i64("Width").unwrap_or(0).max(0) as u32;
                                    let pixel_h = stream.dict.get_i64("Height").unwrap_or(0).max(0) as u32;
                                    let cs = stream
                                        .dict
                                        .get_name("ColorSpace")
                                        .unwrap_or("DeviceRGB")
                                        .to_string();
                                    let bpc = stream.dict.get_i64("BitsPerComponent").unwrap_or(8) as u8;
                                    let filter = stream.dict.get_name("Filter").map(|s| s.to_string());

                                    let m = cur_state.ctm;
                                    let a = m[0];
                                    let b = m[1];
                                    let c = m[2];
                                    let d = m[3];
                                    let e = m[4];
                                    let f = m[5];

                                    let sx = (a * a + b * b).sqrt();
                                    let sy = (c * c + d * d).sqrt();
                                    let det = a * d - b * c;
                                    let flipped_h = det < 0.0 && (a < 0.0 || (a.abs() < 1e-6 && b < 0.0));
                                    let flipped_v = det < 0.0 && (d < 0.0 || (d.abs() < 1e-6 && c < 0.0));

                                    let (base_a, base_b) = if flipped_h { (-a, -b) } else { (a, b) };
                                    let rot_rad = base_b.atan2(base_a);
                                    let rot_deg = rot_rad.to_degrees().rem_euclid(360.0);

                                    images.push(PageImage {
                                        name: name.clone(),
                                        obj_id,
                                        pixel_width: pixel_w,
                                        pixel_height: pixel_h,
                                        color_space: cs,
                                        bits_per_component: bpc,
                                        filter,
                                        matrix: m,
                                        display_x: e,
                                        display_y: f,
                                        display_width: sx,
                                        display_height: sy,
                                        rotation_deg: rot_deg,
                                        flipped_h,
                                        flipped_v,
                                        opacity: cur_state.opacity,
                                        clip_rect: cur_state.clip_rect,
                                    });
                                }
                            }
                        }
                    }
                }
            _ => {}
        }
        token_idx += 1;
    }

    Ok(images)
}

/// Replaces an existing image stream with new raw pixel data conforming to ISO 32000-1 Clause 8.9.
///
/// Traceability: FR-05.2, FR-05.12.
pub fn replace_page_image(
    doc: &mut PdfDocument,
    page_index: usize,
    image_name: &str,
    new_data: Vec<u8>,
    width: u32,
    height: u32,
    color_space: &str,
) -> Result<()> {
    let page = doc
        .get_page(page_index)
        .ok_or_else(|| PdfError::InvalidStructure(format!("Page {} out of range", page_index)))?;

    let res_dict = page
        .resources
        .as_ref()
        .ok_or_else(|| PdfError::InvalidStructure("Page has no resources dictionary".into()))?;

    let xobjs = match res_dict.get("XObject") {
        Some(Object::Dictionary(d)) => d,
        _ => return Err(PdfError::InvalidStructure("Page has no XObject resources".into())),
    };

    let obj_id = match xobjs.get(image_name) {
        Some(Object::Reference(id)) => *id,
        _ => {
            return Err(PdfError::InvalidStructure(format!(
                "Image '{}' not found in page XObjects",
                image_name
            )))
        }
    };

    let img_obj = doc
        .objects
        .get_mut(&obj_id)
        .ok_or(PdfError::ObjectNotFound(obj_id.number, obj_id.generation))?;

    if let Object::Stream(stream) = img_obj {
        stream.dict.insert("Type", PdfName::new("XObject"));
        stream.dict.insert("Subtype", PdfName::new("Image"));
        stream.dict.insert("Width", width as i64);
        stream.dict.insert("Height", height as i64);
        stream.dict.insert("ColorSpace", PdfName::new(color_space));
        stream.dict.insert("BitsPerComponent", 8);
        stream.dict.map.remove(&PdfName::new("Filter"));

        stream.raw_data = new_data;
        let _ = stream.compress_flate();
        stream.update_length();
        Ok(())
    } else {
        Err(PdfError::InvalidStructure(
            "Target image object is not a stream".into(),
        ))
    }
}

/// Replaces an existing image on a page with an image loaded from a file path.
///
/// Supports PNG, JPEG, TIFF, BMP, GIF, and WebP. Traceability: FR-05.1, FR-05.2.
pub fn replace_page_image_from_file(
    doc: &mut PdfDocument,
    page_index: usize,
    image_name: &str,
    file_path: &Path,
) -> Result<()> {
    let img = image::open(file_path).map_err(|e| {
        PdfError::InvalidStructure(format!(
            "Failed to open image '{}': {}",
            file_path.display(),
            e
        ))
    })?;

    let rgb = img.to_rgb8();
    let w = rgb.width();
    let h = rgb.height();

    replace_page_image(
        doc,
        page_index,
        image_name,
        rgb.into_raw(),
        w,
        h,
        "DeviceRGB",
    )
}

/// Extracts the image data for an indirect image XObject and saves it to a file.
///
/// Conforms to ISO 32000-1 Clause 8.9. Traceability: FR-05.9, FR-37.2.
pub fn extract_image_to_file(
    doc: &PdfDocument,
    obj_id: ObjectId,
    output_path: &Path,
) -> Result<PathBuf> {
    let stream = doc
        .objects
        .get(&obj_id)
        .and_then(Object::as_stream)
        .ok_or_else(|| {
            PdfError::InvalidStructure(format!("Image object {:?} is not a stream", obj_id))
        })?;

    let filter = stream.dict.get_name("Filter");
    let width = stream.dict.get_i64("Width").unwrap_or(0) as u32;
    let height = stream.dict.get_i64("Height").unwrap_or(0) as u32;
    let color_space = stream.dict.get_name("ColorSpace").unwrap_or("DeviceRGB");

    if filter == Some("DCTDecode") {
        // Raw stream data is JPEG
        let data = &stream.raw_data;
        std::fs::write(output_path, data)
            .map_err(PdfError::Io)?;
        return Ok(output_path.to_path_buf());
    }

    let decoded = stream.decode()?;

    if width > 0 && height > 0 {
        if color_space == "DeviceRGB" && decoded.len() >= (width * height * 3) as usize {
            if let Some(img) = image::RgbImage::from_raw(width, height, decoded[..(width * height * 3) as usize].to_vec()) {
                img.save(output_path).map_err(|e| PdfError::InvalidStructure(format!("Save image failed: {}", e)))?;
                return Ok(output_path.to_path_buf());
            }
        } else if color_space == "DeviceGray" && decoded.len() >= (width * height) as usize {
            if let Some(img) = image::GrayImage::from_raw(width, height, decoded[..(width * height) as usize].to_vec()) {
                img.save(output_path).map_err(|e| PdfError::InvalidStructure(format!("Save image failed: {}", e)))?;
                return Ok(output_path.to_path_buf());
            }
        } else if color_space == "DeviceCMYK" && decoded.len() >= (width * height * 4) as usize {
            // Convert CMYK to RGB
            let mut rgb_bytes = Vec::with_capacity((width * height * 3) as usize);
            for chunk in decoded.chunks_exact(4) {
                let c = chunk[0] as f32 / 255.0;
                let m = chunk[1] as f32 / 255.0;
                let y = chunk[2] as f32 / 255.0;
                let k = chunk[3] as f32 / 255.0;
                let r = ((1.0 - c) * (1.0 - k) * 255.0).clamp(0.0, 255.0) as u8;
                let g = ((1.0 - m) * (1.0 - k) * 255.0).clamp(0.0, 255.0) as u8;
                let b = ((1.0 - y) * (1.0 - k) * 255.0).clamp(0.0, 255.0) as u8;
                rgb_bytes.push(r);
                rgb_bytes.push(g);
                rgb_bytes.push(b);
            }
            if let Some(img) = image::RgbImage::from_raw(width, height, rgb_bytes) {
                img.save(output_path).map_err(|e| PdfError::InvalidStructure(format!("Save image failed: {}", e)))?;
                return Ok(output_path.to_path_buf());
            }
        }
    }

    // Fallback: write decoded raw bytes
    std::fs::write(output_path, &decoded)
        .map_err(PdfError::Io)?;
    Ok(output_path.to_path_buf())
}

/// Extracts all image XObjects across specified page (or whole document) and writes them to an output directory.
///
/// Conforms to ISO 32000-1 Clause 8.9. Traceability: FR-05.9, FR-37.2.
pub fn extract_all_images_to_dir(
    doc: &PdfDocument,
    page_index: Option<usize>,
    output_dir: &Path,
) -> Result<Vec<PathBuf>> {
    std::fs::create_dir_all(output_dir)
        .map_err(PdfError::Io)?;

    let pages: Vec<usize> = if let Some(p) = page_index {
        vec![p]
    } else {
        (0..doc.page_count()).collect()
    };

    let mut saved_paths = Vec::new();
    let mut seen_ids = std::collections::HashSet::new();

    for p in pages {
        if let Ok(images) = extract_page_images(doc, p) {
            for (idx, im) in images.iter().enumerate() {
                if seen_ids.insert(im.obj_id) {
                    let ext = if im.filter.as_deref() == Some("DCTDecode") { "jpg" } else { "png" };
                    let clean_name = im.name.trim_start_matches('/').replace('/', "_");
                    let filename = format!("page_{}_img_{}_{}.{}", p + 1, idx + 1, clean_name, ext);
                    let out_path = output_dir.join(filename);
                    if let Ok(path) = extract_image_to_file(doc, im.obj_id, &out_path) {
                        saved_paths.push(path);
                    }
                }
            }
        }
    }

    Ok(saved_paths)
}

/// Deletes an image invocation from the page's content stream and optionally removes it from `/Resources`.
///
/// Conforms to ISO 32000-1 Clause 8.9. Traceability: FR-05.3.
pub fn delete_page_image(
    doc: &mut PdfDocument,
    page_index: usize,
    image_name: &str,
) -> Result<()> {
    let page = doc
        .get_page(page_index)
        .ok_or_else(|| PdfError::InvalidStructure(format!("Page {} out of range", page_index)))?;

    let contents_ids = page.contents_ids;
    let target_do = format!("/{} Do", image_name);

    for content_id in contents_ids {
        if let Some(Object::Stream(ref mut stream)) = doc.objects.get_mut(&content_id) {
            let decoded = stream.decode()?;
            let new_bytes = remove_image_drawing_block(&decoded, &target_do);
            if new_bytes.len() != decoded.len() {
                stream.raw_data = new_bytes;
                let _ = stream.compress_flate();
                stream.update_length();
            }
        }
    }

    Ok(())
}

/// Transforms an existing image on a page (position, size, rotation, flip).
///
/// Conforms to ISO 32000-1 Clause 8.3 (Coordinate Transformations). Traceability: FR-05.4, FR-05.5, FR-05.7, FR-05.8.
pub fn transform_page_image(
    doc: &mut PdfDocument,
    page_index: usize,
    image_name: &str,
    options: &ImageTransformOptions,
) -> Result<()> {
    let existing_images = extract_page_images(doc, page_index)?;
    let target = existing_images
        .iter()
        .find(|im| im.name == image_name)
        .ok_or_else(|| {
            PdfError::InvalidStructure(format!("Image '{}' not found on page", image_name))
        })?;

    let new_pos = options.position.unwrap_or((target.display_x, target.display_y));
    let mut new_size = options.size.unwrap_or((target.display_width, target.display_height));

    if options.lock_aspect_ratio && target.display_width > 0.0 && target.display_height > 0.0 {
        let orig_aspect = target.display_width / target.display_height;
        if (new_size.0 - target.display_width).abs() > (new_size.1 - target.display_height).abs() {
            new_size.1 = new_size.0 / orig_aspect;
        } else {
            new_size.0 = new_size.1 * orig_aspect;
        }
    }

    let new_rot_deg = options.rotation_deg.unwrap_or(target.rotation_deg);
    let rot_rad = new_rot_deg.to_radians();
    let cos_r = rot_rad.cos();
    let sin_r = rot_rad.sin();

    let sx = if options.flip_h { -new_size.0 } else { new_size.0 };
    let sy = if options.flip_v { -new_size.1 } else { new_size.1 };

    let a = sx * cos_r;
    let b = sx * sin_r;
    let c = -sy * sin_r;
    let d = sy * cos_r;
    let e = new_pos.0;
    let f = new_pos.1;

    let target_do = format!("/{} Do", image_name);
    let new_cm_str = format!("{:.2} {:.2} {:.2} {:.2} {:.2} {:.2} cm\n", a, b, c, d, e, f);

    let page = doc.get_page(page_index).unwrap();
    let contents_ids = page.contents_ids;

    for content_id in contents_ids {
        if let Some(Object::Stream(ref mut stream)) = doc.objects.get_mut(&content_id) {
            let decoded = stream.decode()?;
            let modified = update_image_matrix(&decoded, &target_do, &new_cm_str);
            if modified.len() != decoded.len() || modified != decoded {
                stream.raw_data = modified;
                let _ = stream.compress_flate();
                stream.update_length();
            }
        }
    }

    Ok(())
}

/// Applies a vector clipping path (`x y w h re W n`) around an image on the page.
///
/// Conforms to ISO 32000-1 Clause 8.4.3 (Clipping Path Operators). Traceability: FR-05.6.
pub fn crop_page_image(
    doc: &mut PdfDocument,
    page_index: usize,
    image_name: &str,
    crop_rect: [f64; 4],
) -> Result<()> {
    let target_do = format!("/{} Do", image_name);
    let crop_cmd = format!(
        "{:.2} {:.2} {:.2} {:.2} re W n\n",
        crop_rect[0], crop_rect[1], crop_rect[2], crop_rect[3]
    );

    let page = doc
        .get_page(page_index)
        .ok_or_else(|| PdfError::InvalidStructure(format!("Page {} out of range", page_index)))?;

    let contents_ids = page.contents_ids;

    for content_id in contents_ids {
        if let Some(Object::Stream(ref mut stream)) = doc.objects.get_mut(&content_id) {
            let decoded = stream.decode()?;
            let modified = insert_or_update_image_crop(&decoded, &target_do, &crop_cmd);
            if modified != decoded {
                stream.raw_data = modified;
                let _ = stream.compress_flate();
                stream.update_length();
            }
        }
    }

    Ok(())
}

/// Adjusts the opacity/transparency of an image using ExtGState dictionary and `/GS... gs`.
///
/// Conforms to ISO 32000-1 Clause 11. Traceability: FR-05.10.
pub fn set_page_image_opacity(
    doc: &mut PdfDocument,
    page_index: usize,
    image_name: &str,
    opacity: f32,
) -> Result<()> {
    let alpha = opacity.clamp(0.0, 1.0);
    let pct = (alpha * 100.0).round() as u32;
    let gs_name = format!("GS_ca{}", pct);

    // 1. Register ExtGState in Page Resources
    let page_ids = doc.page_ids();
    let page_id = *page_ids
        .get(page_index)
        .ok_or_else(|| PdfError::InvalidStructure(format!("Page {} out of range", page_index)))?;

    let page_obj = doc
        .objects
        .get_mut(&page_id)
        .ok_or(PdfError::ObjectNotFound(page_id.number, page_id.generation))?;

    let page_dict = page_obj
        .as_dict_mut()
        .ok_or_else(|| PdfError::InvalidStructure("Expected page dictionary".into()))?;

    let resources = page_dict
        .map
        .entry(PdfName::new("Resources"))
        .or_insert_with(|| Object::Dictionary(PdfDictionary::new()));

    if let Object::Dictionary(res) = resources {
        let ext_dict = res
            .map
            .entry(PdfName::new("ExtGState"))
            .or_insert_with(|| Object::Dictionary(PdfDictionary::new()));

        if let Object::Dictionary(states) = ext_dict {
            if !states.contains_key(&gs_name) {
                let mut state_dict = PdfDictionary::new();
                state_dict.insert("Type", PdfName::new("ExtGState"));
                state_dict.insert("ca", alpha as f64);
                state_dict.insert("CA", alpha as f64);
                states.insert(gs_name.clone(), state_dict);
            }
        }
    }

    // 2. Insert `/GS_ca... gs` before `/ImageName Do` in content stream
    let page = doc.get_page(page_index).unwrap();
    let contents_ids = page.contents_ids;
    let target_do = format!("/{} Do", image_name);
    let gs_cmd = format!("/{} gs\n", gs_name);

    for content_id in contents_ids {
        if let Some(Object::Stream(ref mut stream)) = doc.objects.get_mut(&content_id) {
            let decoded = stream.decode()?;
            let modified = insert_or_update_image_gs(&decoded, &target_do, &gs_cmd);
            if modified != decoded {
                stream.raw_data = modified;
                let _ = stream.compress_flate();
                stream.update_length();
            }
        }
    }

    Ok(())
}

/// Changes the Z-order of an image within a page's content stream (painter's algorithm).
///
/// Conforms to ISO 32000-1 Clause 8. Traceability: FR-05.11.
pub fn reorder_page_image(
    doc: &mut PdfDocument,
    page_index: usize,
    image_name: &str,
    action: ZOrderAction,
) -> Result<()> {
    let page = doc
        .get_page(page_index)
        .ok_or_else(|| PdfError::InvalidStructure(format!("Page {} out of range", page_index)))?;

    let target_do = format!("/{} Do", image_name);
    let contents_ids = page.contents_ids;

    // Consolidate into the active content stream
    for content_id in contents_ids {
        if let Some(Object::Stream(ref mut stream)) = doc.objects.get_mut(&content_id) {
            let decoded = stream.decode()?;
            let mut blocks = split_into_drawing_blocks(&decoded);
            if let Some(target_idx) = blocks.iter().position(|b| b.contains(&target_do)) {
                match action {
                    ZOrderAction::BringToFront => {
                        let b = blocks.remove(target_idx);
                        blocks.push(b);
                    }
                    ZOrderAction::SendToBack => {
                        let b = blocks.remove(target_idx);
                        blocks.insert(0, b);
                    }
                    ZOrderAction::MoveForward => {
                        if target_idx + 1 < blocks.len() {
                            blocks.swap(target_idx, target_idx + 1);
                        }
                    }
                    ZOrderAction::MoveBackward => {
                        if target_idx > 0 {
                            blocks.swap(target_idx, target_idx - 1);
                        }
                    }
                }
                let mut reassembled = Vec::new();
                for b in blocks {
                    reassembled.extend_from_slice(b.as_bytes());
                    if !b.ends_with('\n') {
                        reassembled.push(b'\n');
                    }
                }
                stream.raw_data = reassembled;
                let _ = stream.compress_flate();
                stream.update_length();
                break;
            }
        }
    }

    Ok(())
}

/// Computes magnetic alignment guides and snapping for an image or object bounding box.
///
/// Snaps to page margins (Left, Center, Right, Top, Middle, Bottom) and sibling objects.
/// Traceability: FR-05.9.
pub fn snap_to_alignment_guides(
    pos: (f64, f64),
    size: (f64, f64),
    page_rect: [f64; 4],
    other_rects: &[[f64; 4]],
    tolerance: f64,
) -> SnappedPosition {
    let (mut cur_x, mut cur_y) = pos;
    let (w, h) = size;
    let mut guides = Vec::new();

    let page_left = page_rect[0];
    let page_bottom = page_rect[1];
    let page_w = page_rect[2];
    let page_h = page_rect[3];
    let page_right = page_left + page_w;
    let page_top = page_bottom + page_h;
    let page_center_x = page_left + page_w / 2.0;
    let page_center_y = page_bottom + page_h / 2.0;

    // Horizontal targets: X coordinates
    let mut x_targets: Vec<(f64, &str)> = vec![
        (page_left + 36.0, "Left Margin (0.5 in)"),
        (page_left + 72.0, "Left Margin (1.0 in)"),
        (page_center_x, "Page Center X"),
        (page_right - 72.0, "Right Margin (1.0 in)"),
        (page_right - 36.0, "Right Margin (0.5 in)"),
    ];

    for r in other_rects {
        x_targets.push((r[0], "Object Left"));
        x_targets.push((r[0] + r[2] / 2.0, "Object Center"));
        x_targets.push((r[0] + r[2], "Object Right"));
    }

    // Test snap on left edge, center, and right edge of the moving object
    for (target_x, label) in x_targets {
        if (cur_x - target_x).abs() <= tolerance {
            cur_x = target_x;
            guides.push(SnapGuide {
                orientation: SnapGuideOrientation::Vertical,
                coordinate: target_x,
                label: label.to_string(),
            });
            break;
        } else if (cur_x + w / 2.0 - target_x).abs() <= tolerance {
            cur_x = target_x - w / 2.0;
            guides.push(SnapGuide {
                orientation: SnapGuideOrientation::Vertical,
                coordinate: target_x,
                label: label.to_string(),
            });
            break;
        } else if (cur_x + w - target_x).abs() <= tolerance {
            cur_x = target_x - w;
            guides.push(SnapGuide {
                orientation: SnapGuideOrientation::Vertical,
                coordinate: target_x,
                label: label.to_string(),
            });
            break;
        }
    }

    // Vertical targets: Y coordinates
    let mut y_targets: Vec<(f64, &str)> = vec![
        (page_bottom + 36.0, "Bottom Margin (0.5 in)"),
        (page_bottom + 72.0, "Bottom Margin (1.0 in)"),
        (page_center_y, "Page Center Y"),
        (page_top - 72.0, "Top Margin (1.0 in)"),
        (page_top - 36.0, "Top Margin (0.5 in)"),
    ];

    for r in other_rects {
        y_targets.push((r[1], "Object Bottom"));
        y_targets.push((r[1] + r[3] / 2.0, "Object Middle"));
        y_targets.push((r[1] + r[3], "Object Top"));
    }

    // Test snap on bottom edge, center, and top edge of the moving object
    for (target_y, label) in y_targets {
        if (cur_y - target_y).abs() <= tolerance {
            cur_y = target_y;
            guides.push(SnapGuide {
                orientation: SnapGuideOrientation::Horizontal,
                coordinate: target_y,
                label: label.to_string(),
            });
            break;
        } else if (cur_y + h / 2.0 - target_y).abs() <= tolerance {
            cur_y = target_y - h / 2.0;
            guides.push(SnapGuide {
                orientation: SnapGuideOrientation::Horizontal,
                coordinate: target_y,
                label: label.to_string(),
            });
            break;
        } else if (cur_y + h - target_y).abs() <= tolerance {
            cur_y = target_y - h;
            guides.push(SnapGuide {
                orientation: SnapGuideOrientation::Horizontal,
                coordinate: target_y,
                label: label.to_string(),
            });
            break;
        }
    }

    SnappedPosition {
        snapped_x: cur_x,
        snapped_y: cur_y,
        active_guides: guides,
    }
}

// ---------------------------------------------------------------------------
// Internal content stream modification helpers
// ---------------------------------------------------------------------------

fn concat_matrices(m1: &[f64; 6], m2: &[f64; 6]) -> [f64; 6] {
    let a1 = m1[0];
    let b1 = m1[1];
    let c1 = m1[2];
    let d1 = m1[3];
    let e1 = m1[4];
    let f1 = m1[5];

    let a2 = m2[0];
    let b2 = m2[1];
    let c2 = m2[2];
    let d2 = m2[3];
    let e2 = m2[4];
    let f2 = m2[5];

    [
        a1 * a2 + c1 * b2,
        b1 * a2 + d1 * b2,
        a1 * c2 + c1 * d2,
        b1 * c2 + d1 * d2,
        a1 * e2 + c1 * f2 + e1,
        b1 * e2 + d1 * f2 + f1,
    ]
}

fn resolve_extgstate_opacity(
    resources: Option<&PdfDictionary>,
    gs_name: &str,
    doc: &PdfDocument,
) -> Option<f32> {
    let res = resources?;
    let ext_obj = res.get("ExtGState")?;
    let ext_dict = match ext_obj {
        Object::Dictionary(d) => Some(d),
        Object::Reference(id) => match doc.objects.get(id)? {
            Object::Dictionary(d) => Some(d),
            _ => None,
        },
        _ => None,
    }?;

    let gs_obj = ext_dict.get(gs_name)?;
    let gs_dict = match gs_obj {
        Object::Dictionary(d) => Some(d),
        Object::Reference(id) => match doc.objects.get(id)? {
            Object::Dictionary(d) => Some(d),
            _ => None,
        },
        _ => None,
    }?;

    gs_dict.get_f64("ca").or_else(|| gs_dict.get_f64("CA")).map(|v| v as f32)
}

fn split_into_drawing_blocks(data: &[u8]) -> Vec<String> {
    let text = String::from_utf8_lossy(data);
    let mut blocks = Vec::new();
    let mut cur_block = String::new();
    let mut depth = 0;

    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed == "q" {
            if depth == 0 && !cur_block.trim().is_empty() {
                blocks.push(cur_block);
                cur_block = String::new();
            }
            depth += 1;
            cur_block.push_str(line);
            cur_block.push('\n');
        } else if trimmed == "Q" {
            cur_block.push_str(line);
            cur_block.push('\n');
            if depth > 0 {
                depth -= 1;
                if depth == 0 {
                    blocks.push(cur_block);
                    cur_block = String::new();
                }
            }
        } else {
            cur_block.push_str(line);
            cur_block.push('\n');
        }
    }

    if !cur_block.trim().is_empty() {
        blocks.push(cur_block);
    }

    blocks
}

fn remove_image_drawing_block(data: &[u8], target_do: &str) -> Vec<u8> {
    let blocks = split_into_drawing_blocks(data);
    let mut out = Vec::new();

    for block in blocks {
        if !block.contains(target_do) {
            out.extend_from_slice(block.as_bytes());
        }
    }

    out
}

fn update_image_matrix(data: &[u8], target_do: &str, new_cm: &str) -> Vec<u8> {
    let blocks = split_into_drawing_blocks(data);
    let mut out = Vec::new();

    for block in blocks {
        if block.contains(target_do) {
            // Find existing `... cm` in this block and replace it
            let mut lines = Vec::new();
            let mut replaced_cm = false;

            for line in block.lines() {
                let trimmed = line.trim();
                if trimmed.ends_with(" cm") && !replaced_cm {
                    lines.push(new_cm.trim().to_string());
                    replaced_cm = true;
                } else {
                    lines.push(line.to_string());
                }
            }

            if !replaced_cm {
                // Prepend cm before the Do operator
                let mut new_lines = Vec::new();
                for l in &lines {
                    if l.contains(target_do) {
                        new_lines.push(new_cm.trim().to_string());
                    }
                    new_lines.push(l.clone());
                }
                lines = new_lines;
            }

            for l in lines {
                out.extend_from_slice(l.as_bytes());
                out.push(b'\n');
            }
        } else {
            out.extend_from_slice(block.as_bytes());
        }
    }

    out
}

fn insert_or_update_image_crop(data: &[u8], target_do: &str, crop_cmd: &str) -> Vec<u8> {
    let blocks = split_into_drawing_blocks(data);
    let mut out = Vec::new();

    for block in blocks {
        if block.contains(target_do) {
            let mut lines = Vec::new();
            let mut has_re = false;

            for line in block.lines() {
                let trimmed = line.trim();
                if trimmed.ends_with(" re W n") || trimmed.ends_with(" re W* n") {
                    lines.push(crop_cmd.trim().to_string());
                    has_re = true;
                } else {
                    lines.push(line.to_string());
                }
            }

            if !has_re {
                // Insert right after 'q' or at the beginning of the block
                let mut new_lines = Vec::new();
                let mut inserted = false;
                for l in &lines {
                    new_lines.push(l.clone());
                    if l.trim() == "q" && !inserted {
                        new_lines.push(crop_cmd.trim().to_string());
                        inserted = true;
                    }
                }
                if !inserted {
                    new_lines.insert(0, crop_cmd.trim().to_string());
                }
                lines = new_lines;
            }

            for l in lines {
                out.extend_from_slice(l.as_bytes());
                out.push(b'\n');
            }
        } else {
            out.extend_from_slice(block.as_bytes());
        }
    }

    out
}

fn insert_or_update_image_gs(data: &[u8], target_do: &str, gs_cmd: &str) -> Vec<u8> {
    let blocks = split_into_drawing_blocks(data);
    let mut out = Vec::new();

    for block in blocks {
        if block.contains(target_do) {
            let mut lines = Vec::new();
            let mut has_gs = false;

            for line in block.lines() {
                let trimmed = line.trim();
                if trimmed.ends_with(" gs") {
                    lines.push(gs_cmd.trim().to_string());
                    has_gs = true;
                } else {
                    lines.push(line.to_string());
                }
            }

            if !has_gs {
                // Insert after 'q'
                let mut new_lines = Vec::new();
                let mut inserted = false;
                for l in &lines {
                    new_lines.push(l.clone());
                    if l.trim() == "q" && !inserted {
                        new_lines.push(gs_cmd.trim().to_string());
                        inserted = true;
                    }
                }
                if !inserted {
                    new_lines.insert(0, gs_cmd.trim().to_string());
                }
                lines = new_lines;
            }

            for l in lines {
                out.extend_from_slice(l.as_bytes());
                out.push(b'\n');
            }
        } else {
            out.extend_from_slice(block.as_bytes());
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::insert_raw_image;

    #[test]
    fn test_extract_and_transform_page_image() {
        let mut doc = PdfDocument::new();
        doc.add_blank_page(612.0, 792.0);

        // Insert a 2x2 test image at (100, 200) with size (300, 150)
        let raw_pixels = vec![255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 0];
        insert_raw_image(
            &mut doc,
            0,
            raw_pixels,
            2,
            2,
            100.0,
            200.0,
            300.0,
            150.0,
        )
        .expect("insert image");

        // 1. Extract and inspect metadata
        let images = extract_page_images(&doc, 0).expect("extract images");
        assert_eq!(images.len(), 1);
        let im = &images[0];
        assert_eq!(im.pixel_width, 2);
        assert_eq!(im.pixel_height, 2);
        assert_eq!(im.display_x, 100.0);
        assert_eq!(im.display_y, 200.0);
        assert_eq!(im.display_width, 300.0);
        assert_eq!(im.display_height, 150.0);
        assert_eq!(im.rotation_deg, 0.0);
        assert!(!im.flipped_h);
        assert!(!im.flipped_v);

        // 2. Transform image: Move to (120, 250), resize to (200, 100), rotate 90 deg
        let transform_opts = ImageTransformOptions {
            position: Some((120.0, 250.0)),
            size: Some((200.0, 100.0)),
            rotation_deg: Some(90.0),
            flip_h: false,
            flip_v: false,
            lock_aspect_ratio: false,
        };
        transform_page_image(&mut doc, 0, &im.name, &transform_opts).expect("transform image");

        let updated = extract_page_images(&doc, 0).expect("extract updated");
        assert_eq!(updated.len(), 1);
        let up = &updated[0];
        assert_eq!(up.display_x, 120.0);
        assert_eq!(up.display_y, 250.0);
        assert!((up.display_width - 200.0).abs() < 1.0);
        assert!((up.display_height - 100.0).abs() < 1.0);
        assert!((up.rotation_deg - 90.0).abs() < 1.0);

        // 3. Set opacity to 75%
        set_page_image_opacity(&mut doc, 0, &im.name, 0.75).expect("set opacity");
        let with_opacity = extract_page_images(&doc, 0).expect("extract opacity");
        assert_eq!(with_opacity.len(), 1);
        assert!((with_opacity[0].opacity - 0.75).abs() < 0.05);

        // 4. Crop image
        crop_page_image(&mut doc, 0, &im.name, [100.0, 200.0, 150.0, 100.0]).expect("crop image");
        let with_crop = extract_page_images(&doc, 0).expect("extract crop");
        assert_eq!(with_crop[0].clip_rect, Some([100.0, 200.0, 150.0, 100.0]));

        // 5. Delete image
        delete_page_image(&mut doc, 0, &im.name).expect("delete image");
        let after_delete = extract_page_images(&doc, 0).expect("extract after delete");
        assert_eq!(after_delete.len(), 0);
    }

    #[test]
    fn test_snap_to_alignment_guides() {
        let page_rect = [0.0, 0.0, 612.0, 792.0];
        let other_rects = vec![[100.0, 300.0, 200.0, 150.0]];

        // Moving object near left margin 72.0 (currently at 73.5)
        let pos = (73.5, 400.0);
        let size = (100.0, 50.0);
        let snapped = snap_to_alignment_guides(pos, size, page_rect, &other_rects, 3.0);

        assert_eq!(snapped.snapped_x, 72.0);
        assert_eq!(snapped.snapped_y, 400.0);
        assert_eq!(snapped.active_guides.len(), 2);
        assert_eq!(snapped.active_guides[0].label, "Left Margin (1.0 in)");
        assert_eq!(snapped.active_guides[1].label, "Object Top");
    }
}
