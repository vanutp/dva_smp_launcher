use std::collections::{HashMap, HashSet};
use std::fs::{create_dir_all, File, remove_file};
use std::io;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use clone_macro::clone;
use serde::Deserialize;
use tokio::sync::Mutex;
use walkdir::WalkDir;

use crate::config;
use crate::utils::consts::{ModpackInfo, SERVER_BASE};
use crate::utils::hash_file::hash_file;

#[derive(Deserialize, Debug)]
struct ModpackIndex {
    main_class: String,
    include: Vec<String>,
    objects: HashMap<String, String>,
}

pub async fn sync_modpack<F>(progress_callback: F) -> anyhow::Result<ModpackInfo>
    where
        F: Fn(&str, f32) + Send + 'static
{
    progress_callback("Проверка файлов сборки...", 0.1);
    let index_response = reqwest::Client::new()
        .get(format!("{}index.json", SERVER_BASE))
        .send()
        .await?
        .error_for_status()?
        .json::<ModpackIndex>()
        .await?;
    let mc_dir = config::get_minecraft_dir();

    let mut existing_objects = HashMap::new();
    for rel_path in index_response.include.iter() {
        let rel_include_path = Path::new(rel_path);
        let include_path = mc_dir.join(rel_include_path);
        if include_path.is_file() {
            existing_objects.insert(
                rel_include_path.to_str().unwrap().to_string(),
                hash_file(&include_path),
            );
        } else if include_path.is_dir() {
            for object in WalkDir::new(include_path) {
                let object_path = object?.path().to_path_buf();
                let rel_object_path = object_path.strip_prefix(&mc_dir)?.to_path_buf();
                if object_path.is_file() {
                    existing_objects.insert(
                        rel_object_path.to_str().unwrap().to_string(),
                        hash_file(&object_path),
                    );
                }
            }
        }
    }

    for (object, _) in &existing_objects {
        if index_response.objects.get(object).is_none() {
            let rel_object_path = Path::new(object);
            let object_path = mc_dir.join(rel_object_path);
            remove_file(object_path)?;
        }
    }
    let mut to_download = HashSet::new();
    for (object, hash) in &index_response.objects {
        let existing_object = existing_objects.get(object);
        if existing_object.is_none() || existing_object.unwrap() != hash {
            to_download.insert(object.clone());
        }
    }

    let to_download = Arc::new(Mutex::new(to_download));
    let mut tasks = Vec::new();
    let total_files = to_download.lock().await.len() as i32;
    tasks.push(tokio::spawn(clone!([to_download], async move {
        loop {
            let downloaded_files = total_files - to_download.lock().await.len() as i32;
            progress_callback(
                format!("Загрузка файлов сборки... ({}/{})", downloaded_files, total_files).as_str(),
                downloaded_files as f32 / total_files as f32
            );
            if downloaded_files == total_files {
                break;
            }
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
    })));
    for _ in 0..16 {
        tasks.push(tokio::spawn(clone!([mc_dir, to_download], async move {
            let client = reqwest::Client::new();
            loop {
                let mut to_download = to_download.lock().await;
                if to_download.is_empty() {
                    break;
                }
                let obj = to_download.iter().next().unwrap().clone();
                to_download.remove(&obj);
                drop(to_download);
                let rel_object_path = Path::new(&obj);
                let object_path = mc_dir.join(rel_object_path);
                create_dir_all(object_path.parent().unwrap()).unwrap();
                let url = format!("{}{}", SERVER_BASE, obj);
                let resp = client.get(url)
                    .send()
                    .await
                    // TODO: нормальная обработка ошибок
                    .unwrap();
                let mut file = File::create(object_path).unwrap();
                let mut content = io::Cursor::new(resp.bytes().await.unwrap());
                io::copy(&mut content, &mut file).unwrap();
            }
        })))
    }

    futures::future::join_all(tasks).await;

    Ok(ModpackInfo {
        main_class: index_response.main_class,
    })
}
