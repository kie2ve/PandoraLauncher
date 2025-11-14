use std::{path::PathBuf, sync::Arc};

use schema::content::ContentSource;

use crate::instance::InstanceID;

#[derive(Debug, Clone, Copy)]
pub enum InstallTarget {
    Instance(InstanceID),
    Library,
    NewInstance,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContentType {
    Mod,
    Modpack,
    Resourcepack,
    Shader,
}

#[derive(Debug, Clone)]
pub struct ContentInstall {
    pub target: InstallTarget,
    pub files: Arc<[ContentInstallFile]>,
}

#[derive(Debug, Clone)]
pub struct ContentInstallFile {
    pub download: ContentDownload,
    pub content_type: ContentType,
    pub content_source: ContentSource,
}

#[derive(Debug, Clone)]
pub enum ContentDownload {
    Url {
        url: Arc<str>,
        filename: Arc<str>,
        sha1: Arc<str>,
        size: usize,
    },
    File {
        path: PathBuf,
    }
}
