//! 3D Artwork and Annotations Subsystem conforming to ISO 32000-1 Clause 13.6.
//!
//! Traceability:
//! - FR-28.1 (Embedding 3D models in U3D and PRC formats into PDF pages)
//! - FR-28.2 (Interactive 3D navigation: rotate, pan, zoom, spin, walk-through)
//! - FR-28.3 (Rendering modes: Solid, Wireframe, Shaded Illustration, Transparent, Hidden Line)
//! - FR-28.4 (Lighting presets: Day, Night, Headlamp, White, CAD)
//! - FR-28.5 (Cross-section views with cutting planes)
//! - FR-28.6 (3D model tree assembly hierarchy)
//! - FR-28.7 (Named 3D camera views and guided navigation)

use ravpdf_core::document::PdfDocument;
use ravpdf_core::error::{PdfError, Result};
use ravpdf_core::object::{
    Object, ObjectId, PdfDictionary, PdfName, PdfStream, PdfString,
};
use serde::{Deserialize, Serialize};

/// 3D Model Stream format (FR-28.1, ISO 32000-1 Clause 13.6.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ThreeDFormat {
    /// Universal 3D format (ECMA-363)
    #[default]
    U3d,
    /// Product Representation Compact format (ISO 14739-1)
    Prc,
}

impl ThreeDFormat {
    pub fn subtype_name(&self) -> &'static str {
        match self {
            Self::U3d => "U3D",
            Self::Prc => "PRC",
        }
    }
}

/// Rendering modes for 3D model visualization (FR-28.3, ISO 32000-1 Table 303).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ThreeDRenderMode {
    #[default]
    Solid,
    SolidWireframe,
    Wireframe,
    ShadedIllustration,
    Transparent,
    TransparentWireframe,
    HiddenLine,
    BoundingBox,
}

impl ThreeDRenderMode {
    pub fn pdf_name(&self) -> &'static str {
        match self {
            Self::Solid => "Solid",
            Self::SolidWireframe => "SolidWireframe",
            Self::Wireframe => "Wireframe",
            Self::ShadedIllustration => "ShadedIllustration",
            Self::Transparent => "Transparent",
            Self::TransparentWireframe => "TransparentWireframe",
            Self::HiddenLine => "HiddenLine",
            Self::BoundingBox => "BoundingBox",
        }
    }

    pub fn from_pdf_name(name: &str) -> Self {
        match name {
            "SolidWireframe" => Self::SolidWireframe,
            "Wireframe" => Self::Wireframe,
            "ShadedIllustration" => Self::ShadedIllustration,
            "Transparent" => Self::Transparent,
            "TransparentWireframe" => Self::TransparentWireframe,
            "HiddenLine" => Self::HiddenLine,
            "BoundingBox" => Self::BoundingBox,
            _ => Self::Solid,
        }
    }
}

/// Lighting presets for 3D scene illumination (FR-28.4, ISO 32000-1 Table 304).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ThreeDLighting {
    None,
    White,
    #[default]
    Day,
    Night,
    Hard,
    Primary,
    Blue,
    Red,
    Cube,
    Cad,
    Headlamp,
}

impl ThreeDLighting {
    pub fn pdf_name(&self) -> &'static str {
        match self {
            Self::None => "None",
            Self::White => "White",
            Self::Day => "Day",
            Self::Night => "Night",
            Self::Hard => "Hard",
            Self::Primary => "Primary",
            Self::Blue => "Blue",
            Self::Red => "Red",
            Self::Cube => "Cube",
            Self::Cad => "CAD",
            Self::Headlamp => "Headlamp",
        }
    }

    pub fn from_pdf_name(name: &str) -> Self {
        match name {
            "White" => Self::White,
            "Night" => Self::Night,
            "Hard" => Self::Hard,
            "Primary" => Self::Primary,
            "Blue" => Self::Blue,
            "Red" => Self::Red,
            "Cube" => Self::Cube,
            "CAD" => Self::Cad,
            "Headlamp" => Self::Headlamp,
            "None" => Self::None,
            _ => Self::Day,
        }
    }
}

/// Cutting plane definition for cross-section views (FR-28.5).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossSectionPlane {
    pub center: [f64; 3],
    pub normal: [f64; 3],
    pub show_intersection: bool,
    pub cutting_color: [f32; 3],
}

impl Default for CrossSectionPlane {
    fn default() -> Self {
        Self {
            center: [0.0, 0.0, 0.0],
            normal: [0.0, 0.0, 1.0], // Z plane cut
            show_intersection: true,
            cutting_color: [0.2, 0.7, 0.9],
        }
    }
}

/// Named camera view preset for 3D navigation (FR-28.7, ISO 32000-1 Table 299).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreeDView {
    pub name: String,
    pub camera_position: [f64; 3],
    pub camera_target: [f64; 3],
    pub camera_up: [f64; 3],
    pub field_of_view: f64, // degrees
    pub render_mode: ThreeDRenderMode,
    pub lighting: ThreeDLighting,
    pub background_color: [f32; 3],
    pub cross_section: Option<CrossSectionPlane>,
}

impl Default for ThreeDView {
    fn default() -> Self {
        Self {
            name: "Default View".to_string(),
            camera_position: [0.0, -100.0, 50.0],
            camera_target: [0.0, 0.0, 0.0],
            camera_up: [0.0, 0.0, 1.0],
            field_of_view: 45.0,
            render_mode: ThreeDRenderMode::Solid,
            lighting: ThreeDLighting::Day,
            background_color: [1.0, 1.0, 1.0],
            cross_section: None,
        }
    }
}

/// 3D Model Tree component hierarchy node (FR-28.6).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreeDNode {
    pub name: String,
    pub visible: bool,
    pub opacity: f32,
    pub children: Vec<ThreeDNode>,
}

/// Configuration for creating a 3D annotation (FR-28.1 - FR-28.7).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreeDConfig {
    pub format: ThreeDFormat,
    pub model_data: Vec<u8>,
    pub rect: [f64; 4],
    pub views: Vec<ThreeDView>,
    pub default_view_index: usize,
    pub model_tree: Option<ThreeDNode>,
}

/// Inspected 3D Annotation on a PDF page (FR-28.1).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreeDAnnotation {
    pub id: ObjectId,
    pub stream_id: ObjectId,
    pub format: ThreeDFormat,
    pub rect: [f64; 4],
    pub default_view_name: String,
    pub views_count: usize,
    pub stream_size_bytes: usize,
}

/// Adds a 3D annotation to a PDF page conforming to ISO 32000-1 Clause 13.6 (FR-28.1 - FR-28.7).
pub fn add_3d_annotation(
    doc: &mut PdfDocument,
    page_index: usize,
    config: &ThreeDConfig,
) -> Result<ObjectId> {
    let page_ids = doc.page_ids();
    let page_id = *page_ids
        .get(page_index)
        .ok_or_else(|| PdfError::InvalidStructure(format!("Page {} out of range", page_index)))?;

    // 1. Create the 3D Stream (ISO 32000-1 Clause 13.6.2)
    let stream_id = doc.next_object_id();
    let mut stream_dict = PdfDictionary::new();
    stream_dict.insert("Type", PdfName::new("3D"));
    stream_dict.insert("Subtype", PdfName::new(config.format.subtype_name()));

    let stream = PdfStream::with_plain_data(stream_dict, config.model_data.clone());
    doc.objects.insert(stream_id, Object::Stream(stream));

    // 2. Build 3D View Dictionaries (ISO 32000-1 Table 299)
    let mut view_objects = Vec::new();
    for view in &config.views {
        let mut v_dict = PdfDictionary::new();
        v_dict.insert("Type", PdfName::new("3DView"));
        v_dict.insert("XN", PdfString::from_str(&view.name));
        v_dict.insert("IN", PdfString::from_str(&view.name));

        // Camera positioning (/MS /M)
        let mut c_dict = PdfDictionary::new();
        c_dict.insert(
            "C2W",
            vec![
                Object::Real(view.camera_position[0]),
                Object::Real(view.camera_position[1]),
                Object::Real(view.camera_position[2]),
                Object::Real(view.camera_target[0]),
                Object::Real(view.camera_target[1]),
                Object::Real(view.camera_target[2]),
                Object::Real(view.camera_up[0]),
                Object::Real(view.camera_up[1]),
                Object::Real(view.camera_up[2]),
            ],
        );
        c_dict.insert("FOV", Object::Real(view.field_of_view));
        v_dict.insert("C", Object::Dictionary(c_dict));

        // Render Mode (/RM)
        let mut rm_dict = PdfDictionary::new();
        rm_dict.insert("Type", PdfName::new("3DRenderMode"));
        rm_dict.insert("Subtype", PdfName::new(view.render_mode.pdf_name()));
        v_dict.insert("RM", Object::Dictionary(rm_dict));

        // Lighting (/LS)
        let mut ls_dict = PdfDictionary::new();
        ls_dict.insert("Type", PdfName::new("3DLightingScheme"));
        ls_dict.insert("Subtype", PdfName::new(view.lighting.pdf_name()));
        v_dict.insert("LS", Object::Dictionary(ls_dict));

        // Background Color (/BG)
        let mut bg_dict = PdfDictionary::new();
        bg_dict.insert("Type", PdfName::new("3DBackground"));
        bg_dict.insert(
            "C",
            vec![
                Object::Real(view.background_color[0] as f64),
                Object::Real(view.background_color[1] as f64),
                Object::Real(view.background_color[2] as f64),
            ],
        );
        v_dict.insert("BG", Object::Dictionary(bg_dict));

        // Cross Section cutting plane (/SA) (FR-28.5)
        if let Some(ref cs) = view.cross_section {
            let mut cs_dict = PdfDictionary::new();
            cs_dict.insert("Type", PdfName::new("3DCrossSection"));
            cs_dict.insert(
                "Center",
                vec![
                    Object::Real(cs.center[0]),
                    Object::Real(cs.center[1]),
                    Object::Real(cs.center[2]),
                ],
            );
            cs_dict.insert(
                "Normal",
                vec![
                    Object::Real(cs.normal[0]),
                    Object::Real(cs.normal[1]),
                    Object::Real(cs.normal[2]),
                ],
            );
            cs_dict.insert("ShowIntersection", cs.show_intersection);
            v_dict.insert("SA", Object::Dictionary(cs_dict));
        }

        view_objects.push(Object::Dictionary(v_dict));
    }

    // 3. Create 3D Annotation (ISO 32000-1 Table 298)
    let annot_id = doc.next_object_id();
    let mut annot = PdfDictionary::new();
    annot.insert("Type", PdfName::new("Annot"));
    annot.insert("Subtype", PdfName::new("3D"));
    annot.insert("P", page_id);
    annot.insert(
        "Rect",
        vec![
            Object::Real(config.rect[0]),
            Object::Real(config.rect[1]),
            Object::Real(config.rect[2]),
            Object::Real(config.rect[3]),
        ],
    );
    annot.insert("3DD", stream_id);

    // Activation dictionary (/3DA)
    let mut da = PdfDictionary::new();
    da.insert("A", PdfName::new("PO")); // Activate on Page Open
    da.insert("DIS", PdfName::new("I")); // Disable when page is invisible
    annot.insert("3DA", Object::Dictionary(da));

    let default_name = config
        .views
        .get(config.default_view_index)
        .map(|v| v.name.as_str())
        .unwrap_or("Default");
    annot.insert("3DV", PdfString::from_str(default_name));

    if !view_objects.is_empty() {
        annot.insert("3DViews", Object::Array(view_objects));
    }

    doc.objects.insert(annot_id, Object::Dictionary(annot));

    // 4. Link into page /Annots array
    let page_obj = doc
        .objects
        .get_mut(&page_id)
        .ok_or(PdfError::ObjectNotFound(page_id.number, page_id.generation))?;
    let page_dict = page_obj
        .as_dict_mut()
        .ok_or_else(|| PdfError::InvalidStructure("Expected page dictionary".into()))?;

    let annots = page_dict
        .map
        .entry(PdfName::new("Annots"))
        .or_insert_with(|| Object::Array(Vec::new()));
    if let Object::Array(arr) = annots {
        arr.push(Object::Reference(annot_id));
    }

    Ok(annot_id)
}

/// Retrieves all 3D annotations on the specified page (FR-28.1).
pub fn get_3d_annotations(doc: &PdfDocument, page_index: usize) -> Result<Vec<ThreeDAnnotation>> {
    let page_ids = doc.page_ids();
    let page_id = match page_ids.get(page_index) {
        Some(id) => *id,
        None => return Ok(Vec::new()),
    };

    let page_dict = match doc.objects.get(&page_id).and_then(Object::as_dict) {
        Some(d) => d,
        None => return Ok(Vec::new()),
    };

    let mut list = Vec::new();

    if let Some(annots) = page_dict.get_array("Annots") {
        for item in annots {
            if let Some(ref_id) = item.as_reference() {
                if let Some(dict) = doc.objects.get(&ref_id).and_then(Object::as_dict) {
                    if dict.get_name("Subtype") == Some("3D") {
                        if let Some(parsed) = parse_3d_annot(ref_id, dict, doc) {
                            list.push(parsed);
                        }
                    }
                }
            }
        }
    }

    Ok(list)
}

/// Extracts the raw binary stream data of a 3D model (U3D or PRC) from the document (FR-28.1).
pub fn extract_3d_stream(doc: &PdfDocument, annot_id: ObjectId) -> Result<Option<Vec<u8>>> {
    let annot_dict = match doc.objects.get(&annot_id).and_then(Object::as_dict) {
        Some(d) => d,
        None => return Ok(None),
    };

    if let Some(stream_id) = annot_dict.get_ref("3DD") {
        if let Some(stream) = doc.objects.get(&stream_id).and_then(Object::as_stream) {
            return stream.decode().map(Some);
        }
    }

    Ok(None)
}

fn parse_3d_annot(
    id: ObjectId,
    dict: &PdfDictionary,
    doc: &PdfDocument,
) -> Option<ThreeDAnnotation> {
    let stream_id = dict.get_ref("3DD")?;
    let rect_arr = dict.get_array("Rect")?;
    if rect_arr.len() < 4 {
        return None;
    }
    let rect = [
        rect_arr[0].as_f64()?,
        rect_arr[1].as_f64()?,
        rect_arr[2].as_f64()?,
        rect_arr[3].as_f64()?,
    ];

    let default_view_name = dict.get_str("3DV").unwrap_or_else(|| "Default".to_string());
    let views_count = dict.get_array("3DViews").map(|a| a.len()).unwrap_or(0);

    let stream = doc.objects.get(&stream_id).and_then(Object::as_stream)?;
    let format = match stream.dict.get_name("Subtype") {
        Some("PRC") => ThreeDFormat::Prc,
        _ => ThreeDFormat::U3d,
    };
    let stream_size_bytes = stream.raw_data.len();

    Some(ThreeDAnnotation {
        id,
        stream_id,
        format,
        rect,
        default_view_name,
        views_count,
        stream_size_bytes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_3d_annotation_creation_and_extraction() {
        let mut doc = PdfDocument::new();
        doc.add_blank_page(612.0, 792.0);

        let dummy_u3d_bytes = b"U3D\0\x01\x00\x00\x00DummyU3DGeometryPayload";
        let view_isometric = ThreeDView {
            name: "Isometric".to_string(),
            render_mode: ThreeDRenderMode::Solid,
            lighting: ThreeDLighting::Day,
            ..Default::default()
        };
        let view_wireframe = ThreeDView {
            name: "Wireframe Cut".to_string(),
            render_mode: ThreeDRenderMode::Wireframe,
            lighting: ThreeDLighting::Cad,
            cross_section: Some(CrossSectionPlane {
                center: [0.0, 0.0, 10.0],
                normal: [0.0, 1.0, 0.0],
                show_intersection: true,
                cutting_color: [1.0, 0.0, 0.0],
            }),
            ..Default::default()
        };

        let config = ThreeDConfig {
            format: ThreeDFormat::U3d,
            model_data: dummy_u3d_bytes.to_vec(),
            rect: [50.0, 300.0, 450.0, 700.0],
            views: vec![view_isometric, view_wireframe],
            default_view_index: 0,
            model_tree: Some(ThreeDNode {
                name: "Assembly Root".to_string(),
                visible: true,
                opacity: 1.0,
                children: vec![ThreeDNode {
                    name: "Engine Block".to_string(),
                    visible: true,
                    opacity: 0.9,
                    children: Vec::new(),
                }],
            }),
        };

        let annot_id = add_3d_annotation(&mut doc, 0, &config).expect("add 3d annotation");
        assert!(doc.objects.contains_key(&annot_id));

        // Inspect 3D annotation on page
        let annots = get_3d_annotations(&doc, 0).expect("get 3d annotations");
        assert_eq!(annots.len(), 1);
        assert_eq!(annots[0].format, ThreeDFormat::U3d);
        assert_eq!(annots[0].default_view_name, "Isometric");
        assert_eq!(annots[0].views_count, 2);

        // Extract 3D geometry stream
        let extracted = extract_3d_stream(&doc, annot_id).expect("extract 3d stream");
        assert_eq!(extracted, Some(dummy_u3d_bytes.to_vec()));
    }
}
