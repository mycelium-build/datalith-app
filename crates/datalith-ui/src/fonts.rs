use gpui_kit::{Global, SharedString};

#[derive(Clone)]
pub struct ResolvedFonts {
    pub interface: SharedString,
    pub reading: SharedString,
    pub headings: SharedString,
    pub code: SharedString,
}

impl ResolvedFonts {
    #[must_use]
    pub const fn interface(&self) -> &SharedString {
        &self.interface
    }

    #[must_use]
    pub const fn reading(&self) -> &SharedString {
        &self.reading
    }

    #[must_use]
    pub const fn headings(&self) -> &SharedString {
        &self.headings
    }

    #[must_use]
    pub const fn code(&self) -> &SharedString {
        &self.code
    }
}

impl Global for ResolvedFonts {}
