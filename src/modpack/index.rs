use reqwest::Client;
use serde::{Deserialize, Serialize};
use tokio::sync::oneshot;
use std::collections::HashMap;
use std::fs;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};

use crate::config::build_config;
use crate::config::runtime_config;
use crate::lang::get_loc;
use crate::utils;

use super::files::get_files_in_dir;

#[derive(Serialize, Deserialize, Debug)]
struct ModpackIndex {
    modpack_name: String,
    java_version: String,
    minecraft_version: String,
    modpack_version: String,
    asset_index: String,
    main_class: String,
    libraries: Vec<serde_json::Value>,
    java_args: Vec<serde_json::Value>,
    game_args: Vec<serde_json::Value>,
    include: Vec<String>,
    include_no_overwrite: Vec<String>,
    objects: HashMap<String, String>,
    client_filename: String,
}

async fn load_remote_indexes() -> Result<Vec<ModpackIndex>, Box<dyn std::error::Error>> {
    let client = Client::new();
    let res = client
        .get(format!("{}/index.json", build_config::get_server_base()))
        .send()
        .await?
        .error_for_status()?
        .json::<Vec<ModpackIndex>>()
        .await?;
    Ok(res)
}

fn load_local_indexes(config: &runtime_config::Config) -> Vec<ModpackIndex> {
    let index_path = runtime_config::get_index_path(config);
    if !index_path.is_file() {
        return vec![];
    }
    match fs::read_to_string(index_path) {
        Ok(data) => match serde_json::from_str(&data) {
            Ok(indexes) => indexes,
            Err(_) => vec![],
        },
        Err(_) => vec![],
    }
}

fn save_local_index(config: &runtime_config::Config, index: ModpackIndex) {
    let mut indexes = load_local_indexes(config);
    indexes.retain(|x| x.modpack_name != index.modpack_name);
    indexes.push(index);
    if let Ok(data) = serde_json::to_string_pretty(&indexes) {
        let _ = fs::write(runtime_config::get_index_path(config), data);
    }
}

async fn get_modpack_index(config: &runtime_config::Config, online: bool) -> Option<ModpackIndex> {
    let indexes = if online {
        load_remote_indexes().await.unwrap_or_else(|_| vec![])
    } else {
        load_local_indexes(config)
    };
    indexes
        .into_iter()
        .find(|x| &x.modpack_name == config.modpack_name.as_ref().unwrap())
}

async fn sync_modpack(
    config: &runtime_config::Config,
    index: ModpackIndex,
    force_overwrite: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let modpack_dir = runtime_config::get_minecraft_dir(config, &index.modpack_name);
    let assets_dir = runtime_config::get_assets_dir(config);

    let get_files = |x| get_files_in_dir(&modpack_dir.join(x), &modpack_dir);
    let mut existing_files: Vec<String> = vec![];

    existing_files.extend(index.include.iter().map(get_files).flatten());
    if force_overwrite {
        existing_files.extend(index.include_no_overwrite.iter().map(get_files).flatten());
    }
    existing_files.extend(
        get_files_in_dir(&assets_dir, &assets_dir)
            .into_iter()
            .map(|x| format!("assets/{}", x)),
    );

    for file in existing_files.iter() {
        let path = modpack_dir.join(file);
        if file.starts_with("assets/") {
            continue;
        }
        if !index.objects.contains_key(file) {
            fs::remove_file(path)?;
        }
    }

    let mut remote_files: Vec<String> = vec![];

    let (hash_tx, mut hash_rx): (UnboundedSender<()>, UnboundedReceiver<()>) = unbounded_channel();
    let (result_tx, result_rx) = oneshot::channel();

    let hash_files_count = existing_files.len();
    let _ = tokio::spawn(async move {
        let result = super::files::hash_files(existing_files, &modpack_dir, hash_tx).await;
        let _ = result_tx.send(result);
    });

    let bar = utils::get_fancy_progress_bar(hash_files_count as u64, get_loc(&config.lang).checking_files);
    while let Some(_) = hash_rx.recv().await {
        bar.inc(1);
    }
    bar.finish();

    let hashes = result_rx.await?;
    let mut to_download = vec![];

    for (file, hash) in hashes {
        if let Some(remote_hash) = index.objects.get(&file) {
            if remote_hash != &hash {
                to_download.push(file);
            }
        }
    }

    for file in index.include.iter() {
        if !existing_files.contains(file) {
            to_download.push(file.clone());
        }
    }

    let mut urls: Vec<String> = vec![];
    let mut paths: Vec<String> = vec![];
    for file in to_download.into_iter() {
        if let Some(url) = index.objects.get(file) {
            urls.push(url);
            paths.push(modpack_dir.join(file));
        }
    }

    let (download_tx, mut download_rx): (UnboundedSender<()>, UnboundedReceiver<()>) = unbounded_channel();
    // let (result_tx, result_rx) = oneshot::channel();

    // let download_files_count = to_download.len();
    // let _ = tokio::spawn(async move {
    //     let result = super::files::download_files(to_download, &index, &modpack_dir, download_tx).await;
    //     let _ = result_tx.send(result);
    // });

    Ok(())
}
