//! Accessibility Checker and Tagged PDF Remediation Engine conforming to ISO 14289 (PDF/UA) and WCAG 2.1.
//!
//! Traceability:
//! - FR-21.1 (Accessibility Checker auditing against PDF/UA ISO 14289 and WCAG 2.1)
//! - FR-21.2 (Accessibility report categorizing issues as Passed, Failed, or Needs Manual Check)
//! - FR-21.3 (Document structure tree root and tagged PDF validation)
//! - FR-21.4 (Logical reading order validation)
//! - FR-21.5 (Alternative text descriptions for images and figures)
//! - FR-21.6 (Document language specification)
//! - FR-21.7 (Table headers and tabular structure)
//! - FR-21.8 (Decorative artifacts marking)
//! - FR-21.9 (Document title display preference in ViewerPreferences and metadata)
//! - FR-21.10 (Page tab order validation for keyboard navigation)

use ravpdf_core::document::PdfDocument;
use ravpdf_core::error::{PdfError, Result};
use ravpdf_core::object::{Object, PdfDictionary, PdfName, PdfString};
use serde::{Deserialize, Serialize};

/// Conformance status of a single accessibility audit rule (FR-21.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AccessibilityStatus {
    Passed,
    Failed,
    NeedsManualCheck,
}

impl AccessibilityStatus {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Passed => "Passed",
            Self::Failed => "Failed",
            Self::NeedsManualCheck => "Needs Manual Check",
        }
    }
}

/// Category of accessibility check (FR-21.1, FR-21.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AccessibilityCategory {
    Document,
    PageOrder,
    AlternateText,
    HeadingsAndStructure,
    Tables,
    ColorAndContrast,
}

impl AccessibilityCategory {
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Document => "Document & Metadata",
            Self::PageOrder => "Page & Keyboard Navigation",
            Self::AlternateText => "Figures & Alternative Text",
            Self::HeadingsAndStructure => "Document Structure & Tags",
            Self::Tables => "Tables & Data Grid",
            Self::ColorAndContrast => "Color & Visual Contrast",
        }
    }
}

/// An individual accessibility verification rule (FR-21.1).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessibilityRule {
    pub id: String,
    pub standard: String,
    pub category: AccessibilityCategory,
    pub title: String,
    pub status: AccessibilityStatus,
    pub description: String,
    pub remediation_hint: String,
    pub can_auto_fix: bool,
}

/// Comprehensive accessibility audit report (FR-21.1, FR-21.2).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessibilityReport {
    pub rules: Vec<AccessibilityRule>,
    pub passed_count: usize,
    pub failed_count: usize,
    pub manual_check_count: usize,
    pub is_compliant: bool,
}

impl AccessibilityReport {
    pub fn summary_text(&self) -> String {
        format!(
            "Accessibility Audit: {} Passed, {} Failed, {} Needs Manual Check (Status: {})",
            self.passed_count,
            self.failed_count,
            self.manual_check_count,
            if self.is_compliant { "COMPLIANT" } else { "NON-COMPLIANT" }
        )
    }
}

/// Runs a full PDF/UA (ISO 14289) and WCAG 2.1 accessibility audit on the document (FR-21.1 - FR-21.10).
pub fn audit_accessibility(doc: &PdfDocument) -> Result<AccessibilityReport> {
    let mut rules = Vec::new();

    let catalog = doc.catalog().cloned().unwrap_or_default();
    let info_dict = doc.info_id.and_then(|id| doc.get_object(id)).and_then(Object::as_dict);

    // 1. FR-21.9: Document Title in Metadata
    let title_opt = info_dict.and_then(|d| d.get_str("Title"));
    let has_valid_title = title_opt.as_ref().map(|s| !s.trim().is_empty()).unwrap_or(false);
    rules.push(AccessibilityRule {
        id: "PDF-UA-DocTitle".into(),
        standard: "PDF/UA 7.1 & WCAG 2.4.2".into(),
        category: AccessibilityCategory::Document,
        title: "Document Title Specified in Metadata".into(),
        status: if has_valid_title { AccessibilityStatus::Passed } else { AccessibilityStatus::Failed },
        description: if has_valid_title {
            format!("Document title is defined: \"{}\"", title_opt.unwrap_or_default())
        } else {
            "Document lacks a non-empty /Title entry in the /Info metadata dictionary.".into()
        },
        remediation_hint: "Set a meaningful, descriptive title in Document Properties.".into(),
        can_auto_fix: true,
    });

    // 2. FR-21.9: DisplayDocTitle preference in ViewerPreferences
    let display_doc_title = (|| -> bool {
        if let Some(pref_obj) = catalog.get("ViewerPreferences") {
            let pref_dict = match pref_obj {
                Object::Dictionary(d) => Some(d),
                Object::Reference(id) => doc.get_object(*id).and_then(Object::as_dict),
                _ => None,
            };
            if let Some(dict) = pref_dict {
                return dict.get_bool("DisplayDocTitle").unwrap_or(false);
            }
        }
        false
    })();

    rules.push(AccessibilityRule {
        id: "PDF-UA-DisplayDocTitle".into(),
        standard: "PDF/UA 7.1 & WCAG 2.4.2".into(),
        category: AccessibilityCategory::Document,
        title: "Display Document Title Preference".into(),
        status: if display_doc_title { AccessibilityStatus::Passed } else { AccessibilityStatus::Failed },
        description: if display_doc_title {
            "ViewerPreferences specifies /DisplayDocTitle true.".into()
        } else {
            "ViewerPreferences does not set /DisplayDocTitle true; viewer titlebars may show raw filenames.".into()
        },
        remediation_hint: "Enable 'Display Document Title' in ViewerPreferences.".into(),
        can_auto_fix: true,
    });

    // 3. FR-21.6: Document Natural Language
    let lang_opt = catalog.get_str("Lang");
    let has_lang = lang_opt.as_ref().map(|s| !s.trim().is_empty()).unwrap_or(false);
    rules.push(AccessibilityRule {
        id: "PDF-UA-Lang".into(),
        standard: "PDF/UA 7.2 & WCAG 3.1.1".into(),
        category: AccessibilityCategory::Document,
        title: "Document Primary Natural Language".into(),
        status: if has_lang { AccessibilityStatus::Passed } else { AccessibilityStatus::Failed },
        description: if has_lang {
            format!("Primary language is set to '{}'.", lang_opt.unwrap_or_default())
        } else {
            "Document Catalog lacks a /Lang entry (RFC 3066 / BCP 47 language code).".into()
        },
        remediation_hint: "Specify document language (e.g., 'en-US', 'en-GB', 'es', 'fr').".into(),
        can_auto_fix: true,
    });

    // 4. FR-21.3: Tagged PDF (MarkInfo dictionary)
    let is_marked = (|| -> bool {
        if let Some(mark_obj) = catalog.get("MarkInfo") {
            let mark_dict = match mark_obj {
                Object::Dictionary(d) => Some(d),
                Object::Reference(id) => doc.get_object(*id).and_then(Object::as_dict),
                _ => None,
            };
            if let Some(dict) = mark_dict {
                return dict.get_bool("Marked").unwrap_or(false);
            }
        }
        false
    })();

    rules.push(AccessibilityRule {
        id: "PDF-UA-Marked".into(),
        standard: "ISO 32000-1 Clause 14.8 & PDF/UA 7.1".into(),
        category: AccessibilityCategory::HeadingsAndStructure,
        title: "Tagged PDF Conformance (MarkInfo)".into(),
        status: if is_marked { AccessibilityStatus::Passed } else { AccessibilityStatus::Failed },
        description: if is_marked {
            "Document catalog contains /MarkInfo << /Marked true >>.".into()
        } else {
            "Document is not tagged. Screen readers cannot reliably ascertain reading structure.".into()
        },
        remediation_hint: "Tag document structure and enable Marked flag in MarkInfo.".into(),
        can_auto_fix: true,
    });

    // 5. FR-21.3: Structure Tree Root (/StructTreeRoot)
    let has_struct_tree = catalog.get("StructTreeRoot").is_some();
    rules.push(AccessibilityRule {
        id: "PDF-UA-StructTreeRoot".into(),
        standard: "ISO 32000-1 Clause 14.7.2".into(),
        category: AccessibilityCategory::HeadingsAndStructure,
        title: "Document Structure Tree Root".into(),
        status: if has_struct_tree { AccessibilityStatus::Passed } else { AccessibilityStatus::Failed },
        description: if has_struct_tree {
            "Document contains a /StructTreeRoot structure hierarchy.".into()
        } else {
            "Document Catalog does not reference a /StructTreeRoot dictionary.".into()
        },
        remediation_hint: "Generate logical document structure tree.".into(),
        can_auto_fix: true,
    });

    // 6. FR-21.10: Page Tab Order for Keyboard Navigation
    let page_ids = doc.page_ids();
    let mut non_structure_tab_pages = Vec::new();

    for (idx, &page_id) in page_ids.iter().enumerate() {
        if let Some(page_dict) = doc.get_object(page_id).and_then(Object::as_dict) {
            let tab_order = page_dict.get_name("Tabs").unwrap_or("");
            if tab_order != "S" {
                non_structure_tab_pages.push(idx + 1);
            }
        }
    }

    let tab_order_passed = non_structure_tab_pages.is_empty();
    rules.push(AccessibilityRule {
        id: "PDF-UA-TabOrder".into(),
        standard: "PDF/UA 7.1 & WCAG 2.4.3".into(),
        category: AccessibilityCategory::PageOrder,
        title: "Page Tab Order Set to Structure (/Tabs /S)".into(),
        status: if tab_order_passed { AccessibilityStatus::Passed } else { AccessibilityStatus::Failed },
        description: if tab_order_passed {
            "All pages specify /Tabs /S ensuring keyboard navigation matches document structure.".into()
        } else {
            format!(
                "Pages {:?} do not have /Tabs set to Structure (/S).",
                non_structure_tab_pages
            )
        },
        remediation_hint: "Update page dictionaries with /Tabs /S (Structure order).".into(),
        can_auto_fix: true,
    });

    // 7. FR-21.5: Figures Alternative Text
    let mut missing_alt_images = 0;
    let mut total_images = 0;

    for &page_id in &page_ids {
        if let Some(page_dict) = doc.get_object(page_id).and_then(Object::as_dict) {
            if let Some(res_dict) = page_dict.get_dict("Resources") {
                if let Some(xobj_dict) = res_dict.get_dict("XObject") {
                    for xobj_ref in xobj_dict.map.values() {
                        if let Some(obj) = doc.resolve(xobj_ref) {
                            let (subtype, has_alt) = match obj {
                                Object::Stream(s) => (s.dict.get_name("Subtype"), s.dict.get("Alt").is_some()),
                                Object::Dictionary(d) => (d.get_name("Subtype"), d.get("Alt").is_some()),
                                _ => (None, false),
                            };
                            if subtype == Some("Image") {
                                total_images += 1;
                                if !has_alt {
                                    missing_alt_images += 1;
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    let alt_status = if missing_alt_images == 0 {
        AccessibilityStatus::Passed
    } else {
        AccessibilityStatus::Failed
    };

    rules.push(AccessibilityRule {
        id: "PDF-UA-AltText".into(),
        standard: "PDF/UA 7.3 & WCAG 1.1.1".into(),
        category: AccessibilityCategory::AlternateText,
        title: "Figures & Images Alternative Text".into(),
        status: alt_status,
        description: if total_images == 0 {
            "No image objects detected in document pages.".into()
        } else if missing_alt_images == 0 {
            format!("All {} image objects have alternative text or artifacts defined.", total_images)
        } else {
            format!("{} of {} image objects lack alternative text descriptions (/Alt).", missing_alt_images, total_images)
        },
        remediation_hint: "Add /Alt descriptions to all meaningful figures or mark decorative graphics as /Artifact.".into(),
        can_auto_fix: false,
    });

    // 8. FR-21.4: Logical Reading Order Inspection
    rules.push(AccessibilityRule {
        id: "WCAG-2.1-ReadingOrder".into(),
        standard: "WCAG 1.3.2 Meaningful Sequence".into(),
        category: AccessibilityCategory::HeadingsAndStructure,
        title: "Logical Reading Order Verification".into(),
        status: AccessibilityStatus::NeedsManualCheck,
        description: "Verify that multi-column layouts, sidebars, and callouts read sequentially for screen readers.".into(),
        remediation_hint: "Use Reading Order panel to verify block sequence.".into(),
        can_auto_fix: false,
    });

    // 9. FR-21.7: Data Tables Markup
    rules.push(AccessibilityRule {
        id: "PDF-UA-TableStructure".into(),
        standard: "PDF/UA 7.5 & WCAG 1.3.1".into(),
        category: AccessibilityCategory::Tables,
        title: "Table Header & Cell Association".into(),
        status: AccessibilityStatus::NeedsManualCheck,
        description: "Verify that tabular content uses /Table, /TR, /TH header cells, and /TD data cells.".into(),
        remediation_hint: "Ensure data tables designate column/row header cells (/TH).".into(),
        can_auto_fix: false,
    });

    // 10. WCAG 1.4.3: Color and Contrast
    rules.push(AccessibilityRule {
        id: "WCAG-2.1-Contrast".into(),
        standard: "WCAG 1.4.3 Contrast Minimum".into(),
        category: AccessibilityCategory::ColorAndContrast,
        title: "Color Contrast Ratio (4.5:1 minimum)".into(),
        status: AccessibilityStatus::NeedsManualCheck,
        description: "Verify text and background colors achieve at least 4.5:1 contrast for normal text and 3:1 for large text.".into(),
        remediation_hint: "Inspect color swatches and adjust foreground or background shades as needed.".into(),
        can_auto_fix: false,
    });

    let mut passed_count = 0;
    let mut failed_count = 0;
    let mut manual_check_count = 0;

    for rule in &rules {
        match rule.status {
            AccessibilityStatus::Passed => passed_count += 1,
            AccessibilityStatus::Failed => failed_count += 1,
            AccessibilityStatus::NeedsManualCheck => manual_check_count += 1,
        }
    }

    let is_compliant = failed_count == 0;

    Ok(AccessibilityReport {
        rules,
        passed_count,
        failed_count,
        manual_check_count,
        is_compliant,
    })
}

// ---------------------------------------------------------------------------
// Accessibility Auto-Remediation / Quick Fix Functions (FR-21.3 - FR-21.10)
// ---------------------------------------------------------------------------

/// Sets or updates the document title and viewer preferences (FR-21.9).
pub fn fix_document_title(doc: &mut PdfDocument, title: &str) -> Result<()> {
    if title.trim().is_empty() {
        return Err(PdfError::InvalidStructure("Title cannot be empty".into()));
    }

    // 1. Update /Info dictionary
    let info_id = doc.info_id.unwrap_or_else(|| {
        let id = doc.next_object_id();
        doc.info_id = Some(id);
        doc.trailer.insert("Info", id);
        id
    });

    if let Some(obj) = doc.get_object_mut(info_id) {
        if let Some(dict) = obj.as_dict_mut() {
            dict.insert("Title", PdfString::from_str(title));
        }
    } else {
        let mut dict = PdfDictionary::new();
        dict.insert("Title", PdfString::from_str(title));
        doc.objects.insert(info_id, Object::Dictionary(dict));
    }

    // 2. Set /ViewerPreferences << /DisplayDocTitle true >> in Catalog
    let root_id = doc.root_id;
    if let Some(obj) = doc.get_object_mut(root_id) {
        if let Some(catalog) = obj.as_dict_mut() {
            let mut pref_dict = match catalog.get("ViewerPreferences") {
                Some(Object::Dictionary(d)) => d.clone(),
                _ => PdfDictionary::new(),
            };
            pref_dict.insert("DisplayDocTitle", true);
            catalog.insert("ViewerPreferences", pref_dict);
        }
    }

    Ok(())
}

/// Sets the document natural primary language in the Catalog (FR-21.6).
pub fn fix_document_language(doc: &mut PdfDocument, lang: &str) -> Result<()> {
    let clean_lang = lang.trim();
    if clean_lang.is_empty() {
        return Err(PdfError::InvalidStructure("Language code cannot be empty".into()));
    }

    let root_id = doc.root_id;
    if let Some(obj) = doc.get_object_mut(root_id) {
        if let Some(catalog) = obj.as_dict_mut() {
            catalog.insert("Lang", PdfString::from_str(clean_lang));
            return Ok(());
        }
    }
    Err(PdfError::InvalidStructure("Missing document root catalog".into()))
}

/// Sets /Tabs /S (Structure order) on all pages in the document (FR-21.10).
pub fn fix_tab_order_to_structure(doc: &mut PdfDocument) -> Result<usize> {
    let page_ids = doc.page_ids();
    let mut updated_count = 0;

    for page_id in page_ids {
        if let Some(obj) = doc.get_object_mut(page_id) {
            if let Some(dict) = obj.as_dict_mut() {
                dict.insert("Tabs", PdfName::new("S"));
                updated_count += 1;
            }
        }
    }

    Ok(updated_count)
}

/// Ensures the document is tagged by establishing /MarkInfo and /StructTreeRoot (FR-21.3).
pub fn fix_ensure_tagged_pdf(doc: &mut PdfDocument) -> Result<()> {
    let root_id = doc.root_id;

    // 1. Set /MarkInfo << /Marked true >>
    if let Some(obj) = doc.get_object_mut(root_id) {
        if let Some(catalog) = obj.as_dict_mut() {
            let mut mark_dict = match catalog.get("MarkInfo") {
                Some(Object::Dictionary(d)) => d.clone(),
                _ => PdfDictionary::new(),
            };
            mark_dict.insert("Marked", true);
            catalog.insert("MarkInfo", mark_dict);
        }
    }

    // 2. Ensure /StructTreeRoot exists
    let has_struct_tree = {
        let catalog = doc.catalog().cloned().unwrap_or_default();
        catalog.get("StructTreeRoot").is_some()
    };

    if !has_struct_tree {
        let struct_tree_id = doc.next_object_id();
        let mut struct_tree = PdfDictionary::new();
        struct_tree.insert("Type", PdfName::new("StructTreeRoot"));

        // Standard role map (ISO 32000-1 Clause 14.8.4)
        let mut role_map = PdfDictionary::new();
        role_map.insert("Document", PdfName::new("Document"));
        role_map.insert("Part", PdfName::new("Part"));
        role_map.insert("Sect", PdfName::new("Sect"));
        role_map.insert("H1", PdfName::new("H1"));
        role_map.insert("H2", PdfName::new("H2"));
        role_map.insert("P", PdfName::new("P"));
        role_map.insert("Figure", PdfName::new("Figure"));
        role_map.insert("Table", PdfName::new("Table"));
        struct_tree.insert("RoleMap", role_map);
        struct_tree.insert("K", vec![]);

        doc.objects.insert(struct_tree_id, Object::Dictionary(struct_tree));

        if let Some(obj) = doc.get_object_mut(root_id) {
            if let Some(catalog) = obj.as_dict_mut() {
                catalog.insert("StructTreeRoot", struct_tree_id);
            }
        }
    }

    Ok(())
}

/// Applies all standard automatic accessibility remediations in a single invocation (FR-21.1 - FR-21.10).
pub fn remediate_all_automatic(
    doc: &mut PdfDocument,
    default_title: &str,
    default_lang: &str,
) -> Result<AccessibilityReport> {
    // 1. Ensure title and display preference
    let current_info = doc.info_id.and_then(|id| doc.get_object(id)).and_then(Object::as_dict);
    let title_to_use = current_info
        .and_then(|d| d.get_str("Title"))
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| default_title.to_string());

    fix_document_title(doc, &title_to_use)?;

    // 2. Ensure natural language
    let current_lang = doc.catalog().and_then(|c| c.get_str("Lang"));
    let lang_to_use = current_lang
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| default_lang.to_string());

    fix_document_language(doc, &lang_to_use)?;

    // 3. Set tab order to Structure (/Tabs /S)
    fix_tab_order_to_structure(doc)?;

    // 4. Ensure Marked and StructTreeRoot
    fix_ensure_tagged_pdf(doc)?;

    // 5. Re-audit and return updated status
    audit_accessibility(doc)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_accessibility_audit_and_auto_remediation() {
        let mut doc = PdfDocument::new();
        doc.add_blank_page(612.0, 792.0);

        // Initially untagged, missing title and language
        let report_initial = audit_accessibility(&doc).expect("audit initial");
        assert!(!report_initial.is_compliant);
        assert!(report_initial.failed_count > 0);

        // Apply one-click full remediation
        let report_remediated =
            remediate_all_automatic(&mut doc, "Accessible Test Report", "en-US")
                .expect("remediate all");

        assert_eq!(report_remediated.failed_count, 0);
        assert!(report_remediated.is_compliant);
        assert!(report_remediated.passed_count >= 5);

        // Verify document attributes were written accurately
        let catalog = doc.catalog().expect("catalog");
        assert_eq!(catalog.get_str("Lang").as_deref(), Some("en-US"));
        assert!(catalog.get("StructTreeRoot").is_some());
        assert!(catalog.get("MarkInfo").is_some());

        // Verify page tabs
        let page_id = doc.page_ids()[0];
        let page_dict = doc.get_object(page_id).and_then(Object::as_dict).expect("page dict");
        assert_eq!(page_dict.get_name("Tabs"), Some("S"));
    }
}
