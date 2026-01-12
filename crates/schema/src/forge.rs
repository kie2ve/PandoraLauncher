use std::{collections::HashMap, sync::Arc};

use serde::Deserialize;
use ustr::Ustr;

use crate::version::GameLibrary;

pub const NEOFORGE_INSTALLER_MAVEN_URL: &str = "https://maven.neoforged.net/releases/net/neoforged/neoforge/maven-metadata.xml";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ForgeInstallProfile {
    pub minecraft: Arc<str>,
    pub json: Arc<str>,
    pub mirror_list: Arc<str>,
    pub data: HashMap<String, ForgeSidedData>,
    pub processors: Arc<[ForgeInstallProcessor]>,
    pub libraries: Arc<[GameLibrary]>
}

#[derive(Debug, Deserialize)]
#[cfg_attr(debug_assertions, serde(deny_unknown_fields))]
pub struct ForgeSidedData {
    pub client: Arc<str>,
    pub server: Arc<str>,
}

#[derive(Debug, Deserialize, PartialEq, Eq, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub enum ForgeSide {
    Client,
    Server,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(debug_assertions, serde(deny_unknown_fields))]
pub struct ForgeInstallProcessor {
    pub sides: Option<Arc<[ForgeSide]>>,
    pub jar: Arc<str>,
    pub classpath: Arc<[Arc<str>]>,
    pub args: Arc<[Ustr]>,
    pub outputs: Option<HashMap<String, String>>,
}
