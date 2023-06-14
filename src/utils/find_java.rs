use std::path::{Path, PathBuf};
#[cfg(target_os = "windows")]
use registry::{Hive, Security};
use semver::Version;

struct JavaInstallation {
    version: Version,
    path: PathBuf,
}

#[cfg(target_os = "linux")]
pub fn find_java() -> Option<PathBuf> {
    Some(Path::new("/").to_path_buf())
}

#[cfg(target_os = "windows")]
fn find_java_from_registry_key(base_dir: &str, subdir_suffix: &str, java_path_key: &str) -> Vec<JavaInstallation> {
    let regkey = Hive::LocalMachine.open(base_dir, Security::Read);
    let mut res = Vec::new();
    res
}

#[cfg(target_os = "windows")]
pub fn find_java() -> Option<PathBuf> {
    find_java_from_registry_key(r"SOFTWARE\AdoptOpenJDK\JRE", r"\hotspot\MSI", "Path");
    Some(Path::new("/").to_path_buf())
}

#[cfg(target_os = "macos")]
pub fn find_java() -> Option<PathBuf> {
    Some(Path::new("/").to_path_buf())
}
