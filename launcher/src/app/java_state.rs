use shared::paths::get_java_dir;
use std::path::Path;
use std::sync::Arc;
use tokio::runtime::Runtime;

use crate::config::runtime_config;
use crate::lang::{Lang, LangMessage};
use crate::utils;
use crate::version::complete_version_metadata::CompleteVersionMetadata;

use shared::java;
use shared::progress::{ProgressBar, Unit};

use super::background_task::{BackgroundTask, BackgroundTaskResult};
use super::progress_bar::GuiProgressBar;

#[derive(Clone, PartialEq)]
pub enum JavaDownloadStatus {
    NotDownloaded,
    Downloading,
    Downloaded,
    DownloadError(String),
    DownloadErrorOffline,
}

pub struct JavaDownloadResult {
    pub status: JavaDownloadStatus,
    pub java_installation: Option<java::JavaInstallation>,
}

pub fn download_java(
    runtime: &Runtime,
    required_version: &str,
    java_dir: &Path,
    progress_bar: Arc<dyn ProgressBar<LangMessage>>,
) -> BackgroundTask<JavaDownloadResult> {
    let progress_bar_clone = progress_bar.clone();
    let required_version = required_version.to_string();
    let java_dir = java_dir.to_path_buf();
    let fut = async move {
        progress_bar_clone.set_message(LangMessage::DownloadingJava);
        let result = java::download_java(&required_version, &java_dir, progress_bar_clone).await;
        match result {
            Ok(java_installation) => JavaDownloadResult {
                status: JavaDownloadStatus::Downloaded,
                java_installation: Some(java_installation),
            },
            Err(e) => JavaDownloadResult {
                status: if utils::is_connect_error(&e) {
                    JavaDownloadStatus::DownloadErrorOffline
                } else {
                    JavaDownloadStatus::DownloadError(e.to_string())
                },
                java_installation: None,
            },
        }
    };

    BackgroundTask::with_callback(
        fut,
        runtime,
        Box::new(move || {
            progress_bar.finish();
        }),
    )
}

pub struct JavaState {
    status: JavaDownloadStatus,
    java_download_task: Option<BackgroundTask<JavaDownloadResult>>,
    java_download_progress_bar: Arc<GuiProgressBar>,
    settings_opened: bool,
}

impl JavaState {
    pub fn new(ctx: &egui::Context) -> Self {
        let java_download_progress_bar = Arc::new(GuiProgressBar::new(ctx));
        java_download_progress_bar.set_unit(Unit {
            name: "MB".to_string(),
            size: 1024 * 1024,
        });
        Self {
            status: JavaDownloadStatus::NotDownloaded,
            java_download_task: None,
            java_download_progress_bar,
            settings_opened: false,
        }
    }

    fn check_java(
        &mut self,
        metadata: &CompleteVersionMetadata,
        config: &mut runtime_config::Config,
    ) {
        if let Some(java_path) = config.java_paths.get(metadata.get_name()) {
            if !java::check_java(&metadata.get_java_version(), java_path.as_ref()) {
                config.java_paths.remove(metadata.get_name());
                config.save();
            }
        }

        if config.java_paths.get(metadata.get_name()).is_none() {
            let launcher_dir = config.get_launcher_dir();

            if let Some(java_installation) =
                java::get_java(&metadata.get_java_version(), &get_java_dir(&launcher_dir))
            {
                config.java_paths.insert(
                    metadata.get_name().to_string(),
                    java_installation.path.to_str().unwrap().to_string(),
                );
                config.save();
            }
        }
    }

    pub fn update(
        &mut self,
        runtime: &Runtime,
        metadata: &CompleteVersionMetadata,
        config: &mut runtime_config::Config,
        need_java_check: bool,
    ) {
        if need_java_check {
            self.check_java(metadata, config);
            if config.java_paths.get(metadata.get_name()).is_some() {
                self.status = JavaDownloadStatus::Downloaded;
            }
            if config.java_paths.get(metadata.get_name()).is_none()
                && self.status == JavaDownloadStatus::Downloaded
            {
                self.status = JavaDownloadStatus::NotDownloaded;
            }

            self.settings_opened = false;
        }

        if self.status == JavaDownloadStatus::Downloading && self.java_download_task.is_none() {
            let launcher_dir = config.get_launcher_dir();
            let java_dir = get_java_dir(&launcher_dir);

            self.java_download_progress_bar.reset();

            let java_download_task = download_java(
                runtime,
                &metadata.get_java_version(),
                &java_dir,
                self.java_download_progress_bar.clone(),
            );
            self.java_download_task = Some(java_download_task);
        }

        if let Some(task) = self.java_download_task.as_ref() {
            if task.has_result() {
                let task = self.java_download_task.take().unwrap();
                let result = task.take_result();
                match result {
                    BackgroundTaskResult::Finished(result) => {
                        self.status = result.status;
                        self.java_download_task = None;
                        if self.status == JavaDownloadStatus::Downloaded {
                            let path = result.java_installation.as_ref().unwrap().path.clone();
                            if java::check_java(&metadata.get_java_version(), &path) {
                                config.java_paths.insert(
                                    metadata.get_name().to_string(),
                                    path.to_str().unwrap().to_string(),
                                );
                                config.save();
                            } else {
                                self.status = JavaDownloadStatus::DownloadError(
                                    "Downloaded Java is not valid".to_string(),
                                );
                            }
                        }
                    }
                    BackgroundTaskResult::Cancelled => {
                        self.status = JavaDownloadStatus::NotDownloaded;
                        self.java_download_task = None;
                    }
                }
            }
        }
    }

    fn is_download_needed(&self) -> bool {
        match self.status {
            JavaDownloadStatus::NotDownloaded => true,
            JavaDownloadStatus::DownloadError(_) => true,
            JavaDownloadStatus::DownloadErrorOffline => true,
            JavaDownloadStatus::Downloading => false,
            JavaDownloadStatus::Downloaded => false,
        }
    }

    pub fn schedule_download_if_needed(&mut self) {
        if self.is_download_needed() {
            self.status = JavaDownloadStatus::Downloading;
        }
    }

    pub fn render_ui(
        &mut self,
        ui: &mut egui::Ui,
        config: &mut runtime_config::Config,
        selected_metadata: &CompleteVersionMetadata,
    ) {
        match self.status {
            JavaDownloadStatus::NotDownloaded => {
                ui.label(
                    LangMessage::NeedJava {
                        version: selected_metadata.get_java_version().clone(),
                    }
                    .to_string(&config.lang),
                );
            }
            JavaDownloadStatus::Downloading => {} // message is shown in progress bar
            JavaDownloadStatus::DownloadError(ref e) => {
                ui.label(LangMessage::ErrorDownloadingJava(e.clone()).to_string(&config.lang));
            }
            JavaDownloadStatus::DownloadErrorOffline => {
                ui.label(LangMessage::NoConnectionToJavaServer.to_string(&config.lang));
            }
            JavaDownloadStatus::Downloaded => {
                ui.label(
                    LangMessage::JavaInstalled {
                        version: selected_metadata.get_java_version().clone(),
                    }
                    .to_string(&config.lang),
                );
            }
        }

        if self.status == JavaDownloadStatus::Downloading {
            self.java_download_progress_bar.render(ui, &config.lang);
            self.render_cancel_button(ui, &config.lang);
        }
    }

    pub fn ready_for_launch(&self) -> bool {
        self.status == JavaDownloadStatus::Downloaded
    }

    fn render_cancel_button(&mut self, ui: &mut egui::Ui, lang: &Lang) {
        if ui
            .button(LangMessage::CancelDownload.to_string(lang))
            .clicked()
        {
            self.cancel_download();
        }
    }

    pub fn cancel_download(&mut self) {
        if let Some(task) = self.java_download_task.as_ref() {
            task.cancel();
        }
    }
}
