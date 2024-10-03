use log::debug;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};

use crate::config::build_config;

use super::version_metadata::{Library, LibraryDownloads, Rule};

#[derive(Deserialize)]
pub struct Replacement {
    pub libraries: Vec<Library>,
    pub version: String,
}

#[derive(Deserialize)]
pub struct LibraryPatches {
    groups_to_remove: HashSet<String>,
    artifact_id_to_match: String,
    match_ignore_os: Vec<String>,
    overrides: Vec<Replacement>,
}

lazy_static::lazy_static! {
    static ref LIBRARY_PATCHES: LibraryPatches = {
        let overrides = build_config::LIBRARY_PATCHES;
        serde_json::from_str(overrides).expect("Failed to parse library patches")
    };
}

#[derive(Deserialize)]
pub struct LibraryOverride {
    downloads: Option<LibraryDownloads>,
    natives: Option<HashMap<String, String>>,
    rules: Option<Vec<Rule>>,
}

#[derive(Deserialize)]
pub struct LibraryOverrides {
    #[serde(rename = "match")]
    match_: HashSet<String>,

    #[serde(rename = "override")]
    override_: Option<LibraryOverride>,

    #[serde(rename = "additionalLibraries")]
    additional_libraries: Option<Vec<Library>>,
}

lazy_static::lazy_static! {
    static ref LIBRARY_OVERRIDES: Vec<LibraryOverrides> = {
        let overrides = build_config::MOJANG_LIBRARY_PATCHES;
        serde_json::from_str(overrides).expect("Failed to parse library overrides")
    };
}

fn with_mojang_overrides(libraries: Vec<&Library>) -> Vec<Library> {
    let mut result = vec![];
    for library in libraries {
        if LIBRARY_PATCHES
            .groups_to_remove
            .contains(&library.get_group_id())
        {
            debug!(
                "Not modifying library {} because it is in the groups to remove",
                library.get_full_name()
            );
            result.push(library.clone());
            continue;
        }

        let mut library = library.clone();
        for override_ in &*LIBRARY_OVERRIDES {
            if override_.match_.contains(&library.get_full_name()) {
                if let Some(override_) = &override_.override_ {
                    debug!("Modifying library: {}", library.get_full_name());
                    if let Some(downloads) = &override_.downloads {
                        library.downloads = Some(downloads.clone());
                    }
                    if let Some(natives) = &override_.natives {
                        library.natives = Some(natives.clone());
                    }
                    if let Some(rules) = &override_.rules {
                        library.rules = Some(rules.clone());
                    }
                }
                if let Some(additional_libraries) = &override_.additional_libraries {
                    debug!(
                        "Adding additional libraries for {}",
                        library.get_full_name()
                    );
                    result.extend(additional_libraries.clone());
                }
            }
        }
        result.push(library.clone());
    }

    debug!("Processed {} libraries with mojang overrides", result.len());

    result
}

pub fn with_overrides(libraries: Vec<&Library>) -> Vec<Library> {
    let libraries = with_mojang_overrides(libraries);

    let mut result = vec![];
    let mut override_version = None;
    for library in libraries {
        if LIBRARY_PATCHES
            .groups_to_remove
            .contains(&library.get_group_id())
        {
            debug!(
                "Skipping library {} because it is in the groups to remove",
                library.get_full_name()
            );
            if library.get_artifact_id() == LIBRARY_PATCHES.artifact_id_to_match {
                let mut is_ignored = false;
                for ignored_os in &LIBRARY_PATCHES.match_ignore_os {
                    if library.rules_match(&ignored_os) {
                        debug!("Not selecting library {} to get the version because it matches an ignored OS", library.get_full_name());
                        is_ignored = true;
                        continue;
                    }
                }

                if is_ignored {
                    continue;
                }

                debug!(
                    "Selecting library {} to get the version",
                    library.get_full_name()
                );
                override_version = Some(library.get_version());
            }
            continue;
        }
        result.push(library.clone());
    }

    if let Some(override_version) = override_version {
        for override_ in &LIBRARY_PATCHES.overrides {
            if override_.version == override_version {
                debug!("Adding override libraries for version {}", override_version);
                result.extend(override_.libraries.clone());
            }
        }
    }

    debug!("Processed {} libraries with overrides", result.len());

    result
}
