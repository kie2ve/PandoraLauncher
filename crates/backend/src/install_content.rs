use std::{ffi::OsString, io::Write, path::PathBuf, sync::Arc};

use bridge::{
    install::{ContentDownload, ContentInstall, ContentInstallFile, ContentType},
    message::MessageToFrontend,
    modal_action::{ModalAction, ProgressTracker},
};
use schema::content::ContentSource;
use sha1::{Digest, Sha1};
use tokio::io::AsyncWriteExt;

use crate::BackendState;

#[derive(thiserror::Error, Debug)]
pub enum ContentInstallError {
    #[error("Failed to download remote content")]
    Reqwest(#[from] reqwest::Error),
    #[error("Downloaded file had the wrong size")]
    WrongFilesize,
    #[error("Downloaded file had the wrong hash")]
    WrongHash,
    #[error("Hash isn't a valid sha1 hash:\n{0}")]
    InvalidHash(Arc<str>),
    #[error("Failed to perform I/O operation:\n{0}")]
    IoError(#[from] std::io::Error),
    #[error("Invalid filename:\n{0}")]
    InvalidFilename(Arc<str>),
}

struct InstallFromContentLibrary {
    from: PathBuf,
    hash: [u8; 20],
    filename: OsString,
    content_file: ContentInstallFile,
}

impl BackendState {
    pub async fn install_content(&mut self, content: ContentInstall, modal_action: ModalAction) {
        // todo: check library for hash already on disk!

        for content_file in content.files.iter() {
            let ContentDownload::Url { filename, .. } = &content_file.download else {
                continue;
            };
            if !crate::is_single_component_path(&filename) {
                let error = ContentInstallError::InvalidFilename(filename.clone());
                modal_action.set_error_message(Arc::from(format!("{}", error).as_str()));
                modal_action.set_finished();
                return;
            }
        }

        let mut tasks = Vec::new();

        for content_file in content.files.iter() {
            tasks.push(async {
                match content_file.download {
                    bridge::install::ContentDownload::Url { ref url, ref filename, ref sha1, size } => {
                        let mut expected_hash = [0u8; 20];
                        let Ok(_) = hex::decode_to_slice(&**sha1, &mut expected_hash) else {
                            eprintln!("Content install has invalid sha1: {}", sha1);
                            return Err(ContentInstallError::InvalidHash(sha1.clone()));
                        };

                        // Re-encode as hex just in case the given sha1 was uppercase
                        let hash_as_str = hex::encode(expected_hash);

                        let hash_folder = self.directories.content_library_dir.join(&hash_as_str[..2]);
                        let _ = tokio::fs::create_dir_all(&hash_folder).await;
                        let path = hash_folder.join(hash_as_str);

                        let title = format!("Downloading {}", filename);
                        let tracker = ProgressTracker::new(title.into(), self.send.clone());
                        modal_action.trackers.push(tracker.clone());

                        tracker.set_total(size);
                        tracker.notify().await;

                        let valid_hash_on_disk = {
                            let path = path.clone();
                            tokio::task::spawn_blocking(move || {
                                crate::check_sha1_hash(&path, expected_hash).unwrap_or(false)
                            }).await.unwrap()
                        };

                        if valid_hash_on_disk {
                            tracker.set_count(size);
                            tracker.notify().await;
                            return Ok(InstallFromContentLibrary {
                                from: path,
                                hash: expected_hash,
                                filename: OsString::from(&**filename),
                                content_file: content_file.clone(),
                            });
                        }

                        let response = self.http_client.get(&**url).send().await?;

                        let mut file = tokio::fs::File::create(&path).await?;

                        use futures::StreamExt;
                        let mut stream = response.bytes_stream();

                        let mut total_bytes = 0;

                        let mut hasher = Sha1::new();
                        while let Some(item) = stream.next().await {
                            let item = item?;

                            total_bytes += item.len();
                            tracker.add_count(item.len());
                            tracker.notify().await;

                            hasher.write_all(&item)?;
                            file.write_all(&item).await?;
                        }

                        let actual_hash = hasher.finalize();

                        if *actual_hash != expected_hash {
                            return Err(ContentInstallError::WrongHash);
                        }

                        if total_bytes != size {
                            return Err(ContentInstallError::WrongFilesize);
                        }

                        Ok(InstallFromContentLibrary {
                            from: path,
                            hash: expected_hash,
                            filename: OsString::from(&**filename),
                            content_file: content_file.clone(),
                        })
                    },
                    bridge::install::ContentDownload::File { ref path } => {
                        let filename = path.file_name().unwrap();

                        let title = format!("Copying {}", filename.to_string_lossy());
                        let tracker = ProgressTracker::new(title.into(), self.send.clone());
                        modal_action.trackers.push(tracker.clone());

                        tracker.set_total(3);
                        tracker.notify().await;

                        let data = tokio::fs::read(path).await?;

                        tracker.set_count(1);
                        tracker.notify().await;

                        let mut hasher = Sha1::new();
                        hasher.update(&data);
                        let hash = hasher.finalize();

                        let hash_as_str = hex::encode(hash);

                        let hash_folder = self.directories.content_library_dir.join(&hash_as_str[..2]);
                        let _ = tokio::fs::create_dir_all(&hash_folder).await;
                        let path = hash_folder.join(hash_as_str);

                        let valid_hash_on_disk = {
                            let path = path.clone();
                            tokio::task::spawn_blocking(move || {
                                crate::check_sha1_hash(&path, hash.into()).unwrap_or(false)
                            }).await.unwrap()
                        };

                        tracker.set_count(2);
                        tracker.notify().await;

                        if !valid_hash_on_disk {
                            tokio::fs::write(&path, &data).await?;
                        }

                        tracker.set_count(3);
                        tracker.notify().await;
                        return Ok(InstallFromContentLibrary {
                            from: path,
                            hash: hash.into(),
                            filename: filename.into(),
                            content_file: content_file.clone(),
                        });
                    },
                }
            });
        }

        let result: Result<Vec<InstallFromContentLibrary>, ContentInstallError> = futures::future::try_join_all(tasks).await;
        match result {
            Ok(files) => {
                let mut instance_dir = None;

                match content.target {
                    bridge::install::InstallTarget::Instance(instance_id) => {
                        if let Some(instance) = self.instances.get(instance_id.index) && instance.id == instance_id {
                            instance_dir = Some(instance.dot_minecraft_path.clone());
                        }
                    },
                    bridge::install::InstallTarget::Library => {},
                    bridge::install::InstallTarget::NewInstance => todo!(),
                }

                let sources = files.iter()
                    .filter_map(|install| {
                        if install.content_file.content_source != ContentSource::Manual {
                            Some((install.hash.clone(), install.content_file.content_source))
                        } else {
                            None
                        }
                    });
                self.mod_metadata_manager.set_content_sources(sources);

                if let Some(instance_dir) = instance_dir {
                    for install in files {
                        let mut target_path = instance_dir.to_path_buf();
                        match install.content_file.content_type {
                            ContentType::Mod | ContentType::Modpack => {
                                target_path.push("mods");
                            },
                            ContentType::Resourcepack => {
                                target_path.push("resourcepacks");
                            },
                            ContentType::Shader => {
                                target_path.push("shaderpacks");
                            },
                        }
                        let _ = tokio::fs::create_dir_all(&target_path).await;
                        target_path.push(install.filename);

                        let _ = tokio::fs::hard_link(install.from, target_path).await;
                    }
                }
            },
            Err(error) => {
                modal_action.set_error_message(Arc::from(format!("{}", error).as_str()));
            },
        }

        modal_action.set_finished();
        self.send.send(MessageToFrontend::Refresh).await;
    }
}
