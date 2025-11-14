use bridge::handle::BackendHandle;
use gpui::Entity;

use crate::entity::{
    account::AccountEntries, instance::InstanceEntries, metadata::FrontendMetadata
};

pub mod account;
pub mod instance;
pub mod metadata;

#[derive(Clone)]
pub struct DataEntities {
    pub instances: Entity<InstanceEntries>,
    pub metadata: Entity<FrontendMetadata>,
    pub accounts: Entity<AccountEntries>,
    pub backend_handle: BackendHandle,
}
