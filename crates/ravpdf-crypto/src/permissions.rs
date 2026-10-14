//! PDF Permissions Flags conforming to ISO 32000-1 Section 7.6.3.2 Table 22.
//!
//! Traceability: FR-14.4.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Permissions {
    pub flags: i32,
}

impl Default for Permissions {
    fn default() -> Self {
        Self::all_permitted()
    }
}

impl Permissions {
    /// Bit 3: Print document
    pub const PRINT: i32 = 1 << 2;
    /// Bit 4: Modify contents of document
    pub const MODIFY: i32 = 1 << 3;
    /// Bit 5: Copy or extract text/graphics
    pub const COPY: i32 = 1 << 4;
    /// Bit 6: Add or modify annotations
    pub const ANNOTATE: i32 = 1 << 5;
    /// Bit 9: Fill interactive form fields
    pub const FILL_FORMS: i32 = 1 << 8;
    /// Bit 10: Extract text/graphics for accessibility
    pub const ACCESSIBILITY: i32 = 1 << 9;
    /// Bit 11: Assemble document (insert, rotate, delete pages)
    pub const ASSEMBLE: i32 = 1 << 10;
    /// Bit 12: High-resolution printing
    pub const PRINT_HIGH_RES: i32 = 1 << 11;

    pub fn new(flags: i32) -> Self {
        Self { flags }
    }

    pub fn all_permitted() -> Self {
        Self { flags: -4 } // Bits 7, 8 and reserved bits set to 1 per ISO spec
    }

    pub fn can_print(&self) -> bool {
        (self.flags & Self::PRINT) != 0
    }

    pub fn can_modify(&self) -> bool {
        (self.flags & Self::MODIFY) != 0
    }

    pub fn can_copy(&self) -> bool {
        (self.flags & Self::COPY) != 0
    }

    pub fn can_annotate(&self) -> bool {
        (self.flags & Self::ANNOTATE) != 0
    }

    pub fn can_fill_forms(&self) -> bool {
        (self.flags & Self::FILL_FORMS) != 0
    }

    pub fn can_accessibility(&self) -> bool {
        (self.flags & Self::ACCESSIBILITY) != 0
    }
}
