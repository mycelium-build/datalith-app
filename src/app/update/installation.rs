//! Linux packages share one binary; the `AppImage` runtime supplies `APPIMAGE`.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InstallationKind {
    SelfManaged,
    ExternallyManaged,
}

impl InstallationKind {
    #[must_use]
    pub fn detect() -> Self {
        #[cfg(any(target_os = "windows", target_os = "macos"))]
        {
            Self::SelfManaged
        }
        #[cfg(not(any(target_os = "windows", target_os = "macos")))]
        {
            if std::env::var_os("APPIMAGE").is_some() {
                Self::SelfManaged
            } else {
                Self::ExternallyManaged
            }
        }
    }

    #[must_use]
    pub const fn is_self_managed(self) -> bool {
        matches!(self, Self::SelfManaged)
    }
}

/// The website download page opened for externally managed installations.
pub const DOWNLOAD_PAGE_URL: &str = "https://mycelium-build.github.io/datalith/#download";
