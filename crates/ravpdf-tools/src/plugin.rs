//! RavPDF Plugin & Extension Architecture conforming to ISO 32000-1 and Acrobat Plugin SDK.
//!
//! Traceability:
//! - FR-36.1: Public Rust trait-based plugin API for third-party extensions.
//! - FR-36.2: Dynamic and registry-based plugin loading at startup.
//! - FR-36.3: Capability-based security permissions and sandboxing.
//! - FR-36.4: UI and document hooks: menus, toolbar buttons, context menus, lifecycle events.
//! - FR-36.5: Plugin versioning, metadata manifests, and compatibility validation.
//! - FR-36.6: Sandboxing and permission enforcement against unauthorized access.
//! - FR-36.7: Sample plugins and SDK templates.

use anyhow::{anyhow, Result};
use ravpdf_core::document::PdfDocument;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Capability permissions that can be requested by plugins (FR-36.3, FR-36.6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PluginPermission {
    FileRead,
    FileWrite,
    NetworkAccess,
    UiExtension,
    DocumentTransform,
    SystemInfo,
}

/// Metadata manifest describing a plugin (FR-36.5, FR-36.7).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PluginManifest {
    pub id: String,
    pub name: String,
    pub version: String,
    pub author: String,
    pub description: String,
    pub min_ravpdf_version: String,
    pub requested_permissions: Vec<PluginPermission>,
}

impl PluginManifest {
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        version: impl Into<String>,
        author: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            version: version.into(),
            author: author.into(),
            description: String::new(),
            min_ravpdf_version: "0.1.0".to_string(),
            requested_permissions: Vec::new(),
        }
    }
}

/// Top-level parent menu for plugin UI contributions (FR-36.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MenuCategory {
    File,
    Edit,
    View,
    Tools,
    Help,
}

/// Menu item contributed by a plugin (FR-36.4).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PluginMenuItem {
    pub id: String,
    pub plugin_id: String,
    pub parent_menu: MenuCategory,
    pub label: String,
    pub shortcut: Option<String>,
    pub action_id: String,
}

/// Toolbar button contributed by a plugin (FR-36.4).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PluginToolbarButton {
    pub id: String,
    pub plugin_id: String,
    pub tooltip: String,
    pub icon_symbol: String,
    pub action_id: String,
}

/// Context menu target type (FR-36.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContextMenuTarget {
    Page,
    TextSelection,
    Annotation,
    Image,
}

/// Context menu item contributed by a plugin (FR-36.4).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PluginContextMenuEntry {
    pub id: String,
    pub plugin_id: String,
    pub target: ContextMenuTarget,
    pub label: String,
    pub action_id: String,
}

/// Host environment interface provided to plugins during registration and lifecycle hooks (FR-36.4, FR-36.6).
#[derive(Debug, Default)]
pub struct PluginHost {
    pub registered_menus: Vec<PluginMenuItem>,
    pub registered_toolbars: Vec<PluginToolbarButton>,
    pub registered_context_menus: Vec<PluginContextMenuEntry>,
}

impl PluginHost {
    pub fn new() -> Self {
        Self::default()
    }

    /// Contributes a menu item to the main application UI (FR-36.4).
    pub fn add_menu_item(
        &mut self,
        plugin_id: &str,
        parent: MenuCategory,
        label: &str,
        shortcut: Option<&str>,
        action_id: &str,
    ) {
        self.registered_menus.push(PluginMenuItem {
            id: format!("{}:menu:{}", plugin_id, action_id),
            plugin_id: plugin_id.to_string(),
            parent_menu: parent,
            label: label.to_string(),
            shortcut: shortcut.map(|s| s.to_string()),
            action_id: action_id.to_string(),
        });
    }

    /// Contributes a toolbar button to the application toolbar (FR-36.4).
    pub fn add_toolbar_button(
        &mut self,
        plugin_id: &str,
        tooltip: &str,
        icon_symbol: &str,
        action_id: &str,
    ) {
        self.registered_toolbars.push(PluginToolbarButton {
            id: format!("{}:tb:{}", plugin_id, action_id),
            plugin_id: plugin_id.to_string(),
            tooltip: tooltip.to_string(),
            icon_symbol: icon_symbol.to_string(),
            action_id: action_id.to_string(),
        });
    }

    /// Contributes a context menu item (FR-36.4).
    pub fn add_context_menu_item(
        &mut self,
        plugin_id: &str,
        target: ContextMenuTarget,
        label: &str,
        action_id: &str,
    ) {
        self.registered_context_menus.push(PluginContextMenuEntry {
            id: format!("{}:ctx:{}", plugin_id, action_id),
            plugin_id: plugin_id.to_string(),
            target,
            label: label.to_string(),
            action_id: action_id.to_string(),
        });
    }
}

/// Public Trait that all RavPDF plugins must implement (FR-36.1).
pub trait RavPdfPlugin: Send + Sync {
    /// Return the metadata manifest describing this plugin (FR-36.5).
    fn manifest(&self) -> PluginManifest;

    /// Invoked when the plugin is loaded into the host environment (FR-36.2).
    fn on_load(&mut self, host: &mut PluginHost) -> Result<()>;

    /// Invoked when a document is opened (FR-36.4).
    fn on_document_open(&mut self, _doc: &PdfDocument) -> Result<()> {
        Ok(())
    }

    /// Invoked before saving a document (FR-36.4).
    fn on_document_save(&mut self, _doc: &mut PdfDocument) -> Result<()> {
        Ok(())
    }

    /// Invoked when a plugin menu item, button, or hook action is triggered (FR-36.4).
    fn on_action_triggered(
        &mut self,
        action_id: &str,
        doc: Option<&mut PdfDocument>,
    ) -> Result<Option<String>>;

    /// Invoked when the plugin is unloaded (FR-36.2).
    fn on_unload(&mut self) -> Result<()> {
        Ok(())
    }
}

/// Runtime state and permission record for an active plugin (FR-36.3, FR-36.5).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginInfo {
    pub manifest: PluginManifest,
    pub enabled: bool,
    pub granted_permissions: Vec<PluginPermission>,
}

/// Central registry managing plugin lifecycles, hooks, and security enforcement (FR-36.1 - FR-36.6).
pub struct PluginRegistry {
    plugins: HashMap<String, Box<dyn RavPdfPlugin>>,
    plugin_info: HashMap<String, PluginInfo>,
    host: PluginHost,
}

impl Default for PluginRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl PluginRegistry {
    pub fn new() -> Self {
        Self {
            plugins: HashMap::new(),
            plugin_info: HashMap::new(),
            host: PluginHost::new(),
        }
    }

    /// Registers and initializes a plugin with granted permissions (FR-36.1, FR-36.3, FR-36.6).
    pub fn register_plugin(
        &mut self,
        mut plugin: Box<dyn RavPdfPlugin>,
        granted_permissions: &[PluginPermission],
    ) -> Result<()> {
        let manifest = plugin.manifest();
        let plugin_id = manifest.id.clone();

        // 1. Compatibility verification (FR-36.5)
        if !is_version_compatible(&manifest.min_ravpdf_version, "0.1.0") {
            return Err(anyhow!(
                "Plugin '{}' requires RavPDF version >= {}, incompatible with current 0.1.0",
                manifest.name,
                manifest.min_ravpdf_version
            ));
        }

        // 2. Security validation: Verify requested permissions against granted permissions (FR-36.6)
        for req in &manifest.requested_permissions {
            if !granted_permissions.contains(req) {
                return Err(anyhow!(
                    "Plugin '{}' requested permission '{:?}' which was not granted by policy",
                    manifest.name,
                    req
                ));
            }
        }

        // 3. Initialize plugin on host
        plugin.on_load(&mut self.host)?;

        let info = PluginInfo {
            manifest,
            enabled: true,
            granted_permissions: granted_permissions.to_vec(),
        };

        self.plugin_info.insert(plugin_id.clone(), info);
        self.plugins.insert(plugin_id, plugin);

        Ok(())
    }

    /// Enable or disable a registered plugin (FR-36.2).
    pub fn set_plugin_enabled(&mut self, plugin_id: &str, enabled: bool) -> Result<()> {
        if let Some(info) = self.plugin_info.get_mut(plugin_id) {
            info.enabled = enabled;
            Ok(())
        } else {
            Err(anyhow!("Plugin '{}' not found", plugin_id))
        }
    }

    /// List all registered plugins and their statuses (FR-36.5).
    pub fn list_plugins(&self) -> Vec<PluginInfo> {
        self.plugin_info.values().cloned().collect()
    }

    /// Get registered menu items contributed by enabled plugins (FR-36.4).
    pub fn get_active_menu_items(&self) -> Vec<&PluginMenuItem> {
        self.host
            .registered_menus
            .iter()
            .filter(|m| {
                self.plugin_info
                    .get(&m.plugin_id)
                    .map(|info| info.enabled)
                    .unwrap_or(false)
            })
            .collect()
    }

    /// Get registered toolbar buttons contributed by enabled plugins (FR-36.4).
    pub fn get_active_toolbar_buttons(&self) -> Vec<&PluginToolbarButton> {
        self.host
            .registered_toolbars
            .iter()
            .filter(|b| {
                self.plugin_info
                    .get(&b.plugin_id)
                    .map(|info| info.enabled)
                    .unwrap_or(false)
            })
            .collect()
    }

    /// Dispatch action to target plugin (FR-36.4).
    pub fn dispatch_action(
        &mut self,
        plugin_id: &str,
        action_id: &str,
        doc: Option<&mut PdfDocument>,
    ) -> Result<Option<String>> {
        if let Some(info) = self.plugin_info.get(plugin_id) {
            if !info.enabled {
                return Err(anyhow!("Plugin '{}' is currently disabled", plugin_id));
            }
        } else {
            return Err(anyhow!("Plugin '{}' not found", plugin_id));
        }

        if let Some(plugin) = self.plugins.get_mut(plugin_id) {
            plugin.on_action_triggered(action_id, doc)
        } else {
            Err(anyhow!("Plugin instance '{}' not found", plugin_id))
        }
    }

    /// Notify all enabled plugins that a document was opened (FR-36.4).
    pub fn notify_document_open(&mut self, doc: &PdfDocument) -> Result<()> {
        for (id, plugin) in self.plugins.iter_mut() {
            if let Some(info) = self.plugin_info.get(id) {
                if info.enabled {
                    plugin.on_document_open(doc)?;
                }
            }
        }
        Ok(())
    }

    /// Notify all enabled plugins before saving a document (FR-36.4).
    pub fn notify_document_save(&mut self, doc: &mut PdfDocument) -> Result<()> {
        for (id, plugin) in self.plugins.iter_mut() {
            if let Some(info) = self.plugin_info.get(id) {
                if info.enabled {
                    plugin.on_document_save(doc)?;
                }
            }
        }
        Ok(())
    }
}

/// Helper function to validate semver compatibility (FR-36.5).
fn is_version_compatible(required: &str, current: &str) -> bool {
    let req_parts: Vec<u32> = required
        .split('.')
        .filter_map(|s| s.parse().ok())
        .collect();
    let cur_parts: Vec<u32> = current
        .split('.')
        .filter_map(|s| s.parse().ok())
        .collect();

    if req_parts.len() < 2 || cur_parts.len() < 2 {
        return true;
    }

    // Major version match
    if req_parts[0] > cur_parts[0] {
        return false;
    }
    if req_parts[0] == cur_parts[0] && req_parts[1] > cur_parts[1] {
        return false;
    }
    true
}

// ============================================================================
// Sample Reference Plugin Implementation (FR-36.7)
// ============================================================================

/// Sample third-party plugin: Document Watermark & Statistics Helper (FR-36.7).
pub struct SampleWatermarkPlugin {
    pub call_count: usize,
}

impl Default for SampleWatermarkPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl SampleWatermarkPlugin {
    pub fn new() -> Self {
        Self { call_count: 0 }
    }
}

impl RavPdfPlugin for SampleWatermarkPlugin {
    fn manifest(&self) -> PluginManifest {
        let mut m = PluginManifest::new(
            "org.ravpdf.sample-watermark",
            "Sample Watermark & Stats Plugin",
            "1.0.0",
            "RavPDF Extension Authors",
        );
        m.description = "Demonstrates menu, toolbar, and document modification hooks".to_string();
        m.requested_permissions = vec![
            PluginPermission::UiExtension,
            PluginPermission::DocumentTransform,
        ];
        m
    }

    fn on_load(&mut self, host: &mut PluginHost) -> Result<()> {
        let id = "org.ravpdf.sample-watermark";
        host.add_menu_item(
            id,
            MenuCategory::Tools,
            "Apply Sample Draft Watermark",
            Some("Ctrl+Shift+W"),
            "apply_draft_watermark",
        );
        host.add_toolbar_button(id, "Apply Sample Watermark", "W", "apply_draft_watermark");
        host.add_context_menu_item(
            id,
            ContextMenuTarget::Page,
            "Watermark This Page",
            "apply_draft_watermark",
        );
        Ok(())
    }

    fn on_action_triggered(
        &mut self,
        action_id: &str,
        doc: Option<&mut PdfDocument>,
    ) -> Result<Option<String>> {
        self.call_count += 1;
        if action_id == "apply_draft_watermark" {
            if let Some(doc) = doc {
                crate::watermark::apply_text_watermark(doc, "SAMPLE DRAFT", 0.25, 45.0, 54.0)?;
                Ok(Some(format!(
                    "Applied sample draft watermark across {} pages",
                    doc.page_count()
                )))
            } else {
                Ok(Some("No document open to watermark".to_string()))
            }
        } else {
            Ok(None)
        }
    }
}

// ============================================================================
// Unit Tests (FR-36.1 - FR-36.7)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_registration_and_ui_hooks() {
        let mut registry = PluginRegistry::new();
        let plugin = Box::new(SampleWatermarkPlugin::new());

        let permissions = [
            PluginPermission::UiExtension,
            PluginPermission::DocumentTransform,
        ];

        registry
            .register_plugin(plugin, &permissions)
            .expect("register plugin");

        let plugins = registry.list_plugins();
        assert_eq!(plugins.len(), 1);
        assert_eq!(plugins[0].manifest.name, "Sample Watermark & Stats Plugin");
        assert!(plugins[0].enabled);

        let menus = registry.get_active_menu_items();
        assert_eq!(menus.len(), 1);
        assert_eq!(menus[0].label, "Apply Sample Draft Watermark");

        let toolbars = registry.get_active_toolbar_buttons();
        assert_eq!(toolbars.len(), 1);
        assert_eq!(toolbars[0].icon_symbol, "W");
    }

    #[test]
    fn test_plugin_permission_security_enforcement() {
        let mut registry = PluginRegistry::new();
        let plugin = Box::new(SampleWatermarkPlugin::new());

        // Grant only UiExtension, missing DocumentTransform
        let permissions = [PluginPermission::UiExtension];

        let err = registry.register_plugin(plugin, &permissions);
        assert!(err.is_err());
        let err_str = err.unwrap_err().to_string();
        assert!(err_str.contains("requested permission"));
    }

    #[test]
    fn test_plugin_action_dispatch_and_doc_transform() {
        let mut registry = PluginRegistry::new();
        let plugin = Box::new(SampleWatermarkPlugin::new());
        let permissions = [
            PluginPermission::UiExtension,
            PluginPermission::DocumentTransform,
        ];

        registry
            .register_plugin(plugin, &permissions)
            .expect("register plugin");

        let mut doc = PdfDocument::new();
        doc.add_blank_page(612.0, 792.0);

        let res = registry
            .dispatch_action(
                "org.ravpdf.sample-watermark",
                "apply_draft_watermark",
                Some(&mut doc),
            )
            .expect("dispatch action");

        assert!(res.is_some());
        assert!(res.unwrap().contains("Applied sample draft watermark across 1 pages"));
    }
}
