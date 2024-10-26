use std::path::Path;

use crate::{
    config::{build_config, runtime_config},
    lang::LangMessage,
};

use shared::{
    paths::get_manifest_path,
    version::version_manifest::{VersionInfo, VersionManifest},
};

use super::background_task::{BackgroundTask, BackgroundTaskResult};

#[derive(Clone, PartialEq)]
enum FetchStatus {
    Fetching,
    FetchedRemote,
    FetchedLocalRemoteError(String),
    FetchedLocalOffline,
}

struct ManifestFetchResult {
    status: FetchStatus,
    manifest: VersionManifest,
}

fn fetch_manifest<Callback>(
    runtime: &tokio::runtime::Runtime,
    manifest_path: &Path,
    callback: Callback,
) -> BackgroundTask<ManifestFetchResult>
where
    Callback: FnOnce() + Send + 'static,
{
    let manifest_path = manifest_path.to_path_buf();

    let fut = async move {
        let result = VersionManifest::fetch(&build_config::get_version_manifest_url()).await;
        match result {
            Ok(manifest) => ManifestFetchResult {
                status: FetchStatus::FetchedRemote,
                manifest: manifest,
            },
            Err(e) => {
                let mut connect_error = false;
                if let Some(re) = e.downcast_ref::<reqwest::Error>() {
                    if re.is_connect() {
                        connect_error = true;
                    }
                }

                ManifestFetchResult {
                    status: if connect_error {
                        FetchStatus::FetchedLocalOffline
                    } else {
                        FetchStatus::FetchedLocalRemoteError(e.to_string())
                    },
                    manifest: VersionManifest::read_local_safe(&manifest_path).await,
                }
            }
        }
    };

    BackgroundTask::with_callback(fut, runtime, Box::new(callback))
}

pub struct ManifestState {
    status: FetchStatus,
    fetch_task: Option<BackgroundTask<ManifestFetchResult>>,
    manifest: Option<VersionManifest>,
}

impl ManifestState {
    pub fn new() -> Self {
        return ManifestState {
            status: FetchStatus::Fetching,
            fetch_task: None,
            manifest: None,
        };
    }

    pub fn update(
        &mut self,
        runtime: &tokio::runtime::Runtime,
        config: &mut runtime_config::Config,
        ctx: &egui::Context,
    ) -> bool {
        if self.status == FetchStatus::Fetching && self.fetch_task.is_none() {
            let launcher_dir = config.get_launcher_dir();
            let manifest_path = get_manifest_path(&launcher_dir);

            let ctx = ctx.clone();
            self.fetch_task = Some(fetch_manifest(runtime, &manifest_path, move || {
                ctx.request_repaint();
            }));
        }

        if let Some(task) = self.fetch_task.as_ref() {
            if task.has_result() {
                let task = self.fetch_task.take().unwrap();
                let result = task.take_result();
                match result {
                    BackgroundTaskResult::Finished(result) => {
                        self.status = result.status.clone();
                        if config.selected_modpack_name.is_none()
                            && result.manifest.versions.len() == 1
                        {
                            config.selected_modpack_name =
                                result.manifest.versions.first().map(|x| x.get_name());
                            config.save();
                        }
                        self.manifest = Some(result.manifest);
                    }
                    BackgroundTaskResult::Cancelled => {
                        self.status = FetchStatus::Fetching;
                    }
                }

                return true;
            }
        }

        false
    }

    pub fn render_ui(&mut self, ui: &mut egui::Ui, config: &mut runtime_config::Config) -> bool {
        let mut selected_modpack_name = config.selected_modpack_name.clone();

        ui.horizontal(|ui| {
            ui.label(LangMessage::SelectModpack.to_string(&config.lang));
            egui::ComboBox::from_id_source("modpacks")
                .selected_text(
                    selected_modpack_name
                        .clone()
                        .unwrap_or_else(|| LangMessage::NotSelected.to_string(&config.lang)),
                )
                .show_ui(ui, |ui| match self.manifest.as_ref() {
                    Some(r) => {
                        let modpack_names: Vec<String> =
                            r.versions.iter().map(|x| x.get_name()).collect();
                        for modpack_name in modpack_names {
                            ui.selectable_value(
                                &mut selected_modpack_name,
                                Some(modpack_name.clone()),
                                modpack_name,
                            );
                        }
                    }
                    None => {
                        ui.label(LangMessage::NoModpacks.to_string(&config.lang));
                    }
                });
        });

        match self.status {
            FetchStatus::Fetching => {
                ui.label(LangMessage::FetchingVersionManifest.to_string(&config.lang));
            }
            FetchStatus::FetchedRemote => {}
            FetchStatus::FetchedLocalOffline => {
                ui.label(LangMessage::NoConnectionToManifestServer.to_string(&config.lang));
            }
            FetchStatus::FetchedLocalRemoteError(ref s) => {
                ui.label(
                    LangMessage::ErrorFetchingRemoteManifest(s.clone()).to_string(&config.lang),
                );
            }
        }

        if self.status != FetchStatus::FetchedRemote && self.status != FetchStatus::Fetching {
            if ui
                .button(LangMessage::FetchManifest.to_string(&config.lang))
                .clicked()
            {
                self.status = FetchStatus::Fetching;
            }
        }

        if config.selected_modpack_name != selected_modpack_name {
            config.selected_modpack_name = selected_modpack_name;
            config.save();
            true
        } else {
            false
        }
    }

    pub fn get_selected_modpack(&self, config: &runtime_config::Config) -> Option<&VersionInfo> {
        return self.manifest.as_ref().and_then(|manifest| {
            manifest
                .versions
                .iter()
                .find(|x| Some(&x.get_name()) == config.selected_modpack_name.as_ref())
        });
    }

    pub fn online(&self) -> bool {
        self.status == FetchStatus::FetchedRemote
    }
}
