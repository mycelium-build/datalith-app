/// The release tag stamped by release CI, if this is a release build.
pub const RELEASE_TAG: Option<&str> = option_env!("DATALITH_RELEASE_TAG");

/// The user-facing version of this build.
#[must_use]
pub fn version() -> &'static str {
    version_from_tag(RELEASE_TAG)
}

/// The release tag of this build, `v`-prefixed.
#[must_use]
pub const fn release_tag() -> &'static str {
    release_tag_from(RELEASE_TAG)
}

fn version_from_tag(tag: Option<&'static str>) -> &'static str {
    tag.map_or(env!("CARGO_PKG_VERSION"), |tag| {
        tag.strip_prefix('v').unwrap_or(tag)
    })
}

const fn release_tag_from(tag: Option<&'static str>) -> &'static str {
    match tag {
        Some(tag) => tag,
        None => concat!("v", env!("CARGO_PKG_VERSION")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_drops_the_v_prefix() {
        assert_eq!(version_from_tag(Some("v0.2.0")), "0.2.0");
        assert_eq!(version_from_tag(Some("v0.2.0-rc.1")), "0.2.0-rc.1");
    }

    #[test]
    fn version_tolerates_a_missing_prefix() {
        assert_eq!(version_from_tag(Some("0.2.0")), "0.2.0");
    }

    #[test]
    fn development_builds_fall_back_to_the_crate_version() {
        assert_eq!(version_from_tag(None), env!("CARGO_PKG_VERSION"));
        assert_eq!(
            release_tag_from(None),
            concat!("v", env!("CARGO_PKG_VERSION"))
        );
    }

    #[test]
    fn release_tag_keeps_the_prefix() {
        assert_eq!(release_tag_from(Some("v0.2.0")), "v0.2.0");
    }

    #[test]
    fn version_reflects_the_compile_time_stamp() {
        match RELEASE_TAG {
            Some(tag) => assert_eq!(version(), tag.trim_start_matches('v')),
            None => assert_eq!(version(), env!("CARGO_PKG_VERSION")),
        }
    }
}
