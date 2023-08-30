pub mod channel_service;
pub mod collection_shard_distribution;
mod conversions;
pub mod dummy_shard;
pub mod forward_proxy_shard;
pub mod local_shard;
pub mod local_shard_operations;
pub mod proxy_shard;
pub mod remote_shard;
#[allow(dead_code)]
pub mod replica_set;
pub mod resolve;
pub mod shard;
pub mod shard_config;
pub mod shard_holder;
pub mod shard_trait;
pub mod shard_versioning;
pub mod telemetry;
pub mod transfer;

use std::path::{Path, PathBuf};

use shard::ShardId;

use crate::operations::types::{CollectionError, CollectionResult};
use crate::shards::shard_versioning::versioned_shard_path;

pub const HASH_RING_SHARD_SCALE: u32 = 100;

pub type CollectionId = String;

pub type ShardVersion = usize;

pub async fn create_shard_dir(
    collection_path: &Path,
    shard_id: ShardId,
) -> CollectionResult<PathBuf> {
    let shard_path = versioned_shard_path(collection_path, shard_id, 0);
    match tokio::fs::create_dir(&shard_path).await {
        Ok(_) => Ok(shard_path),
        Err(e) => {
            if e.kind() == std::io::ErrorKind::AlreadyExists {
                Err(CollectionError::service_error(format!(
                    "shard path already exists: {:?}",
                    shard_path
                )))
            } else {
                Err(CollectionError::from(e))
            }
        }
    }
}
