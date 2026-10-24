//! Multimedia and Rich Media Subsystem conforming to ISO 32000-1 Clause 12.5.6.21 & Clause 13.2.
//!
//! Traceability:
//! - FR-27.1 (Embedding H.264 MP4/MOV and AAC/MP3 audio into PDF pages)
//! - FR-27.2 (Playback controls: play, pause, seek, volume)
//! - FR-27.3 (External media streaming via HTTP/HTTPS URLs)
//! - FR-27.4 (Poster image thumbnails for video placeholders)
//! - FR-27.5 (Playback behaviors: auto-play, loop, floating window)
//! - FR-27.6 (Media playback regions on page)

use ravpdf_core::document::PdfDocument;
use ravpdf_core::error::{PdfError, Result};
use ravpdf_core::object::{
    Object, ObjectId, PdfDictionary, PdfName, PdfStream, PdfString,
};
use serde::{Deserialize, Serialize};

/// Media content source: embedded stream data or external URL stream (FR-27.1, FR-27.3).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MediaContent {
    Embedded {
        data: Vec<u8>,
        mime_type: String,
        filename: String,
    },
    ExternalUrl {
        url: String,
        mime_type: String,
    },
}

/// Activation conditions for rich media playback (FR-27.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum RichMediaActivation {
    /// /PO: When page is opened
    PageOpen,
    /// /PV: When page becomes visible in viewer
    PageVisible,
    /// /XA: Explicit user click on media annotation
    #[default]
    UserClick,
}

impl RichMediaActivation {
    pub fn pdf_condition_name(&self) -> &'static str {
        match self {
            Self::PageOpen => "PO",
            Self::PageVisible => "PV",
            Self::UserClick => "XA",
        }
    }
}

/// Deactivation conditions for rich media playback (FR-27.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum RichMediaDeactivation {
    /// /PC: When page is closed
    #[default]
    PageClose,
    /// /PI: When page becomes invisible
    PageInvisible,
    /// /XD: Explicit user click or stop action
    UserStop,
}

impl RichMediaDeactivation {
    pub fn pdf_condition_name(&self) -> &'static str {
        match self {
            Self::PageClose => "PC",
            Self::PageInvisible => "PI",
            Self::UserStop => "XD",
        }
    }
}

/// Configuration for creating a RichMedia annotation (FR-27.1 - FR-27.6).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RichMediaConfig {
    pub title: String,
    pub content: MediaContent,
    pub rect: [f64; 4],
    pub poster_image: Option<Vec<u8>>,
    pub auto_play: bool,
    pub loop_playback: bool,
    pub show_controls: bool,
    pub volume: f32, // 0.0 to 1.0
    pub activation: RichMediaActivation,
    pub deactivation: RichMediaDeactivation,
}

impl Default for RichMediaConfig {
    fn default() -> Self {
        Self {
            title: "Embedded Video".to_string(),
            content: MediaContent::ExternalUrl {
                url: "https://example.com/video.mp4".to_string(),
                mime_type: "video/mp4".to_string(),
            },
            rect: [72.0, 400.0, 360.0, 600.0],
            poster_image: None,
            auto_play: false,
            loop_playback: false,
            show_controls: true,
            volume: 1.0,
            activation: RichMediaActivation::UserClick,
            deactivation: RichMediaDeactivation::PageClose,
        }
    }
}

/// Represents an inspected RichMedia annotation on a PDF page (FR-27.1).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RichMediaAnnotation {
    pub id: ObjectId,
    pub title: String,
    pub rect: [f64; 4],
    pub is_embedded: bool,
    pub external_url: Option<String>,
    pub mime_type: String,
    pub auto_play: bool,
    pub loop_playback: bool,
    pub volume: f32,
}

/// Adds a RichMedia annotation to a page conforming to ISO 32000-1 Clause 12.5.6.21 / Clause 13.2 (FR-27.1 - FR-27.6).
pub fn add_rich_media_annotation(
    doc: &mut PdfDocument,
    page_index: usize,
    config: &RichMediaConfig,
) -> Result<ObjectId> {
    let page_ids = doc.page_ids();
    let page_id = *page_ids
        .get(page_index)
        .ok_or_else(|| PdfError::InvalidStructure(format!("Page {} out of range", page_index)))?;

    // 1. Create Media Stream / URL descriptor
    let mime_type = match &config.content {
        MediaContent::Embedded { mime_type, .. } => mime_type.clone(),
        MediaContent::ExternalUrl { mime_type, .. } => mime_type.clone(),
    };

    let media_obj_id = match &config.content {
        MediaContent::Embedded {
            data,
            filename,
            mime_type,
        } => {
            let mut stream_dict = PdfDictionary::new();
            stream_dict.insert("Type", PdfName::new("EmbeddedFile"));
            stream_dict.insert("Subtype", PdfName::new(mime_type.as_str()));
            let mut params = PdfDictionary::new();
            params.insert("Size", data.len() as i64);
            stream_dict.insert("Params", Object::Dictionary(params));

            let ef_id = doc.next_object_id();
            let stream = PdfStream::with_plain_data(stream_dict, data.clone());
            doc.objects.insert(ef_id, Object::Stream(stream));

            // File Specification Dictionary (ISO 32000-1 Clause 7.11.3)
            let mut filespec = PdfDictionary::new();
            filespec.insert("Type", PdfName::new("Filespec"));
            filespec.insert("F", PdfString::from_str(filename));
            filespec.insert("UF", PdfString::from_str(filename));
            let mut ef = PdfDictionary::new();
            ef.insert("F", ef_id);
            filespec.insert("EF", Object::Dictionary(ef));

            let m_id = doc.next_object_id();
            doc.objects.insert(m_id, Object::Dictionary(filespec));
            m_id
        }
        MediaContent::ExternalUrl { url, .. } => {
            let mut filespec = PdfDictionary::new();
            filespec.insert("Type", PdfName::new("Filespec"));
            filespec.insert("FS", PdfName::new("URL"));
            filespec.insert("F", PdfString::from_str(url));

            let m_id = doc.next_object_id();
            doc.objects.insert(m_id, Object::Dictionary(filespec));
            m_id
        }
    };

    // 2. Poster Image if present (FR-27.4)
    let poster_id = if let Some(ref img_data) = config.poster_image {
        let p_id = doc.next_object_id();
        let mut xobj = PdfDictionary::new();
        xobj.insert("Type", PdfName::new("XObject"));
        xobj.insert("Subtype", PdfName::new("Image"));
        xobj.insert("Width", 320);
        xobj.insert("Height", 240);
        xobj.insert("ColorSpace", PdfName::new("DeviceRGB"));
        xobj.insert("BitsPerComponent", 8);
        let stream = PdfStream::with_plain_data(xobj, img_data.clone());
        doc.objects.insert(p_id, Object::Stream(stream));
        Some(p_id)
    } else {
        None
    };

    // 3. RichMedia Settings Dictionary (ISO 32000-1 Clause 12.5.6.21)
    let mut activation_dict = PdfDictionary::new();
    activation_dict.insert("Condition", PdfName::new(config.activation.pdf_condition_name()));

    let mut deactivation_dict = PdfDictionary::new();
    deactivation_dict.insert("Condition", PdfName::new(config.deactivation.pdf_condition_name()));

    let mut settings = PdfDictionary::new();
    settings.insert("Activation", Object::Dictionary(activation_dict));
    settings.insert("Deactivation", Object::Dictionary(deactivation_dict));

    // 4. Media Clip & Configuration
    let mut media_params = PdfDictionary::new();
    media_params.insert("AutoPlay", config.auto_play);
    media_params.insert("Loop", config.loop_playback);
    media_params.insert("ShowControls", config.show_controls);
    media_params.insert("Volume", Object::Real(config.volume as f64));
    media_params.insert("MimeType", PdfString::from_str(&mime_type));

    let mut instance = PdfDictionary::new();
    instance.insert("Params", Object::Dictionary(media_params));
    instance.insert("Asset", media_obj_id);

    let mut configuration = PdfDictionary::new();
    configuration.insert("Instances", vec![Object::Dictionary(instance)]);

    let mut content_dict = PdfDictionary::new();
    content_dict.insert("Configurations", vec![Object::Dictionary(configuration)]);

    // 5. RichMedia Annotation Dictionary
    let annot_id = doc.next_object_id();
    let mut annot = PdfDictionary::new();
    annot.insert("Type", PdfName::new("Annot"));
    annot.insert("Subtype", PdfName::new("RichMedia"));
    annot.insert("P", page_id);
    annot.insert("NM", PdfString::from_str(&config.title));
    annot.insert(
        "Rect",
        vec![
            Object::Real(config.rect[0]),
            Object::Real(config.rect[1]),
            Object::Real(config.rect[2]),
            Object::Real(config.rect[3]),
        ],
    );
    annot.insert("RichMediaSettings", Object::Dictionary(settings));
    annot.insert("RichMediaContent", Object::Dictionary(content_dict));

    if let Some(post_id) = poster_id {
        let mut ap = PdfDictionary::new();
        ap.insert("N", post_id);
        annot.insert("AP", Object::Dictionary(ap));
    }

    doc.objects.insert(annot_id, Object::Dictionary(annot));

    // 6. Link into page /Annots array
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

/// Retrieves all RichMedia annotations on the specified page (FR-27.1).
pub fn get_rich_media_annotations(
    doc: &PdfDocument,
    page_index: usize,
) -> Result<Vec<RichMediaAnnotation>> {
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
                    if dict.get_name("Subtype") == Some("RichMedia") {
                        if let Some(parsed) = parse_rich_media_annot(ref_id, dict, doc) {
                            list.push(parsed);
                        }
                    }
                }
            }
        }
    }

    Ok(list)
}

/// Extracts the raw binary stream of an embedded media clip (FR-27.1).
pub fn extract_rich_media_content(doc: &PdfDocument, annot_id: ObjectId) -> Result<Option<Vec<u8>>> {
    let annot_dict = match doc.objects.get(&annot_id).and_then(Object::as_dict) {
        Some(d) => d,
        None => return Ok(None),
    };

    let content_dict = match annot_dict.get_dict("RichMediaContent") {
        Some(d) => d,
        None => return Ok(None),
    };

    if let Some(configs) = content_dict.get_array("Configurations") {
        if let Some(first_config) = configs.first().and_then(Object::as_dict) {
            if let Some(instances) = first_config.get_array("Instances") {
                if let Some(first_inst) = instances.first().and_then(Object::as_dict) {
                    if let Some(asset_id) = first_inst.get_ref("Asset") {
                        if let Some(filespec) = doc.objects.get(&asset_id).and_then(Object::as_dict) {
                            if let Some(ef) = filespec.get_dict("EF") {
                                if let Some(ef_id) = ef.get_ref("F") {
                                    if let Some(stream) = doc.objects.get(&ef_id).and_then(Object::as_stream) {
                                        return stream.decode().map(Some);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(None)
}

fn parse_rich_media_annot(
    id: ObjectId,
    dict: &PdfDictionary,
    doc: &PdfDocument,
) -> Option<RichMediaAnnotation> {
    let title = dict.get_str("NM").unwrap_or_else(|| "Rich Media".to_string());
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

    let mut is_embedded = false;
    let mut external_url = None;
    let mut mime_type = "video/mp4".to_string();
    let mut auto_play = false;
    let mut loop_playback = false;
    let mut volume = 1.0;

    if let Some(content) = dict.get_dict("RichMediaContent") {
        if let Some(configs) = content.get_array("Configurations") {
            if let Some(c0) = configs.first().and_then(Object::as_dict) {
                if let Some(instances) = c0.get_array("Instances") {
                    if let Some(i0) = instances.first().and_then(Object::as_dict) {
                        if let Some(params) = i0.get_dict("Params") {
                            auto_play = params.get_bool("AutoPlay").unwrap_or(false);
                            loop_playback = params.get_bool("Loop").unwrap_or(false);
                            volume = params.get_f64("Volume").unwrap_or(1.0) as f32;
                            if let Some(mt) = params.get_str("MimeType") {
                                mime_type = mt;
                            }
                        }
                        if let Some(asset_id) = i0.get_ref("Asset") {
                            if let Some(filespec) = doc.objects.get(&asset_id).and_then(Object::as_dict) {
                                if filespec.get_name("FS") == Some("URL") {
                                    external_url = filespec.get_str("F");
                                    is_embedded = false;
                                } else if filespec.get_dict("EF").is_some() {
                                    is_embedded = true;
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    Some(RichMediaAnnotation {
        id,
        title,
        rect,
        is_embedded,
        external_url,
        mime_type,
        auto_play,
        loop_playback,
        volume,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rich_media_embedded_and_streaming() {
        let mut doc = PdfDocument::new();
        doc.add_blank_page(612.0, 792.0);

        // 1. Add Embedded MP4
        let dummy_mp4 = b"\x00\x00\x00\x18ftypmp42\x00\x00\x00\x00mp42isom";
        let config_embedded = RichMediaConfig {
            title: "Product Demo Video".to_string(),
            content: MediaContent::Embedded {
                data: dummy_mp4.to_vec(),
                mime_type: "video/mp4".to_string(),
                filename: "demo.mp4".to_string(),
            },
            rect: [100.0, 400.0, 400.0, 600.0],
            poster_image: Some(vec![1, 2, 3, 4]),
            auto_play: true,
            loop_playback: true,
            show_controls: true,
            volume: 0.8,
            activation: RichMediaActivation::PageOpen,
            deactivation: RichMediaDeactivation::PageClose,
        };

        let emb_id = add_rich_media_annotation(&mut doc, 0, &config_embedded).expect("add embedded media");
        assert!(doc.objects.contains_key(&emb_id));

        // 2. Add External Streaming Video
        let config_stream = RichMediaConfig {
            title: "Live Stream".to_string(),
            content: MediaContent::ExternalUrl {
                url: "https://stream.example.com/live.m3u8".to_string(),
                mime_type: "application/x-mpegURL".to_string(),
            },
            rect: [100.0, 150.0, 400.0, 350.0],
            poster_image: None,
            auto_play: false,
            loop_playback: false,
            show_controls: true,
            volume: 1.0,
            activation: RichMediaActivation::UserClick,
            deactivation: RichMediaDeactivation::UserStop,
        };

        let stream_id = add_rich_media_annotation(&mut doc, 0, &config_stream).expect("add stream media");
        assert!(doc.objects.contains_key(&stream_id));

        // 3. Inspect Annotations
        let list = get_rich_media_annotations(&doc, 0).expect("get annotations");
        assert_eq!(list.len(), 2);

        let parsed_emb = list.iter().find(|a| a.title == "Product Demo Video").unwrap();
        assert!(parsed_emb.is_embedded);
        assert!(parsed_emb.auto_play);
        assert!(parsed_emb.loop_playback);
        assert!((parsed_emb.volume - 0.8).abs() < 1e-4);

        let parsed_stream = list.iter().find(|a| a.title == "Live Stream").unwrap();
        assert!(!parsed_stream.is_embedded);
        assert_eq!(parsed_stream.external_url.as_deref(), Some("https://stream.example.com/live.m3u8"));

        // 4. Extract Embedded Media
        let extracted = extract_rich_media_content(&doc, emb_id).expect("extract media");
        assert_eq!(extracted, Some(dummy_mp4.to_vec()));
    }
}
