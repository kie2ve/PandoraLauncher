use std::{path::{Path, PathBuf}, sync::Arc};

use schema::{content::ContentSource, loader::Loader};
use ustr::Ustr;

use crate::instance::InstanceID;

#[derive(Debug, Clone)]
pub enum InstallTarget {
    Instance(InstanceID),
    Library,
    NewInstance {
        loader: Loader,
        name: Arc<str>,
        minecraft_version: Option<Arc<str>>,
    },
}

#[derive(Debug, Clone)]
pub struct ContentInstall {
    pub target: InstallTarget,
    pub files: Arc<[ContentInstallFile]>,
}

#[derive(Debug, Clone)]
pub struct ContentInstallFile {
    pub replace_old: Option<Arc<Path>>,
    pub path: Arc<Path>,
    pub download: ContentDownload,
    pub content_source: ContentSource,
}

#[derive(Debug, Clone)]
pub enum ContentDownload {
    Url {
        url: Arc<str>,
        sha1: Arc<str>,
        size: usize,
    },
    File {
        path: PathBuf,
    }
}
