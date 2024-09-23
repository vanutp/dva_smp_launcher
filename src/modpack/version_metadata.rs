use std::{
    collections::{HashMap, HashSet},
    error::Error,
    path::Path,
};

use serde::Deserialize;
use tokio::{fs, io::AsyncReadExt as _};

#[derive(Deserialize)]
pub struct GameRule {
    pub action: String,
    pub features: HashMap<String, bool>,
}

#[derive(Deserialize)]
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

#[derive(Deserialize)]
pub struct Os {
    pub name: Option<String>,
}

#[derive(Deserialize)]
pub struct Rule {
    pub action: String,
    pub os: Os,
}

impl Rule {
    pub fn match_os(&self) -> bool {
        if let Some(name) = &self.os.name {
            let os_matches = match name.as_str() {
                "osx" => cfg!(target_os = "macos"),
                "windows" => cfg!(windows),
                "linux" => cfg!(target_os = "linux"),
                _ => false,
            };
            let allow = self.action == "allow";
            os_matches == allow
        } else {
            true
        }
    }
}

#[derive(Deserialize)]
pub struct ComplexArgument<R> {
    pub value: ArgumentValue,
    pub rules: Vec<R>,
}

pub trait Argument {
    fn apply(&self) -> bool;
    fn get_values(&self) -> Vec<&str>;
}

#[derive(Deserialize)]
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
                    if !rule.match_os() {
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

#[derive(Deserialize)]
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

// #[derive(Deserialize)]
// pub struct JavaVersion {
//     #[serde(rename = "majorVersion")]
//     pub major_version: i64,
// }

#[derive(Deserialize)]
pub struct Download {
    pub path: String,
    pub sha1: String,
    pub url: String,
}

#[derive(Deserialize)]
pub struct LibraryDownload {
    pub artifact: Download,
}

#[derive(Deserialize)]
pub struct Library {
    pub name: String,
    pub downloads: Option<LibraryDownload>,
    pub rules: Option<Vec<Rule>>,
    pub url: Option<String>,
    pub sha1: Option<String>,
}

impl Library {
    pub fn apply_rules(&self) -> bool {
        if let Some(rules) = &self.rules {
            for rule in rules {
                if !rule.match_os() {
                    return false;
                }
            }
        }
        true
    }

    pub fn get_path(&self) -> String {
        if let Some(downloads) = &self.downloads {
            downloads.artifact.path.clone()
        } else {
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
    }

    pub fn get_url(&self) -> Option<String> {
        match &self.downloads {
            Some(downloads) => Some(downloads.artifact.url.clone()),
            None => Some(self.url.clone()? + &self.get_path()),
        }
    }

    pub fn get_sha1(&self) -> Option<String> {
        match &self.downloads {
            Some(downloads) => Some(downloads.artifact.sha1.clone()),
            None => self.sha1.clone(),
        }
    }

    pub fn get_sha1_url(&self) -> Option<String> {
        Some(self.url.clone()? + &self.get_path() + ".sha1")
    }
}

#[derive(Deserialize)]
struct VersionMetadata {
    pub arguments: Option<Arguments>,

    #[serde(rename = "assetIndex")]
    pub asset_index: Option<AssetIndex>,

    pub id: String,

    // #[serde(rename = "javaVersion")]
    // pub java_version: Option<JavaVersion>,
    pub libraries: Vec<Library>,

    #[serde(rename = "mainClass")]
    pub main_class: String,

    #[serde(rename = "inheritsFrom")]
    pub inherits_from: Option<String>,

    #[serde(rename = "minecraftArguments")]
    pub minecraft_arguments: Option<String>,
}

async fn read_versions_metadata(
    versions_dir_path: &Path,
) -> Result<Vec<VersionMetadata>, Box<dyn Error + Send + Sync>> {
    let mut versions_metadata = Vec::new();
    let mut entries = fs::read_dir(versions_dir_path).await?;

    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        if path.is_dir() {
            let version_dir = path;
            let version_name = version_dir.file_name().unwrap().to_str().unwrap();
            let json_file_path = version_dir.join(format!("{}.json", version_name));
            if json_file_path.is_file() {
                let mut file = fs::File::open(&json_file_path).await?;
                let mut contents = Vec::new();
                file.read_to_end(&mut contents).await?;
                let metadata: VersionMetadata = serde_json::from_slice(&contents)?;
                versions_metadata.push(metadata);
            }
        }
    }

    Ok(versions_metadata)
}

#[derive(thiserror::Error, Debug)]
pub enum VersionMetadataError {
    #[error("Bad arguments")]
    BadArgumentsError,
    #[error("Too many base metadata found")]
    TooManyBaseMetadataError,
    #[error("Base metadata not found")]
    BaseMetadataNotFoundError,
}

pub struct MergedVersionMetadata {
    pub arguments: Arguments,
    pub asset_index: AssetIndex,
    pub id: String,
    // pub java_version: JavaVersion,
    pub libraries: Vec<Library>,
    pub main_class: String,
}

impl MergedVersionMetadata {
    fn from_version_metadata(
        version_metadata: VersionMetadata,
    ) -> Result<MergedVersionMetadata, Box<dyn Error + Send + Sync>> {
        let version_arguments;
        if let Some(arguments) = version_metadata.arguments {
            version_arguments = arguments;
        } else {
            let minecraft_arguments = version_metadata
                .minecraft_arguments
                .ok_or(Box::new(VersionMetadataError::BadArgumentsError))?;
            version_arguments = Arguments {
                game: minecraft_arguments
                    .split_whitespace()
                    .map(|x| VariableArgument::Simple(x.to_string()))
                    .collect(),
                jvm: Vec::new(),
            };
        }

        Ok(MergedVersionMetadata {
            arguments: version_arguments,
            asset_index: version_metadata
                .asset_index
                .ok_or(Box::new(VersionMetadataError::BadArgumentsError))?,
            id: version_metadata.id,
            // java_version: version_metadata
            //     .java_version
            //     .ok_or(Box::new(VersionMetadataError::BadArgumentsError))?,
            libraries: version_metadata.libraries,
            main_class: version_metadata.main_class,
        })
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

    Ok(())
}

fn merge_recursive(
    current_metadata: &mut MergedVersionMetadata,
    metadata_children: &mut HashMap<String, Vec<VersionMetadata>>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let id = current_metadata.id.clone();

    if let Some(current_metadata_children) = metadata_children.remove(&id) {
        for child_metadata in current_metadata_children.into_iter() {
            merge_two_metadata(current_metadata, child_metadata)?;
            merge_recursive(current_metadata, metadata_children)?;
        }
    }

    Ok(())
}

fn merge_version_metadata(
    versions_metadata: Vec<VersionMetadata>,
) -> Result<MergedVersionMetadata, Box<dyn Error + Send + Sync>> {
    let mut children_ids = HashSet::new();
    for metadata in versions_metadata.iter() {
        if metadata.inherits_from.is_some() {
            children_ids.insert(metadata.id.clone());
        }
    }

    let mut base_metadata_id = None;
    for metadata in versions_metadata.iter() {
        if !children_ids.contains(&metadata.id) {
            if base_metadata_id.is_some() {
                return Err(Box::new(VersionMetadataError::TooManyBaseMetadataError));
            }
            base_metadata_id = Some(metadata.id.clone());
        }
    }
    let base_metadata_id =
        base_metadata_id.ok_or(Box::new(VersionMetadataError::BaseMetadataNotFoundError))?;

    let mut base_metadata = None;
    let mut metadata_children = HashMap::new();
    for current_metadata in versions_metadata.into_iter() {
        if let Some(parent_id) = &current_metadata.inherits_from {
            let metadata = metadata_children
                .entry(parent_id.clone())
                .or_insert_with(Vec::new);
            metadata.push(current_metadata);
        } else if current_metadata.id == base_metadata_id {
            base_metadata = Some(current_metadata);
        }
    }

    let base_metadata =
        base_metadata.ok_or(Box::new(VersionMetadataError::BaseMetadataNotFoundError))?;
    let mut merged_metadata = MergedVersionMetadata::from_version_metadata(base_metadata)?;
    merge_recursive(&mut merged_metadata, &mut metadata_children)?;

    Ok(merged_metadata)
}

pub async fn get_merged_metadata(
    versions_dir_path: &Path,
) -> Result<MergedVersionMetadata, Box<dyn Error + Send + Sync>> {
    let versions_metadata = read_versions_metadata(versions_dir_path).await?;
    let merged_metadata = merge_version_metadata(versions_metadata)?;
    Ok(merged_metadata)
}
