use std::path::{Path, PathBuf};

use async_trait::async_trait;
use shared::{utils::BoxResult, version::version_metadata::VersionMetadata};

pub struct GeneratorResult {
    // ordered from parent to child
    pub metadata: Vec<VersionMetadata>,

    pub extra_libs_paths: Vec<PathBuf>,
}

#[async_trait]
pub trait VersionGenerator {
    async fn generate(&self, work_dir: &Path) -> BoxResult<GeneratorResult>;
}
