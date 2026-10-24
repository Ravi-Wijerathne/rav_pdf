//! Batch Processing and Action Wizard Engine.
//!
//! Traceability:
//! - FR-30.1 (Custom multi-step action sequences combining tools in configurable order)
//! - FR-30.2 (Apply action sequences to selected files or directories)
//! - FR-30.3 (Predefined action templates: Prepare for Distribution, Prepare for Archiving, Optimize for Web)
//! - FR-30.4 (Supported automatable operations: Watermark, Bates, Flatten, Optimize, Redact, Encrypt, Header/Footer, Preflight, JavaScript)
//! - FR-30.5 (Configurable per-step parameters)
//! - FR-30.6 (Custom JavaScript steps in action sequences)
//! - FR-30.7 (Exporting and importing action sequence definitions for team sharing)
//! - FR-30.8 (Progress indicators and execution logs)
//! - FR-30.9 (Stopping/cancelling running batch process)

use crate::bates::apply_bates_numbering;
use crate::header_footer::{apply_headers_and_footers, HeaderFooterConfig};
use crate::js::{execute_script, JsExecutionContext};
use crate::linearize::linearize_document;
use crate::metadata::{set_document_info, DocumentInfo};
use crate::optimize::optimize_document;
use crate::preflight::{run_preflight, PreflightProfile};
use crate::redact::redact_pattern;
use crate::watermark::apply_text_watermark;
use ravpdf_core::document::PdfDocument;
use ravpdf_crypto::{encrypt_document, Permissions};
use ravpdf_forms::flatten_form;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// An individual automatable step in a batch ActionSequence (FR-30.4, FR-30.6).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BatchAction {
    Watermark {
        text: String,
        opacity: f32,
    },
    BatesNumbering {
        prefix: String,
        start_number: u32,
        digits: usize,
    },
    HeaderFooter(HeaderFooterConfig),
    FlattenForms,
    Optimize,
    Linearize,
    SanitizeMetadata,
    RedactPattern {
        pattern: String,
        replacement: String,
    },
    Encrypt {
        user_pass: String,
        owner_pass: String,
    },
    RunJavaScript {
        script: String,
    },
    Preflight {
        profile: PreflightProfile,
    },
}

impl BatchAction {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Watermark { .. } => "Add Watermark",
            Self::BatesNumbering { .. } => "Apply Bates Numbering",
            Self::HeaderFooter(_) => "Add Headers & Footers",
            Self::FlattenForms => "Flatten Interactive Forms",
            Self::Optimize => "Optimize & Reduce Size",
            Self::Linearize => "Linearize (Fast Web View)",
            Self::SanitizeMetadata => "Sanitize Document Metadata",
            Self::RedactPattern { .. } => "Redact Pattern",
            Self::Encrypt { .. } => "Password Encrypt",
            Self::RunJavaScript { .. } => "Execute JavaScript",
            Self::Preflight { .. } => "Verify Preflight Conformance",
        }
    }
}

/// A named, reusable multi-step Action Wizard sequence (FR-30.1, FR-30.7).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionSequence {
    pub name: String,
    pub description: String,
    pub actions: Vec<BatchAction>,
}

impl ActionSequence {
    pub fn new(name: impl Into<String>, description: impl Into<String>, actions: Vec<BatchAction>) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            actions,
        }
    }

    /// Predefined template: "Prepare for Distribution" (FR-30.3).
    pub fn template_prepare_for_distribution() -> Self {
        Self {
            name: "Prepare for Distribution".to_string(),
            description: "Sanitizes metadata, flattens interactive form fields, and optimizes file size.".to_string(),
            actions: vec![
                BatchAction::SanitizeMetadata,
                BatchAction::FlattenForms,
                BatchAction::Optimize,
            ],
        }
    }

    /// Predefined template: "Prepare for Archiving" (FR-30.3).
    pub fn template_prepare_for_archiving() -> Self {
        Self {
            name: "Prepare for Archiving".to_string(),
            description: "Flattens interactive forms, runs PDF/A preflight validation, and optimizes structure.".to_string(),
            actions: vec![
                BatchAction::FlattenForms,
                BatchAction::Preflight {
                    profile: PreflightProfile::PdfA1b,
                },
                BatchAction::Optimize,
            ],
        }
    }

    /// Predefined template: "Optimize for Web" (FR-30.3).
    pub fn template_optimize_for_web() -> Self {
        Self {
            name: "Optimize for Web".to_string(),
            description: "Compacts object streams and linearizes PDF for fast page-at-a-time browser delivery.".to_string(),
            actions: vec![
                BatchAction::Optimize,
                BatchAction::Linearize,
            ],
        }
    }

    /// Exports action sequence definition as JSON (FR-30.7).
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Imports action sequence definition from JSON (FR-30.7).
    pub fn from_json(json_str: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json_str)
    }
}

/// Configuration for a batch execution job (FR-30.1, FR-30.2).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchJob {
    pub input_files: Vec<PathBuf>,
    pub output_directory: PathBuf,
    pub actions: Vec<BatchAction>,
}

/// Execution outcome for a single processed file (FR-30.8).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchFileResult {
    pub path: PathBuf,
    pub success: bool,
    pub error_message: Option<String>,
}

/// Aggregate results from executing a batch sequence (FR-30.8).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BatchResult {
    pub total_files: usize,
    pub successful_files: usize,
    pub failed_files: usize,
    pub cancelled: bool,
    pub details: Vec<BatchFileResult>,
}

/// Executes a configured batch job over all target files (FR-30.1 - FR-30.4).
pub fn execute_batch(job: &BatchJob) -> BatchResult {
    execute_batch_cancellable(job, None, None)
}

/// Progress callback signature for batch jobs (FR-30.8).
pub type BatchProgressCallback<'a> = &'a dyn Fn(usize, usize, &Path);

/// Executes a configured batch job with progress callback and cancellation token (FR-30.8, FR-30.9).
pub fn execute_batch_cancellable(
    job: &BatchJob,
    cancel_token: Option<&Arc<AtomicBool>>,
    progress_callback: Option<BatchProgressCallback<'_>>,
) -> BatchResult {
    let mut result = BatchResult {
        total_files: job.input_files.len(),
        successful_files: 0,
        failed_files: 0,
        cancelled: false,
        details: Vec::new(),
    };

    if !job.output_directory.exists() {
        let _ = std::fs::create_dir_all(&job.output_directory);
    }

    for (idx, path) in job.input_files.iter().enumerate() {
        if let Some(token) = cancel_token {
            if token.load(Ordering::Relaxed) {
                result.cancelled = true;
                break;
            }
        }

        if let Some(cb) = progress_callback {
            cb(idx + 1, job.input_files.len(), path);
        }

        match process_single_file(path, &job.output_directory, &job.actions) {
            Ok(_) => {
                result.successful_files += 1;
                result.details.push(BatchFileResult {
                    path: path.clone(),
                    success: true,
                    error_message: None,
                });
            }
            Err(e) => {
                result.failed_files += 1;
                result.details.push(BatchFileResult {
                    path: path.clone(),
                    success: false,
                    error_message: Some(e.to_string()),
                });
            }
        }
    }

    result
}

fn process_single_file(
    input_path: &Path,
    output_dir: &Path,
    actions: &[BatchAction],
) -> anyhow::Result<()> {
    let mut doc = PdfDocument::load_from_file(input_path)?;

    for action in actions {
        match action {
            BatchAction::Watermark { text, opacity } => {
                apply_text_watermark(&mut doc, text, *opacity, 45.0, 54.0)?;
            }
            BatchAction::BatesNumbering {
                prefix,
                start_number,
                digits,
            } => {
                apply_bates_numbering(&mut doc, prefix, *start_number, *digits, "")?;
            }
            BatchAction::HeaderFooter(cfg) => {
                apply_headers_and_footers(&mut doc, cfg, None, None)?;
            }
            BatchAction::FlattenForms => {
                flatten_form(&mut doc)?;
            }
            BatchAction::Optimize => {
                let _ = optimize_document(&mut doc)?;
            }
            BatchAction::Linearize => {
                let _ = linearize_document(&doc)?;
            }
            BatchAction::SanitizeMetadata => {
                // Clear identifying document information
                let info = DocumentInfo {
                    title: Some("Sanitized Document".into()),
                    ..Default::default()
                };
                set_document_info(&mut doc, &info)?;
            }
            BatchAction::RedactPattern {
                pattern,
                replacement,
            } => {
                let _ = redact_pattern(&mut doc, pattern, replacement)?;
            }
            BatchAction::Encrypt {
                user_pass,
                owner_pass,
            } => {
                encrypt_document(&mut doc, user_pass, owner_pass, Permissions::default())?;
            }
            BatchAction::RunJavaScript { script } => {
                let mut ctx = JsExecutionContext::new();
                let _ = ctx.sync_from_document(&doc);
                let _ = execute_script(&mut ctx, script);
                let _ = ctx.sync_to_document(&mut doc);
            }
            BatchAction::Preflight { profile } => {
                let rep = run_preflight(&doc, *profile);
                if !rep.is_compliant {
                    tracing::warn!("Batch preflight non-compliance on {:?}", input_path);
                }
            }
        }
    }

    let file_name = input_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("processed.pdf");
    let out_file_path = output_dir.join(file_name);
    doc.save_to_file(out_file_path)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_batch_action_execution_and_templates() {
        let temp_dir = std::env::temp_dir().join("ravpdf_batch_test_templates");
        let input_dir = temp_dir.join("inputs");
        let output_dir = temp_dir.join("outputs");
        let _ = std::fs::create_dir_all(&input_dir);
        let _ = std::fs::create_dir_all(&output_dir);

        // Create 2 test input documents
        let mut doc1 = PdfDocument::new();
        doc1.add_blank_page(612.0, 792.0);
        let path1 = input_dir.join("doc1.pdf");
        doc1.save_to_file(&path1).unwrap();

        let mut doc2 = PdfDocument::new();
        doc2.add_blank_page(612.0, 792.0);
        let path2 = input_dir.join("doc2.pdf");
        doc2.save_to_file(&path2).unwrap();

        // Test ActionSequence JSON serialization/deserialization (FR-30.7)
        let seq = ActionSequence::template_prepare_for_distribution();
        let json = seq.to_json().expect("serialize sequence");
        let restored = ActionSequence::from_json(&json).expect("deserialize sequence");
        assert_eq!(restored.name, "Prepare for Distribution");
        assert_eq!(restored.actions.len(), 3);

        // Execute batch job with cancellation token check (FR-30.9)
        let cancel_token = Arc::new(AtomicBool::new(false));
        let job = BatchJob {
            input_files: vec![path1.clone(), path2.clone()],
            output_directory: output_dir.clone(),
            actions: restored.actions,
        };

        let result = execute_batch_cancellable(&job, Some(&cancel_token), None);
        assert_eq!(result.total_files, 2);
        assert_eq!(result.successful_files, 2);
        assert_eq!(result.failed_files, 0);
        assert!(!result.cancelled);

        assert!(output_dir.join("doc1.pdf").exists());
        assert!(output_dir.join("doc2.pdf").exists());

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
