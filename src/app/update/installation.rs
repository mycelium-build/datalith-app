//! Linux packages share one binary; the `AppImage` runtime supplies `APPIMAGE`.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InstallationKind {
    SelfManaged,
    #[cfg(any(test, not(any(target_os = "windows", target_os = "macos"))))]
    ExternallyManaged,
}

impl InstallationKind {
    #[cfg(any(target_os = "windows", target_os = "macos"))]
    #[must_use]
    pub const fn detect() -> Self {
        Self::SelfManaged
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    #[must_use]
    pub fn detect() -> Self {
        if std::env::var_os("APPIMAGE").is_some() {
            Self::SelfManaged
        } else {
            Self::ExternallyManaged
        }
    }

    #[must_use]
    pub const fn is_self_managed(self) -> bool {
        matches!(self, Self::SelfManaged)
    }
}

/// The website download page opened for externally managed installations.
pub const DOWNLOAD_PAGE_URL: &str = "https://mycelium-build.github.io/datalith/#download";
