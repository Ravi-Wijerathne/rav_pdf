//! Annotation Rendering conforming to ISO 32000-1 Section 12.5.5 and Section 12.5.6.
//!
//! Traceability:
//! - FR-01.1.1 (Render all PDF page content including annotations)
//! - FR-08.1 - FR-08.3 (Sticky notes, FreeText, Highlights, Underline, StrikeOut)
//! - FR-09.1 - FR-09.5 (Rectangles, Circles, Lines)
//! - FR-10.1 - FR-10.4 (Rubber stamps)

use crate::interpreter::ContentInterpreter;
use ravpdf_core::document::PdfDocument;
use ravpdf_core::object::{Object, PdfDictionary};
use tiny_skia::{Color, Paint, PathBuilder, Pixmap, Rect, Stroke, Transform};

/// Renders all annotations attached to `page_index` onto the target `pixmap`.
pub fn render_page_annotations(
    pixmap: &mut Pixmap,
    doc: &PdfDocument,
    page_index: usize,
    crop_box: [f64; 4],
    zoom: f32,
) {
    let page_ids = doc.page_ids();
    let page_id = match page_ids.get(page_index) {
        Some(id) => *id,
        None => return,
    };

    let page_dict = match doc.objects.get(&page_id).and_then(Object::as_dict) {
        Some(d) => d,
        None => return,
    };

    let annots_array = match page_dict.get_array("Annots") {
        Some(arr) => arr,
        None => return,
    };

    let page_h = (crop_box[3] - crop_box[1]).abs() as f32;

    for item in annots_array {
        let annot_dict = match item {
            Object::Dictionary(d) => Some(d),
            Object::Reference(id) => doc.objects.get(id).and_then(Object::as_dict),
            _ => None,
        };

        if let Some(dict) = annot_dict {
            render_single_annotation(pixmap, doc, dict, crop_box, page_h, zoom);
        }
    }
}

fn render_single_annotation(
    pixmap: &mut Pixmap,
    doc: &PdfDocument,
    dict: &PdfDictionary,
    crop_box: [f64; 4],
    _page_h: f32,
    zoom: f32,
) {
    // 1. If an appearance stream exists (/AP << /N stream >>), render it first
    if let Some(ap_dict) = dict.get_dict("AP") {
        if let Some(n_obj) = ap_dict.get("N") {
            let stream = match n_obj {
                Object::Stream(s) => Some(s),
                Object::Reference(id) => doc.objects.get(id).and_then(Object::as_stream),
                _ => None,
            };

            if let Some(s) = stream {
                if let Ok(bytes) = s.decode() {
                    let mut interpreter = ContentInterpreter::new(pixmap, crop_box, zoom);
                    let _ = interpreter.interpret(&bytes, s.dict.get_dict("Resources"));
                    return;
                }
            }
        }
    }

    // 2. Fallback procedural rendering for standard ISO 32000-1 annotation subtypes
    let subtype = match dict.get_name("Subtype") {
        Some(s) => s,
        None => return,
    };

    let rect_arr = match dict.get_array("Rect") {
        Some(arr) if arr.len() >= 4 => arr,
        _ => return,
    };

    let x1 = rect_arr[0].as_f64().unwrap_or(0.0);
    let y1 = rect_arr[1].as_f64().unwrap_or(0.0);
    let x2 = rect_arr[2].as_f64().unwrap_or(0.0);
    let y2 = rect_arr[3].as_f64().unwrap_or(0.0);

    let rx = ((x1.min(x2) - crop_box[0]) as f32 * zoom).round();
    let ry = ((crop_box[3] - y1.max(y2)) as f32 * zoom).round();
    let rw = ((x2 - x1).abs() as f32 * zoom).round().max(1.0);
    let rh = ((y2 - y1).abs() as f32 * zoom).round().max(1.0);

    let color = if let Some(c_arr) = dict.get_array("C") {
        if c_arr.len() >= 3 {
            Color::from_rgba(
                c_arr[0].as_f64().unwrap_or(1.0) as f32,
                c_arr[1].as_f64().unwrap_or(1.0) as f32,
                c_arr[2].as_f64().unwrap_or(0.0) as f32,
                dict.get_f64("CA").unwrap_or(1.0) as f32,
            )
            .unwrap_or(Color::from_rgba8(255, 255, 0, 128))
        } else {
            Color::from_rgba8(255, 255, 0, 128)
        }
    } else {
        Color::from_rgba8(255, 255, 0, 128)
    };

    match subtype {
        "Highlight" => {
            let mut paint = Paint::default();
            // Default translucent yellow highlight (FR-08.2.1)
            let hl_color = Color::from_rgba(color.red(), color.green(), color.blue(), 0.4)
                .unwrap_or(Color::from_rgba8(255, 255, 0, 102));
            paint.set_color(hl_color);
            paint.anti_alias = true;

            if let Some(skia_rect) = Rect::from_xywh(rx, ry, rw, rh) {
                pixmap.fill_rect(skia_rect, &paint, Transform::identity(), None);
            }
        }

        "Underline" => {
            let mut paint = Paint::default();
            paint.set_color(Color::from_rgba8(0, 102, 204, 255));
            paint.anti_alias = true;

            let stroke = Stroke {
                width: 1.5_f32 * zoom,
                ..Default::default()
            };

            let mut pb = PathBuilder::new();
            pb.move_to(rx, ry + rh);
            pb.line_to(rx + rw, ry + rh);
            if let Some(path) = pb.finish() {
                pixmap.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
            }
        }

        "StrikeOut" => {
            let mut paint = Paint::default();
            paint.set_color(Color::from_rgba8(204, 0, 0, 255));
            paint.anti_alias = true;

            let stroke = Stroke {
                width: 1.5_f32 * zoom,
                ..Default::default()
            };

            let mut pb = PathBuilder::new();
            pb.move_to(rx, ry + rh * 0.5);
            pb.line_to(rx + rw, ry + rh * 0.5);
            if let Some(path) = pb.finish() {
                pixmap.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
            }
        }

        "Square" => {
            let mut paint = Paint::default();
            paint.set_color(color);
            paint.anti_alias = true;

            let stroke = Stroke {
                width: 2.0_f32 * zoom,
                ..Default::default()
            };

            if let Some(skia_rect) = Rect::from_xywh(rx, ry, rw, rh) {
                let mut pb = PathBuilder::new();
                pb.push_rect(skia_rect);
                if let Some(path) = pb.finish() {
                    pixmap.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
                }
            }
        }

        "Circle" => {
            let mut paint = Paint::default();
            paint.set_color(color);
            paint.anti_alias = true;

            let stroke = Stroke {
                width: 2.0_f32 * zoom,
                ..Default::default()
            };

            // Standard 4-arc cubic bezier ellipse
            let cx = rx + rw * 0.5;
            let cy = ry + rh * 0.5;
            let rx_rad = rw * 0.5;
            let ry_rad = rh * 0.5;
            let kx = 0.552_284_8_f32 * rx_rad;
            let ky = 0.552_284_8_f32 * ry_rad;

            let mut pb = PathBuilder::new();
            pb.move_to(cx, cy - ry_rad);
            pb.cubic_to(cx + kx, cy - ry_rad, cx + rx_rad, cy - ky, cx + rx_rad, cy);
            pb.cubic_to(cx + rx_rad, cy + ky, cx + kx, cy + ry_rad, cx, cy + ry_rad);
            pb.cubic_to(cx - kx, cy + ry_rad, cx - rx_rad, cy + ky, cx - rx_rad, cy);
            pb.cubic_to(cx - rx_rad, cy - ky, cx - kx, cy - ry_rad, cx, cy - ry_rad);
            pb.close();

            if let Some(path) = pb.finish() {
                pixmap.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
            }
        }

        "Line" => {
            if let Some(l_arr) = dict.get_array("L") {
                if l_arr.len() >= 4 {
                    let lx1 = l_arr[0].as_f64().unwrap_or(x1);
                    let ly1 = l_arr[1].as_f64().unwrap_or(y1);
                    let lx2 = l_arr[2].as_f64().unwrap_or(x2);
                    let ly2 = l_arr[3].as_f64().unwrap_or(y2);

                    let p1_x = (lx1 - crop_box[0]) as f32 * zoom;
                    let p1_y = (crop_box[3] - ly1) as f32 * zoom;
                    let p2_x = (lx2 - crop_box[0]) as f32 * zoom;
                    let p2_y = (crop_box[3] - ly2) as f32 * zoom;

                    let mut paint = Paint::default();
                    paint.set_color(color);
                    paint.anti_alias = true;

                    let stroke = Stroke {
                        width: 2.0_f32 * zoom,
                        ..Default::default()
                    };

                    let mut pb = PathBuilder::new();
                    pb.move_to(p1_x, p1_y);
                    pb.line_to(p2_x, p2_y);
                    if let Some(path) = pb.finish() {
                        pixmap.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
                    }
                }
            }
        }

        "Text" => {
            // Render iconic sticky note symbol (FR-08.1.1)
            let note_size = (20.0 * zoom).clamp(14.0, 32.0);
            if let Some(note_rect) = Rect::from_xywh(rx, ry, note_size, note_size) {
                let mut bg_paint = Paint::default();
                bg_paint.set_color(Color::from_rgba8(254, 240, 138, 240));
                pixmap.fill_rect(note_rect, &bg_paint, Transform::identity(), None);

                let mut stroke_paint = Paint::default();
                stroke_paint.set_color(Color::from_rgba8(202, 138, 4, 255));
                let stroke = Stroke {
                    width: 1.0_f32,
                    ..Default::default()
                };

                let mut pb = PathBuilder::new();
                pb.push_rect(note_rect);
                // Lines inside sticky note icon
                pb.move_to(rx + 3.0, ry + 6.0);
                pb.line_to(rx + note_size - 3.0, ry + 6.0);
                pb.move_to(rx + 3.0, ry + 10.0);
                pb.line_to(rx + note_size - 3.0, ry + 10.0);
                pb.move_to(rx + 3.0, ry + 14.0);
                pb.line_to(rx + note_size - 6.0, ry + 14.0);

                if let Some(path) = pb.finish() {
                    pixmap.stroke_path(&path, &stroke_paint, &stroke, Transform::identity(), None);
                }
            }
        }

        "Stamp" => {
            // Render rubber stamp box (FR-10.1)
            let stamp_name = dict.get_name("Name").unwrap_or("DRAFT");
            let mut border_paint = Paint::default();
            border_paint.set_color(Color::from_rgba8(220, 38, 38, 220));
            border_paint.anti_alias = true;

            let stroke = Stroke {
                width: 2.5_f32 * zoom,
                ..Default::default()
            };

            if let Some(stamp_rect) = Rect::from_xywh(rx, ry, rw, rh) {
                let mut pb = PathBuilder::new();
                pb.push_rect(stamp_rect);
                if let Some(path) = pb.finish() {
                    pixmap.stroke_path(&path, &border_paint, &stroke, Transform::identity(), None);
                }
            }

            // Approximate stamp label text rendering
            let mut text_paint = Paint::default();
            text_paint.set_color(Color::from_rgba8(220, 38, 38, 220));
            text_paint.anti_alias = true;

            let font_size = (rh * 0.5).clamp(8.0, 36.0);
            let char_w = font_size * 0.5;
            let mut cur_x = rx + (rw - (stamp_name.len() as f32 * char_w)).max(0.0) * 0.5;
            let cur_y = ry + (rh - font_size) * 0.5;

            for ch in stamp_name.chars() {
                if !ch.is_control() {
                    let mut pb = PathBuilder::new();
                    pb.move_to(cur_x, cur_y);
                    pb.line_to(cur_x + char_w * 0.8, cur_y);
                    pb.line_to(cur_x + char_w * 0.8, cur_y + font_size * 0.8);
                    pb.line_to(cur_x, cur_y + font_size * 0.8);
                    pb.close();
                    if let Some(path) = pb.finish() {
                        let s = Stroke {
                            width: (font_size * 0.1).max(0.5),
                            ..Default::default()
                        };
                        pixmap.stroke_path(&path, &text_paint, &s, Transform::identity(), None);
                    }
                }
                cur_x += char_w;
            }
        }

        "Ink" => {
            if let Some(ink_list) = dict.get_array("InkList") {
                let stroke_w = dict
                    .get_dict("BS")
                    .and_then(|bs| bs.get_f64("W"))
                    .unwrap_or(2.0) as f32
                    * zoom;

                let stroke = Stroke {
                    width: stroke_w.max(1.0),
                    ..Default::default()
                };

                let mut paint = Paint::default();
                paint.set_color(color);
                paint.anti_alias = true;

                for stroke_obj in ink_list {
                    if let Object::Array(coords) = stroke_obj {
                        let mut pb = PathBuilder::new();
                        let mut started = false;
                        let mut i = 0;
                        while i + 1 < coords.len() {
                            if let (Some(x), Some(y)) = (coords[i].as_f64(), coords[i + 1].as_f64())
                            {
                                let px = (x - crop_box[0]) as f32 * zoom;
                                let py = (crop_box[3] - y) as f32 * zoom;
                                if !started {
                                    pb.move_to(px, py);
                                    started = true;
                                } else {
                                    pb.line_to(px, py);
                                }
                            }
                            i += 2;
                        }
                        if let Some(path) = pb.finish() {
                            pixmap.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
                        }
                    }
                }
            }
        }

        "Polygon" | "PolyLine" => {
            if let Some(vertices) = dict.get_array("Vertices") {
                let stroke_w = dict
                    .get_dict("BS")
                    .and_then(|bs| bs.get_f64("W"))
                    .unwrap_or(2.0) as f32
                    * zoom;

                let stroke = Stroke {
                    width: stroke_w.max(1.0),
                    ..Default::default()
                };

                let mut stroke_paint = Paint::default();
                stroke_paint.set_color(color);
                stroke_paint.anti_alias = true;

                let mut pb = PathBuilder::new();
                let mut started = false;
                let mut i = 0;
                while i + 1 < vertices.len() {
                    if let (Some(x), Some(y)) = (vertices[i].as_f64(), vertices[i + 1].as_f64()) {
                        let px = (x - crop_box[0]) as f32 * zoom;
                        let py = (crop_box[3] - y) as f32 * zoom;
                        if !started {
                            pb.move_to(px, py);
                            started = true;
                        } else {
                            pb.line_to(px, py);
                        }
                    }
                    i += 2;
                }

                if subtype == "Polygon" {
                    pb.close();
                }

                if let Some(path) = pb.finish() {
                    // Optional interior color fill /IC
                    if let Some(ic_arr) = dict.get_array("IC") {
                        if ic_arr.len() >= 3 {
                            if let (Some(r), Some(g), Some(b)) =
                                (ic_arr[0].as_f64(), ic_arr[1].as_f64(), ic_arr[2].as_f64())
                            {
                                let mut fill_paint = Paint::default();
                                fill_paint.set_color(
                                    Color::from_rgba(r as f32, g as f32, b as f32, color.alpha())
                                        .unwrap_or(Color::WHITE),
                                );
                                fill_paint.anti_alias = true;
                                pixmap.fill_path(
                                    &path,
                                    &fill_paint,
                                    tiny_skia::FillRule::Winding,
                                    Transform::identity(),
                                    None,
                                );
                            }
                        }
                    }
                    pixmap.stroke_path(&path, &stroke_paint, &stroke, Transform::identity(), None);
                }
            }
        }

        _ => {}
    }
}
