use std::collections::{HashMap, HashSet};

use crate::collection_state::ShardInfo;
use crate::shards::shard::{PeerId, ShardId};

#[derive(Debug, Clone)]
pub struct CollectionShardDistribution {
    pub shards: HashMap<ShardId, HashSet<PeerId>>,
}

impl CollectionShardDistribution {
    pub fn all_local(shard_number: Option<u32>, this_peer_id: PeerId) -> Self {
        Self {
            shards: (0..shard_number.unwrap_or(1))
                .map(|shard_id| (shard_id, vec![this_peer_id].into_iter().collect()))
                .collect(),
        }
    }

    pub fn from_shards_info(shards_info: HashMap<ShardId, ShardInfo>) -> Self {
        Self {
            shards: shards_info
                .into_iter()
                .map(|(shard_id, info)| (shard_id, info.replicas.into_keys().collect()))
                .collect(),
        }
    }

    pub fn shard_count(&self) -> usize {
        self.shards.len()
    }

    pub fn shard_replica_count(&self) -> usize {
        self.shards.values().map(|shard| shard.len()).sum()
    }
}
