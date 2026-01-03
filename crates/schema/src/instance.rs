use serde::{Deserialize, Serialize};
use ustr::Ustr;

use crate::loader::Loader;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct InstanceConfiguration {
    pub minecraft_version: Ustr,
    pub loader: Loader,
    #[serde(default, deserialize_with = "crate::try_deserialize", skip_serializing_if = "Option::is_none")]
    pub memory: Option<InstanceMemoryConfiguration>
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct InstanceMemoryConfiguration {
    pub min: u32,
    pub max: u32,
}
