use cargo_packager_updater::semver::Version;

use super::state::UpdateFailure;

pub trait StagedUpdate: Send {
    fn version(&self) -> &Version;

    fn download(&self, on_progress: &dyn Fn(u64, Option<u64>)) -> Result<Vec<u8>, UpdateFailure>;

    fn install(&self, bytes: &[u8]) -> anyhow::Result<()>;
}

pub trait UpdateSource: Send + Sync + 'static {
    fn check(&self) -> Result<Option<Box<dyn StagedUpdate>>, UpdateFailure>;
}
