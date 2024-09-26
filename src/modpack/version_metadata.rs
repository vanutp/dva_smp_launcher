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

use super::version_manifest::VersionInfo;

#[derive(Deserialize, Clone)]
pub struct GameRule {
    pub action: String,
    pub features: HashMap<String, bool>,
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

fn get_metadata_os_name() -> String {
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

#[derive(Deserialize, Clone)]
struct Os {
    name: Option<String>,
    arch: Option<String>,
}

#[derive(Deserialize, Clone)]
pub struct Rule {
    action: String,
    os: Option<Os>,
}

impl Rule {
    pub fn is_allowed(&self) -> bool {
        let allow = self.action == "allow";
        if let Some(os) = &self.os {
            if let Some(name) = &os.name {
                let os_matches = name == &get_metadata_os_name();
                return os_matches == allow;
            }
            if let Some(arch) = &os.arch {
                let arch_matches = arch == std::env::consts::ARCH;
                return arch_matches == allow;
            }
            true
        } else {
            allow
        }
    }
}

#[derive(Deserialize, Clone)]
pub struct ComplexArgument<R> {
    pub value: ArgumentValue,
    pub rules: Vec<R>,
}

pub trait Argument {
    fn apply(&self) -> bool;
    fn get_values(&self) -> Vec<&str>;
}

#[derive(Deserialize, Clone)]
#[serde(untagged)]
pub enum VariableArgument<R> {
    Simple(String),
    Complex(ComplexArgument<R>),
}

impl<T> VariableArgument<T> {
    pub fn get_values(&self) -> Vec<&str> {
        match self {
            VariableArgument::Simple(s) => vec![s.as_str()],
            VariableArgument::Complex(complex) => complex.value.get_values(),
        }
    }
}

impl Argument for VariableArgument<GameRule> {
    fn apply(&self) -> bool {
        match self {
            VariableArgument::Simple(_) => true,
            VariableArgument::Complex(complex) => {
                for rule in &complex.rules {
                    for (key, value) in &rule.features {
                        let custom_resolution = key == "has_custom_resolution" && *value;
                        let allow = rule.action == "allow";
                        if custom_resolution == allow {
                            return true;
                        }
                    }
                }
                false
            }
        }
    }

    fn get_values(&self) -> Vec<&str> {
        self.get_values()
    }
}

impl Argument for VariableArgument<Rule> {
    fn apply(&self) -> bool {
        match self {
            VariableArgument::Simple(_) => true,
            VariableArgument::Complex(complex) => {
                for rule in &complex.rules {
                    if !rule.is_allowed() {
                        return false;
                    }
                }
                true
            }
        }
    }

    fn get_values(&self) -> Vec<&str> {
        self.get_values()
    }
}

#[derive(Deserialize, Clone)]
pub struct Arguments {
    pub game: Vec<VariableArgument<GameRule>>,
    pub jvm: Vec<VariableArgument<Rule>>,
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

#[derive(Deserialize)]
pub struct Download {
    pub path: String,
    pub sha1: String,
    pub url: String,
}

#[derive(Deserialize)]
pub struct LibraryDownloads {
    pub artifact: Option<Download>,
    pub classifiers: Option<HashMap<String, Download>>,
}

#[derive(Deserialize)]
pub struct Library {
    name: String,
    downloads: Option<LibraryDownloads>,
    rules: Option<Vec<Rule>>,
    url: Option<String>,
    sha1: Option<String>,
    natives: Option<HashMap<String, String>>,
}

impl Library {
    fn apply_rules(&self) -> bool {
        if let Some(rules) = &self.rules {
            for rule in rules {
                if !rule.is_allowed() {
                    return false;
                }
            }
        }
        true
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

    fn get_natives_name(&self) -> Option<String> {
        let os_name = get_metadata_os_name();
        self.natives.as_ref()?.get(&os_name).cloned()
    }

    pub fn get_check_download_entries(&self, libraries_dir: &Path) -> Vec<CheckDownloadEntry> {
        let mut entries = vec![];

        if self.apply_rules() {
            if let Some(downloads) = &self.downloads {
                if let Some(artifact) = &downloads.artifact {
                    entries.push(CheckDownloadEntry {
                        url: artifact.url.clone(),
                        remote_sha1: Some(artifact.sha1.clone()),
                        path: libraries_dir.join(&artifact.path),
                    });
                }
            } else if let Some(url) = &self.url {
                entries.push(CheckDownloadEntry {
                    url: url.clone(),
                    remote_sha1: self.sha1.clone(),
                    path: libraries_dir.join(&self.get_path_from_name()),
                });
            }
        }

        if let Some(classifiers) = self.downloads.as_ref().and_then(|d| d.classifiers.as_ref()) {
            if let Some(natives_name) = self.get_natives_name() {
                if let Some(natives_download) = classifiers.get(&natives_name) {
                    entries.push(CheckDownloadEntry {
                        url: natives_download.url.clone(),
                        remote_sha1: Some(natives_download.sha1.clone()),
                        path: libraries_dir.join(&natives_download.path),
                    });
                }
            }
        }

        entries
    }

    pub fn get_paths(&self, libraries_dir: &Path) -> Vec<PathBuf> {
        let mut paths = vec![];

        if self.apply_rules() {
            if let Some(downloads) = &self.downloads {
                if let Some(artifact) = &downloads.artifact {
                    paths.push(libraries_dir.join(&artifact.path));
                }
            } else {
                paths.push(libraries_dir.join(&self.get_path_from_name()));
            }
        }

        if let Some(classifiers) = self.downloads.as_ref().and_then(|d| d.classifiers.as_ref()) {
            if let Some(natives_name) = self.get_natives_name() {
                if let Some(natives_download) = classifiers.get(&natives_name) {
                    paths.push(libraries_dir.join(&natives_download.path));
                }
            }
        }

        paths
    }

    pub fn get_sha1_url(&self) -> Option<String> {
        Some(self.url.clone()? + &self.get_path_from_name() + ".sha1")
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
    pub asset_index: Option<AssetIndex>,

    pub downloads: Option<Downloads>,
    pub id: String,

    #[serde(rename = "javaVersion")]
    pub java_version: Option<JavaVersion>,
    pub libraries: Vec<Library>,

    #[serde(rename = "mainClass")]
    pub main_class: String,

    #[serde(rename = "inheritsFrom")]
    pub inherits_from: Option<String>,

    #[serde(rename = "minecraftArguments")]
    minecraft_arguments: Option<String>,
}
lazy_static::lazy_static! {
    static ref LEGACY_JVM_ARGS: Vec<VariableArgument<Rule>> = vec![
        VariableArgument::Complex(ComplexArgument {
            value: ArgumentValue::String("-XstartOnFirstThread".to_string()),
            rules: vec![Rule{
                action: "allow".to_string(),
                os: Some(Os {
                    name: Some("osx".to_string()),
                    arch: None,
                }),
            }],
        }),
        VariableArgument::Complex(ComplexArgument {
            value: ArgumentValue::String("-XX:HeapDumpPath=MojangTricksIntelDriversForPerformance_javaw.exe_minecraft.exe.heapdump".to_string()),
            rules: vec![Rule{
                action: "allow".to_string(),
                os: Some(Os {
                    name: Some("windows".to_string()),
                    arch: None,
                }),
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
    pub libraries: Vec<Library>,
    pub main_class: String,
    pub downloads: Option<Downloads>,
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
            id: version_metadata.id,
            java_version: version_metadata
                .java_version
                .ok_or(Box::new(VersionMetadataError::BadArgumentsError))?,
            libraries: version_metadata.libraries,
            main_class: version_metadata.main_class,
            downloads: version_metadata.downloads,
        })
    }

    pub fn get_client_jar_path(&self, versions_dir: &Path) -> PathBuf {
        versions_dir.join(&self.id).join(format!("{}.jar", self.id))
    }
}

fn merge_two_metadata(
    parent_metadata: &mut MergedVersionMetadata,
    child_metadata: VersionMetadata,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    if let Some(arguments) = child_metadata.arguments {
        parent_metadata.arguments.game.extend(arguments.game);
        parent_metadata.arguments.jvm.extend(arguments.jvm);
    }
    parent_metadata.libraries.extend(child_metadata.libraries);

    parent_metadata.id = child_metadata.id;
    parent_metadata.main_class = child_metadata.main_class;

    if parent_metadata.downloads.is_none() && child_metadata.downloads.is_some() {
        parent_metadata.downloads = child_metadata.downloads;
    }

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
