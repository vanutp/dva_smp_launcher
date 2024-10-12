use std::{
    error::Error,
    path::{Path, PathBuf},
};

use async_trait::async_trait;

pub struct GeneratorResult {
    pub id: String,
    pub extra_libs_paths: Vec<PathBuf>,
}

#[async_trait]
pub trait VersionGenerator {
    async fn generate(
        &self,
        output_dir: &Path,
        work_dir: &Path,
    ) -> Result<GeneratorResult, Box<dyn Error + Send + Sync>>;
}
