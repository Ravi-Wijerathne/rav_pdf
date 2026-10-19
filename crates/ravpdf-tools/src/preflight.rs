//! Preflight Analysis and Standards Conformance Validation Engine.
//!
//! Traceability:
//! - FR-22.4 (Verify/validate PDF documents against standard profiles and generate conformance reports)
//! - FR-23.1.1 (Preflight engine analyzing PDFs against predefined rule profiles)
//! - FR-23.1.2 (Check for: fonts, encryption, metadata, stream compression integrity, color spaces)
//! - FR-23.1.3 (Detailed reports with pass/fail/warning indicators)

use ravpdf_core::document::PdfDocument;
use ravpdf_core::object::Object;
use serde::{Deserialize, Serialize};

/// Preflight profile defining the validation target (FR-23.1.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PreflightProfile {
    PdfA1b,
    PrintProduction,
    GeneralQuality,
}

impl PreflightProfile {
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::PdfA1b => "PDF/A-1b (ISO 19005-1 Archival Conformance)",
            Self::PrintProduction => "Print Production & Commercial Prepress",
            Self::GeneralQuality => "General Document Quality & Integrity",
        }
    }
}

/// Severity level of a preflight check item (FR-23.1.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PreflightSeverity {
    Info,
    Warning,
    Error,
}

/// A single check executed during preflight analysis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreflightCheck {
    pub id: String,
    pub category: String,
    pub title: String,
    pub severity: PreflightSeverity,
    pub passed: bool,
    pub details: String,
}

/// Comprehensive preflight validation report (FR-22.4, FR-23.1.3).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreflightReport {
    pub profile: PreflightProfile,
    pub checks: Vec<PreflightCheck>,
    pub total_passed: usize,
    pub total_warnings: usize,
    pub total_errors: usize,
    pub is_compliant: bool,
}

impl PreflightReport {
    pub fn new(profile: PreflightProfile) -> Self {
        Self {
            profile,
            checks: Vec::new(),
            total_passed: 0,
            total_warnings: 0,
            total_errors: 0,
            is_compliant: true,
        }
    }

    pub fn add_check(&mut self, check: PreflightCheck) {
        if check.passed {
            self.total_passed += 1;
        } else {
            match check.severity {
                PreflightSeverity::Error => {
                    self.total_errors += 1;
                    self.is_compliant = false;
                }
                PreflightSeverity::Warning => {
                    self.total_warnings += 1;
                }
                PreflightSeverity::Info => {}
            }
        }
        self.checks.push(check);
    }
}

/// Executes preflight checks on a document against the specified profile (FR-23.1.1).
pub fn run_preflight(doc: &PdfDocument, profile: PreflightProfile) -> PreflightReport {
    let mut report = PreflightReport::new(profile);

    // Rule 1: PDF Specification Version Check
    let (major, minor) = doc.version;
    let version_str = format!("%PDF-{}.{}", major, minor);
    let version_ok = major == 1 && minor <= 7;
    report.add_check(PreflightCheck {
        id: "RULE-SPEC-01".into(),
        category: "Specification".into(),
        title: "PDF Version Header".into(),
        severity: PreflightSeverity::Error,
        passed: version_ok,
        details: if version_ok {
            format!("Document complies with ISO standard header ({})", version_str)
        } else {
            format!("Unrecognized or non-standard version header: {}", version_str)
        },
    });

    // Rule 2: Security & Encryption Check (PDF/A disallows standard encryption)
    let is_encrypted = doc.trailer.contains_key("Encrypt")
        || doc
            .objects
            .values()
            .any(|o| o.as_dict().and_then(|d| d.get_name("Filter")) == Some("Standard"));

    if profile == PreflightProfile::PdfA1b {
        report.add_check(PreflightCheck {
            id: "RULE-SEC-01".into(),
            category: "Security".into(),
            title: "PDF/A Encryption Restriction".into(),
            severity: PreflightSeverity::Error,
            passed: !is_encrypted,
            details: if !is_encrypted {
                "Document is unencrypted (required for PDF/A long-term preservation)".into()
            } else {
                "PDF/A strictly prohibits encryption; passwords must be removed".into()
            },
        });
    } else {
        report.add_check(PreflightCheck {
            id: "RULE-SEC-02".into(),
            category: "Security".into(),
            title: "Encryption Handler".into(),
            severity: PreflightSeverity::Info,
            passed: true,
            details: if is_encrypted {
                "Document is password-protected with encryption".into()
            } else {
                "Document is unencrypted".into()
            },
        });
    }

    // Rule 3: Document Title & Metadata Completeness (FR-33)
    let mut title_found = false;
    let mut creator_found = false;
    if let Some(info_id) = doc.info_id {
        if let Some(info_dict) = doc.get_object(info_id).and_then(Object::as_dict) {
            title_found = info_dict.get_str("Title").is_some_and(|t| !t.trim().is_empty());
            creator_found = info_dict.get_str("Producer").or_else(|| info_dict.get_str("Creator")).is_some();
        }
    }

    report.add_check(PreflightCheck {
        id: "RULE-META-01".into(),
        category: "Metadata".into(),
        title: "Document Title".into(),
        severity: if profile == PreflightProfile::PdfA1b {
            PreflightSeverity::Error
        } else {
            PreflightSeverity::Warning
        },
        passed: title_found,
        details: if title_found {
            "Document Title is properly populated in /Info dictionary".into()
        } else {
            "Document Title is missing; required for PDF/A and accessible reading".into()
        },
    });

    report.add_check(PreflightCheck {
        id: "RULE-META-02".into(),
        category: "Metadata".into(),
        title: "Producer / Creator Identification".into(),
        severity: PreflightSeverity::Warning,
        passed: creator_found,
        details: if creator_found {
            "Producer / Creator application is recorded in document metadata".into()
        } else {
            "Producer metadata missing; recommended for provenance traceability".into()
        },
    });

    // Rule 4: Page Tree & Content Stream Integrity
    let page_count = doc.page_count();
    let has_pages = page_count > 0;
    report.add_check(PreflightCheck {
        id: "RULE-STRUCT-01".into(),
        category: "Page Structure".into(),
        title: "Page Tree Validity".into(),
        severity: PreflightSeverity::Error,
        passed: has_pages,
        details: if has_pages {
            format!("Page tree is valid with {} page(s)", page_count)
        } else {
            "Document has zero pages or invalid /Pages hierarchy".into()
        },
    });

    // Rule 5: Stream Decoding & Compression
    let mut decode_errors = 0;
    let mut uncompressed_streams = 0;
    for obj in doc.objects.values() {
        if let Object::Stream(s) = obj {
            if s.decode().is_err() {
                decode_errors += 1;
            }
            if s.dict.get("Filter").is_none() {
                uncompressed_streams += 1;
            }
        }
    }

    report.add_check(PreflightCheck {
        id: "RULE-STREAM-01".into(),
        category: "Streams".into(),
        title: "Content Stream Decodability".into(),
        severity: PreflightSeverity::Error,
        passed: decode_errors == 0,
        details: if decode_errors == 0 {
            "All stream filters (Flate, ASCII85, etc.) decoded successfully without corruption".into()
        } else {
            format!("{} stream(s) failed decoding; corrupt or unsupported filters present", decode_errors)
        },
    });

    if profile == PreflightProfile::PrintProduction || profile == PreflightProfile::GeneralQuality {
        report.add_check(PreflightCheck {
            id: "RULE-OPT-01".into(),
            category: "Optimization".into(),
            title: "Stream Compression (Flate)".into(),
            severity: PreflightSeverity::Warning,
            passed: uncompressed_streams == 0,
            details: if uncompressed_streams == 0 {
                "All streams are compressed to minimize file size".into()
            } else {
                format!("{} uncompressed stream(s) found; consider running 'Reduce File Size'", uncompressed_streams)
            },
        });
    }

    // Rule 6: Print Production & Color Spaces
    if profile == PreflightProfile::PrintProduction {
        let mut has_resources = true;
        for i in 0..page_count {
            if let Some(p) = doc.get_page(i) {
                if p.resources.is_none() {
                    has_resources = false;
                    break;
                }
            }
        }

        report.add_check(PreflightCheck {
            id: "RULE-PRINT-01".into(),
            category: "Print".into(),
            title: "Resource Dictionary Completeness".into(),
            severity: PreflightSeverity::Warning,
            passed: has_resources,
            details: if has_resources {
                "Every page contains a resolved /Resources dictionary".into()
            } else {
                "Some pages rely on inherited resources; explicit resources recommended for prepress".into()
            },
        });
    }

    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metadata::{set_document_info, DocumentInfo};

    #[test]
    fn test_preflight_checks() {
        let mut doc = PdfDocument::new();
        doc.add_blank_page(612.0, 792.0);

        // Before adding title metadata, PDF/A check should fail on title
        let rep_before = run_preflight(&doc, PreflightProfile::PdfA1b);
        assert!(!rep_before.is_compliant);
        assert!(rep_before.total_errors > 0);

        // Add required title & creator metadata
        let mut info = DocumentInfo::default();
        info.title = Some("Annual Report 2026".into());
        info.creator = Some("RavPDF Standards Engine".into());
        set_document_info(&mut doc, &info).expect("set doc info");

        let rep_after = run_preflight(&doc, PreflightProfile::PdfA1b);
        assert!(rep_after.is_compliant);
        assert_eq!(rep_after.total_errors, 0);
        assert!(rep_after.total_passed >= 4);
    }
}
