use std::{
    collections::HashMap,
    error::Error,
    path::{Path, PathBuf},
};

use serde::Deserialize;
use tokio::{fs, io::AsyncReadExt as _};

use crate::{
    files::{self, CheckDownloadEntry},
    progress,
};

use super::{overrides, version_manifest::VersionInfo};

fn get_os_name() -> String {
    if cfg!(windows) {
        "windows".to_string()
    } else if cfg!(target_os = "macos") {
        "osx".to_string()
    } else if cfg!(target_os = "linux") {
        "linux".to_string()
    } else {
        unimplemented!("Unsupported OS");
    }
}

fn get_system_arch() -> String {
    match std::env::consts::ARCH {
        "aarch64" => "arm64",
        "arm" => "arm32",
        arch => arch,
    }
    .to_string()
}

fn get_arch_os_name() -> String {
    get_os_name()
        + match get_system_arch().as_str() {
            "arm32" => "-arm32",
            "arm64" => "-arm64",
            _ => "",
        }
}

#[derive(Deserialize, Clone)]
struct Os {
    name: Option<String>,
    arch: Option<String>,
}

impl Os {
    pub(in crate::modpack) fn matches(&self) -> bool {
        let os_name = get_os_name();
        let os_arch = get_system_arch();

        if let Some(self_arch) = &self.arch {
            if self_arch != &os_arch {
                return false;
            }
        }
        if let Some(self_name) = &self.name {
            if self_name != &os_name && self_name != &format!("{}-{}", os_name, os_arch) {
                return false;
            }
        }

        true
    }
}

#[derive(Deserialize, Clone)]
pub struct Rule {
    action: String,
    os: Option<Os>,
    features: Option<HashMap<String, bool>>,
}

#[derive(Deserialize, Clone)]
#[serde(untagged)]
pub enum ArgumentValue {
    String(String),
    Array(Vec<String>),
}

impl ArgumentValue {
    pub fn get_values(&self) -> Vec<&str> {
        match self {
            ArgumentValue::String(s) => vec![s.as_str()],
            ArgumentValue::Array(a) => a.iter().map(|x| x.as_str()).collect(),
        }
    }
}

impl Rule {
    pub fn is_allowed(&self) -> Option<bool> {
        let is_allowed = self.action == "allow";
        let matching_features = vec!["has_custom_resolution"];

        let mut matches = true;

        if let Some(os) = &self.os {
            if !os.matches() {
                matches = false;
            }
        }

        if let Some(features) = &self.features {
            for (feature, value) in features {
                let contains = matching_features.contains(&feature.as_str());
                if contains != *value {
                    matches = false;
                    break;
                }
            }
        }

        if matches {
            Some(is_allowed)
        } else {
            None
        }
    }
}

fn rules_apply(rules: &Vec<Rule>) -> bool {
    let mut some_allowed = false;
    for rule in rules {
        if let Some(is_allowed) = rule.is_allowed() {
            if !is_allowed {
                return false;
            }
            some_allowed = true;
        }
    }
    some_allowed
}

#[derive(Deserialize, Clone)]
pub struct ComplexArgument {
    pub value: ArgumentValue,
    pub rules: Vec<Rule>,
}

#[derive(Deserialize, Clone)]
#[serde(untagged)]
pub enum VariableArgument {
    Simple(String),
    Complex(ComplexArgument),
}

impl VariableArgument {
    pub fn get_matching_values(&self) -> Vec<&str> {
        match self {
            VariableArgument::Simple(s) => vec![s.as_str()],
            VariableArgument::Complex(complex) => {
                if rules_apply(&complex.rules) {
                    complex.value.get_values()
                } else {
                    vec![]
                }
            }
        }
    }
}

#[derive(Deserialize, Clone)]
pub struct Arguments {
    pub game: Vec<VariableArgument>,
    pub jvm: Vec<VariableArgument>,
}

#[derive(Deserialize)]
pub struct AssetIndex {
    pub id: String,
    pub sha1: String,
    pub url: String,
}

#[derive(Deserialize)]
pub struct JavaVersion {
    #[serde(rename = "majorVersion")]
    pub major_version: u64,
}

#[derive(Deserialize, Clone)]
pub struct Download {
    pub path: Option<String>,
    pub sha1: String,
    pub url: String,
}

#[derive(Deserialize, Clone)]
pub struct LibraryDownloads {
    pub artifact: Option<Download>,
    pub classifiers: Option<HashMap<String, Download>>,
}

#[derive(Deserialize, Clone)]
pub struct LibraryExtract {
    pub exclude: Option<Vec<String>>,
}

#[derive(Deserialize, Clone)]
pub struct Library {
    pub(in crate::modpack) name: String,
    pub(in crate::modpack) downloads: Option<LibraryDownloads>,
    pub(in crate::modpack) rules: Option<Vec<Rule>>,
    url: Option<String>,
    sha1: Option<String>,
    pub(in crate::modpack) natives: Option<HashMap<String, String>>,
    extract: Option<LibraryExtract>,
}

impl Library {
    pub(in crate::modpack) fn rules_match(&self) -> bool {
        if let Some(rules) = &self.rules {
            rules_apply(rules)
        } else {
            true
        }
    }

    pub fn get_path_from_name(&self) -> String {
        let full_name = self.name.clone();
        let mut parts: Vec<&str> = full_name.split(':').collect();
        if parts.len() != 4 {
            parts.push("");
        }
        let (pkg, name, version, suffix) = (parts[0], parts[1], parts[2], parts[3]);
        let pkg_path = pkg.replace('.', "/");
        let suffix = if suffix.is_empty() {
            "".to_string()
        } else {
            format!("-{}", suffix)
        };
        format!(
            "{}/{}/{}/{}-{}{}.jar",
            pkg_path, name, version, name, version, suffix
        )
    }

    fn get_natives_path(&self, libraries_dir: &Path, download: &Download) -> PathBuf {
        let filename = download.url.split('/').last().unwrap_or(&download.url);
        libraries_dir.join(filename)
    }

    pub fn get_check_download_enties(&self, libraries_dir: &Path) -> Vec<CheckDownloadEntry> {
        let mut entries = vec![];

        if let Some(downloads) = &self.downloads {
            if let Some(artifact) = &downloads.artifact {
                if let Some(path) = self.get_path(libraries_dir) {
                    entries.push(CheckDownloadEntry {
                        url: artifact.url.clone(),
                        remote_sha1: Some(artifact.sha1.clone()),
                        path,
                    });
                }
            }
        }
        if let Some(url) = &self.url {
            entries.push(CheckDownloadEntry {
                url: url.clone(),
                remote_sha1: self.sha1.clone(),
                path: libraries_dir.join(&self.get_path_from_name()),
            });
        }

        if let Some(natives_name) = self.get_natives_name() {
            if let Some(downloads) = &self.downloads {
                if let Some(classifiers) = &downloads.classifiers {
                    if let Some(download) = classifiers.get(&natives_name) {
                        entries.push(CheckDownloadEntry {
                            url: download.url.clone(),
                            remote_sha1: Some(download.sha1.clone()),
                            path: libraries_dir
                                .join(self.get_natives_path(libraries_dir, download)),
                        });
                    }
                }
            }
        }

        entries
    }

    pub fn get_path(&self, libraries_dir: &Path) -> Option<PathBuf> {
        if let Some(downloads) = &self.downloads {
            if let Some(artifact) = &downloads.artifact {
                if let Some(path) = &artifact.path {
                    return Some(libraries_dir.join(path));
                } else {
                    return Some(libraries_dir.join(&self.get_path_from_name()));
                }
            }
        }
        if self.url.is_some() {
            return Some(libraries_dir.join(&self.get_path_from_name()));
        }

        None
    }

    pub fn get_sha1_url(&self) -> Option<String> {
        Some(self.url.clone()? + &self.get_path_from_name() + ".sha1")
    }

    fn get_natives_name(&self) -> Option<String> {
        let natives = self.natives.as_ref()?;
        if let Some(natives_name) = natives.get(&get_arch_os_name()) {
            return Some(natives_name.clone());
        }

        None
    }

    pub fn get_natives_paths(&self, libraries_dir: &Path) -> Vec<PathBuf> {
        if let Some(natives_name) = self.get_natives_name() {
            let mut paths = vec![];
            if let Some(downloads) = &self.downloads {
                if let Some(classifiers) = &downloads.classifiers {
                    if let Some(download) = classifiers.get(&natives_name) {
                        paths.push(
                            libraries_dir.join(self.get_natives_path(libraries_dir, download)),
                        );
                    }
                }
            }
            paths
        } else {
            vec![]
        }
    }

    pub fn get_extract(&self) -> Option<&LibraryExtract> {
        self.extract.as_ref()
    }

    pub(in crate::modpack) fn get_group_id(&self) -> String {
        let parts: Vec<&str> = self.name.split(':').collect();
        parts[0].to_string()
    }

    pub(in crate::modpack) fn get_full_name(&self) -> String {
        self.name.clone()
    }
}

#[derive(Deserialize)]
pub struct ClientDownload {
    pub sha1: String,
    pub url: String,
}

#[derive(Deserialize)]
pub struct Downloads {
    pub client: Option<ClientDownload>,
}

#[derive(Deserialize)]
struct VersionMetadata {
    arguments: Option<Arguments>,

    #[serde(rename = "assetIndex")]
    asset_index: Option<AssetIndex>,

    downloads: Option<Downloads>,
    id: String,

    #[serde(rename = "javaVersion")]
    java_version: Option<JavaVersion>,
    libraries: Vec<Library>,

    #[serde(rename = "mainClass")]
    main_class: String,

    #[serde(rename = "inheritsFrom")]
    inherits_from: Option<String>,

    #[serde(rename = "minecraftArguments")]
    minecraft_arguments: Option<String>,
}

lazy_static::lazy_static! {
    static ref LEGACY_JVM_ARGS: Vec<VariableArgument> = vec![
        VariableArgument::Complex(ComplexArgument {
            value: ArgumentValue::String("-XX:HeapDumpPath=MojangTricksIntelDriversForPerformance_javaw.exe_minecraft.exe.heapdump".to_string()),
            rules: vec![Rule{
                action: "allow".to_string(),
                os: Some(Os {
                    name: Some("windows".to_string()),
                    arch: None,
                }),
                features: None,
            }],
        }),
        VariableArgument::Complex(ComplexArgument {
            value: ArgumentValue::Array(vec!["-Dos.name=Windows 10".to_string(), "-Dos.version=10.0".to_string()]),
            rules: vec![Rule{
                action: "allow".to_string(),
                os: Some(Os {
                    name: Some("windows".to_string()),
                    arch: None,
                }),
                features: None,
            }],
        }),
        VariableArgument::Simple("-Djava.library.path=${natives_directory}".to_string()),
        VariableArgument::Simple("-Dminecraft.launcher.brand=${launcher_name}".to_string()),
        VariableArgument::Simple("-Dminecraft.launcher.version=${launcher_version}".to_string()),
        VariableArgument::Simple("-cp".to_string()),
        VariableArgument::Simple("${classpath}".to_string()),
    ];
}

impl VersionMetadata {
    pub fn get_arguments(&self) -> Result<Arguments, Box<dyn Error + Send + Sync>> {
        match &self.arguments {
            Some(arguments) => Ok(arguments.clone()),
            None => {
                let minecraft_arguments = self.minecraft_arguments.clone().unwrap();
                Ok(Arguments {
                    game: minecraft_arguments
                        .split_whitespace()
                        .map(|x| VariableArgument::Simple(x.to_string()))
                        .collect(),
                    jvm: LEGACY_JVM_ARGS.clone(),
                })
            }
        }
    }
}

fn get_version_metadata_path(versions_dir: &Path, version_id: &str) -> PathBuf {
    versions_dir
        .join(version_id)
        .join(format!("{}.json", version_id))
}

async fn read_version_metadata(
    versions_dir: &Path,
    version_id: &str,
) -> Result<VersionMetadata, Box<dyn Error + Send + Sync>> {
    let version_path = get_version_metadata_path(versions_dir, version_id);
    let mut file = fs::File::open(version_path).await?;
    let mut content = String::new();
    file.read_to_string(&mut content).await?;
    let metadata = serde_json::from_str(&content)?;
    Ok(metadata)
}

#[derive(thiserror::Error, Debug)]
pub enum VersionMetadataError {
    #[error("Bad arguments")]
    BadArgumentsError,
}

pub struct MergedVersionMetadata {
    pub arguments: Arguments,
    pub asset_index: AssetIndex,
    pub id: String,
    pub java_version: JavaVersion,
    libraries: Vec<Library>,
    pub main_class: String,
    pub downloads: Option<Downloads>,
    pub hierarchy_ids: Vec<String>,
}

impl MergedVersionMetadata {
    fn from_version_metadata(
        version_metadata: VersionMetadata,
    ) -> Result<MergedVersionMetadata, Box<dyn Error + Send + Sync>> {
        Ok(MergedVersionMetadata {
            arguments: version_metadata.get_arguments()?,
            asset_index: version_metadata
                .asset_index
                .ok_or(Box::new(VersionMetadataError::BadArgumentsError))?,
            id: version_metadata.id.clone(),
            java_version: version_metadata
                .java_version
                .ok_or(Box::new(VersionMetadataError::BadArgumentsError))?,
            libraries: version_metadata.libraries,
            main_class: version_metadata.main_class,
            downloads: version_metadata.downloads,
            hierarchy_ids: vec![version_metadata.id],
        })
    }

    pub fn get_client_jar_path(&self, versions_dir: &Path) -> PathBuf {
        versions_dir.join(&self.id).join(format!("{}.jar", self.id))
    }

    pub fn get_libraries(&self, version_ids: Vec<String>) -> Vec<Library> {
        let overridden = overrides::with_overrides(self.libraries.iter().collect(), version_ids);
        overridden
            .into_iter()
            .filter(|l| l.rules_match())
            .collect()
    }
}

fn merge_two_metadata(
    child_metadata: &mut MergedVersionMetadata,
    parent_metadata: VersionMetadata,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    if let Some(arguments) = parent_metadata.arguments {
        child_metadata.arguments.game.extend(arguments.game);
        child_metadata.arguments.jvm.extend(arguments.jvm);
    }
    child_metadata.libraries.extend(parent_metadata.libraries);

    if child_metadata.downloads.is_none() && parent_metadata.downloads.is_some() {
        child_metadata.downloads = parent_metadata.downloads;
    }

    child_metadata.hierarchy_ids.push(parent_metadata.id);

    Ok(())
}

pub async fn read_local_merged_version_metadata(
    version_id: &str,
    versions_dir: &Path,
) -> Result<MergedVersionMetadata, Box<dyn Error + Send + Sync>> {
    let mut metadata = read_version_metadata(versions_dir, version_id).await?;
    let mut inherits_from = metadata.inherits_from.clone();
    let mut merged_metadata = MergedVersionMetadata::from_version_metadata(metadata)?;
    while let Some(parent_id) = &inherits_from {
        metadata = read_version_metadata(versions_dir, parent_id).await?;
        inherits_from = metadata.inherits_from.clone();
        merge_two_metadata(&mut merged_metadata, metadata)?;
    }

    Ok(merged_metadata)
}

pub async fn get_merged_version_metadata(
    version_info: &VersionInfo,
    versions_dir: &Path,
) -> Result<MergedVersionMetadata, Box<dyn Error + Send + Sync>> {
    let metadata_info = version_info.get_metadata_info();

    let check_entries: Vec<CheckDownloadEntry> = metadata_info
        .iter()
        .map(|metadata| CheckDownloadEntry {
            url: metadata.url.clone(),
            remote_sha1: Some(metadata.sha1.clone()),
            path: get_version_metadata_path(versions_dir, &metadata.id),
        })
        .collect();

    let download_entries =
        files::get_download_entries(check_entries, progress::no_progress_bar()).await?;
    files::download_files(download_entries, progress::no_progress_bar()).await?;

    read_local_merged_version_metadata(&version_info.id, versions_dir).await
}
