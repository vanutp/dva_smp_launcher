use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::fs;
use sha1::{Sha1, Digest};
use reqwest::Client;
use tokio::fs as async_fs;
use tokio::io::AsyncWriteExt;
use tokio::sync::Semaphore;
use tokio::sync::mpsc::UnboundedSender;

use reqwest::Error as ReqwestError;
use std::io::Error as IOError;

async fn hash_file(path: &Path) -> Result<String, std::io::Error> {
    let data = async_fs::read(path).await?;
    Ok(format!("{:x}", Sha1::digest(&data)))
}

pub async fn hash_files(files: Vec<String>, base_dir: &Path, tx: UnboundedSender<()>) -> HashMap<String, String> {
    let semaphore = Arc::new(Semaphore::new(num_cpus::get()));
    let hashes = Arc::new(Mutex::new(HashMap::new()));
    let mut tasks = Vec::new();

    for file in files {
        let path = base_dir.join(&file);
        let semaphore = semaphore.clone();
        let hashes = hashes.clone();
        let thread_tx = tx.clone();

        tasks.push(tokio::spawn(async move {
            let permit = semaphore.acquire_owned().await.unwrap();
            let hash = hash_file(&path).await.unwrap();
            let mut hashes = hashes.lock().unwrap();
            hashes.insert(file, hash);
            thread_tx.send(()).unwrap();
            drop(permit);
        }));
    }

    futures::future::join_all(tasks).await;
    let hashes = Arc::try_unwrap(hashes).expect("Arc unwrap failed").into_inner();
    hashes.unwrap()
}

pub fn get_files_in_dir(path: &Path, rel_to: &Path) -> Vec<String> {
    let mut files = Vec::new();
    if path.is_file() {
        if let Ok(rel_path) = path.strip_prefix(rel_to) {
            files.push(rel_path.to_string_lossy().replace("\\", "/"));
        }
    } else if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.flatten() {
            if entry.path().is_dir() {
                continue;
            }
            if let Ok(rel_path) = entry.path().strip_prefix(rel_to) {
                files.push(rel_path.to_string_lossy().replace("\\", "/"));
            }
        }
    }
    files
}

#[derive(Debug)]
enum DownloadError {
    Reqwest(ReqwestError),
    Io(IOError),
}

impl From<ReqwestError> for DownloadError {
    fn from(err: ReqwestError) -> DownloadError {
        DownloadError::Reqwest(err)
    }
}

impl From<IOError> for DownloadError {
    fn from(err: IOError) -> DownloadError {
        DownloadError::Io(err)
    }
}

async fn download_file(client: &Client, url: &str, path: &Path) -> Result<(), DownloadError> {
    let response = client.get(url).send().await?.bytes().await?;
    let mut file = async_fs::File::create(path).await?;
    file.write_all(&response).await?;
    Ok(())
}

pub async fn download_files(urls: Vec<String>, paths: Vec<PathBuf>, tx: UnboundedSender<()>) {
    const MAX_CONCURRENT_DOWNLOADS: usize = 10;

    let client = Client::new();
    let semaphore = Arc::new(Semaphore::new(MAX_CONCURRENT_DOWNLOADS));

    let tasks: Vec<_> = urls.into_iter().zip(paths.into_iter()).map(|(url, path)| {
        let client = client.clone();
        let semaphore = semaphore.clone();
        let thread_tx = tx.clone();
        tokio::spawn(async move {
            let permit = semaphore.acquire_owned().await.unwrap();
            if let Err(e) = download_file(&client, &url, &path).await {
                panic!("Failed to download {}: {:?}", url, e);
            }
            thread_tx.send(()).unwrap();
            drop(permit);
        })
    }).collect();

    futures::future::join_all(tasks).await;
}
