use std::path::{Path, PathBuf};

use crate::operations::types::{CollectionError, CollectionResult};
use crate::shards::shard::ShardId;
use crate::shards::shard_config::{ShardConfig, ShardType};
use crate::shards::ShardVersion;

async fn shards_versions(
    collection_path: &Path,
    shard_id: ShardId,
) -> CollectionResult<Vec<(ShardVersion, PathBuf)>> {
    let mut entries = tokio::fs::read_dir(collection_path).await?;
    let mut all_versions: Vec<(ShardVersion, PathBuf)> = vec![];
    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        if path.is_dir() {
            let file_name_opt = path.file_name().and_then(|file_name| file_name.to_str());

            if file_name_opt.is_none() {
                continue;
            }

            let file_name = file_name_opt.unwrap();

            if file_name.starts_with(&format!("{shard_id}-")) {
                let version_opt = file_name
                    .split('-')
                    .nth(1)
                    .and_then(|version_str| version_str.parse::<ShardVersion>().ok());

                if version_opt.is_none() {
                    continue;
                }
                let version = version_opt.unwrap();
                all_versions.push((version, path.clone()));
            } else if file_name == format!("{shard_id}") {
                let version = 0;
                all_versions.push((version, path.clone()));
            }
        }
    }
    all_versions.sort_by_key(|(version, _)| *version);
    all_versions = all_versions.into_iter().rev().collect();
    Ok(all_versions)
}

pub async fn drop_old_shards(collection_path: &Path, shard_id: ShardId) -> CollectionResult<()> {
    for (_version, old_path) in shards_versions(collection_path, shard_id)
        .await?
        .into_iter()
        .skip(1)
    {
        // delete old shard's data folder
        tokio::fs::remove_dir_all(&old_path).await?;
    }
    Ok(())
}

pub fn versioned_shard_path(
    collection_path: &Path,
    shard_id: ShardId,
    version: ShardVersion,
) -> PathBuf {
    if version == 0 {
        collection_path.join(format!("{shard_id}"))
    } else {
        collection_path.join(format!("{shard_id}-{version}"))
    }
}

pub async fn latest_shard_paths(
    collection_path: &Path,
    shard_id: ShardId,
) -> CollectionResult<Vec<(PathBuf, ShardVersion, ShardType)>> {
    // Assume `all_versions` is sorted by version in descending order.
    let mut res = vec![];
    let mut seen_temp_shard = false;
    let all_versions = shards_versions(collection_path, shard_id).await?;
    for (version, path) in all_versions {
        let shard_config_opt = ShardConfig::load(&path)?;

        if let Some(shard_config) = shard_config_opt {
            match shard_config.r#type {
                ShardType::Local => {
                    res.push((path, version, shard_config.r#type));
                    break; // We don't need older local shards.
                }
                ShardType::Remote { .. } => {
                    res.push((path, version, shard_config.r#type));
                    break; // We don't need older remote shards.
                }
                ShardType::Temporary => {
                    if !seen_temp_shard {
                        res.push((path, version, shard_config.r#type));
                        seen_temp_shard = true;
                    }
                }
                ShardType::ReplicaSet => {
                    res.push((path, version, shard_config.r#type));
                    break; // We don't need older replica set shards.
                }
            }
        } else {
            log::warn!("Shard config not found for {}, skipping", path.display());
        }
    }
    if (seen_temp_shard && res.len() < 2) || res.is_empty() {
        return Err(CollectionError::service_error(format!(
            "No shard found: {shard_id} at {collection_path}",
            shard_id = shard_id,
            collection_path = collection_path.display()
        )));
    }
    Ok(res)
}
