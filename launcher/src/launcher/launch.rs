use log::debug;
use maplit::hashmap;
use shared::paths::{
    get_client_jar_path, get_instance_dir, get_libraries_dir, get_logs_dir, get_natives_dir,
};
use shared::utils::BoxResult;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use tokio::process::{Child, Command as TokioCommand};

use crate::auth::base::get_auth_provider;
use crate::config::runtime_config::Config;
use crate::version::complete_version_metadata::CompleteVersionMetadata;
use crate::version::rules;

use shared::version::version_metadata;

use super::compat;

const GC_OPTIONS: &[&str] = &[
    "-XX:+UnlockExperimentalVMOptions",
    "-XX:+UseG1GC",
    "-XX:G1NewSizePercent=20",
    "-XX:G1ReservePercent=20",
    "-XX:MaxGCPauseMillis=50",
    "-XX:G1HeapRegionSize=32M",
    "-XX:+DisableExplicitGC",
    "-XX:+AlwaysPreTouch",
    "-XX:+ParallelRefProcEnabled",
];

#[cfg(target_os = "windows")]
const PATHSEP: &str = ";";
#[cfg(not(target_os = "windows"))]
const PATHSEP: &str = ":";

fn replace_launch_config_variables(
    argument: String,
    variables: &HashMap<String, String>,
) -> String {
    variables.iter().fold(argument, |acc, (k, v)| {
        acc.replace(&format!("${{{}}}", k), v)
    })
}

fn process_args(
    args: &Vec<version_metadata::VariableArgument>,
    variables: &HashMap<String, String>,
) -> Vec<String> {
    let mut options = vec![];
    for arg in args {
        options.extend(
            rules::get_matching_values(arg)
                .iter()
                .map(|v| replace_launch_config_variables(v.to_string(), variables)),
        );
    }
    options
}

#[derive(thiserror::Error, Debug)]
pub enum LaunchError {
    #[error("Not authorized")]
    NotAuthorized,
    #[error("Missing authlib injector")]
    MissingAuthlibInjector,
    #[error("Missing library {0}")]
    MissingLibrary(PathBuf),
    #[error("Java path for version {0} not found")]
    JavaPathNotFound(String),
}

pub async fn launch(
    version_metadata: &CompleteVersionMetadata,
    config: &Config,
    online: bool,
) -> BoxResult<Child> {
    let auth_data = version_metadata.get_auth_data();
    let auth_provider = get_auth_provider(auth_data);

    let version_auth_data = config.get_version_auth_data(auth_data);
    if version_auth_data.is_none() {
        return Err(Box::new(LaunchError::NotAuthorized));
    }
    let version_auth_data = version_auth_data.unwrap();

    let launcher_dir = config.get_launcher_dir();
    let mut minecraft_dir = get_instance_dir(&launcher_dir, version_metadata.get_name());
    let libraries_dir = get_libraries_dir(&launcher_dir);
    let natives_dir = get_natives_dir(&launcher_dir);

    let minecraft_dir_short = minecraft_dir.clone();
    if cfg!(windows) {
        minecraft_dir = PathBuf::from(compat::win_get_long_path_name(
            &minecraft_dir_short.to_string_lossy(),
        )?);
    }

    let mut used_library_paths = HashSet::new();
    let mut classpath = vec![];
    for library in version_metadata.get_libraries_with_overrides() {
        if !rules::library_matches_os(&library) {
            continue;
        }

        let path = library.get_path(&libraries_dir);
        if let Some(path) = path {
            if !path.is_file() {
                return Err(Box::new(LaunchError::MissingLibrary(path.clone())));
            }

            let path_string = path.to_string_lossy().to_string();
            if !used_library_paths.contains(&path_string) {
                // vanilla mojang manifests have duplicates for some reason
                used_library_paths.insert(path_string.clone());
                classpath.push(path_string);
            }
        }
    }

    let client_jar_path = get_client_jar_path(&launcher_dir, version_metadata.get_id());
    if !client_jar_path.exists() {
        return Err(Box::new(LaunchError::MissingLibrary(client_jar_path)));
    }

    classpath.push(client_jar_path.to_string_lossy().to_string());

    let mut classpath_str = classpath.join(PATHSEP);
    if cfg!(windows) {
        classpath_str = classpath_str.replace("/", "\\");
    }

    let variables: HashMap<String, String> = hashmap! {
        "natives_directory".to_string() => natives_dir.to_str().unwrap().to_string(),
        "launcher_name".to_string() => "java-minecraft-launcher".to_string(),
        "launcher_version".to_string() => "1.6.84-j".to_string(),
        "classpath".to_string() => classpath_str,
        "classpath_separator".to_string() => PATHSEP.to_string(),
        "library_directory".to_string() => libraries_dir.to_str().unwrap().to_string(),
        "auth_player_name".to_string() => version_auth_data.user_info.username.clone(),
        "version_name".to_string() => version_metadata.get_id().to_string(),
        "game_directory".to_string() => minecraft_dir.to_str().unwrap().to_string(),
        "assets_root".to_string() => config.get_assets_dir().to_str().unwrap().to_string(),
        "assets_index_name".to_string() => version_metadata.get_asset_index()?.id.to_string(),
        "auth_uuid".to_string() => version_auth_data.user_info.uuid.replace("-", ""),
        "auth_access_token".to_string() => version_auth_data.token.clone(),
        "clientid".to_string() => "".to_string(),
        "auth_xuid".to_string() => "".to_string(),
        "user_type".to_string() => if online { "mojang" } else { "offline" }.to_string(),
        "version_type".to_string() => "release".to_string(),
        "resolution_width".to_string() => "925".to_string(),
        "resolution_height".to_string() => "530".to_string(),
        "user_properties".to_string() => "{}".to_string(),
    };

    let mut java_options = vec![
        GC_OPTIONS
            .iter()
            .map(|&s| s.to_string())
            .collect::<Vec<_>>(),
        vec![
            "-Xms512M".to_string(),
            format!("-Xmx{}", config.xmx),
            "-Duser.language=en".to_string(),
            "-Dfile.encoding=UTF-8".to_string(),
        ],
    ]
    .concat();

    if online {
        if let Some(auth_url) = auth_provider.get_auth_url() {
            let authlib_injector_path = launcher_dir.join(
                &version_metadata
                    .get_extra()
                    .ok_or(LaunchError::MissingAuthlibInjector)?
                    .authlib_injector
                    .as_ref()
                    .ok_or(LaunchError::MissingAuthlibInjector)?
                    .path,
            );
            if !authlib_injector_path.exists() {
                return Err(Box::new(LaunchError::MissingAuthlibInjector));
            }
            java_options.insert(
                0,
                format!(
                    "-javaagent:{}={}",
                    authlib_injector_path.to_str().unwrap(),
                    auth_url,
                ),
            );
        }
    }

    let arguments = version_metadata.get_arguments()?;

    java_options.extend(process_args(&arguments.jvm, &variables));
    let minecraft_options = process_args(&arguments.game, &variables);

    let java_path = config
        .java_paths
        .get(version_metadata.get_name())
        .ok_or_else(|| LaunchError::JavaPathNotFound(version_metadata.get_name().to_string()))?;

    debug!(
        "Launching java {} with arguments {:?}",
        java_path, java_options
    );
    debug!("Main class: {}", version_metadata.get_main_class());
    debug!("Game arguments: {:?}", minecraft_options);

    let mut cmd = TokioCommand::new(java_path);
    cmd.args(&java_options)
        .arg(&version_metadata.get_main_class())
        .args(&minecraft_options)
        .current_dir(minecraft_dir_short);

    // for some reason this is needed on macOS for minecraft process not to crash with
    // "Assertion failed: (count <= len && "snprintf() output has been truncated"), function LOAD_ERROR, file dispatch.c, line 74."
    std::env::remove_var("DYLD_FALLBACK_LIBRARY_PATH");

    let file =
        std::fs::File::create(get_logs_dir(&launcher_dir).join("latest_minecraft_launch.log"))?;
    cmd.stdout(file.try_clone()?);
    cmd.stderr(file);

    #[cfg(target_os = "windows")]
    {
        use winapi::um::winbase::CREATE_NO_WINDOW;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    Ok(cmd.spawn()?)
}
