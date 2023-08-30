use std::path::Path;
use std::sync::Arc;

use async_trait::async_trait;
use segment::types::{
    ExtendedPointId, Filter, ScoredPoint, WithPayload, WithPayloadInterface, WithVector,
};
use tokio::runtime::Handle;

use crate::operations::types::{
    CollectionError, CollectionInfo, CollectionResult, CountRequest, CountResult, PointRequest,
    Record, SearchRequestBatch, UpdateResult,
};
use crate::operations::CollectionUpdateOperations;
use crate::shards::shard_trait::ShardOperation;
use crate::shards::telemetry::LocalShardTelemetry;

#[derive(Clone, Debug)]
pub struct DummyShard {
    message: String,
}

impl DummyShard {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    pub async fn create_snapshot(
        &self,
        _temp_path: &Path,
        _target_path: &Path,
        _save_wal: bool,
    ) -> CollectionResult<()> {
        self.dummy()
    }

    pub async fn on_optimizer_config_update(&self) -> CollectionResult<()> {
        self.dummy()
    }

    pub fn get_telemetry_data(&self) -> LocalShardTelemetry {
        LocalShardTelemetry {
            variant_name: Some("dummy shard".into()),
            segments: vec![],
            optimizations: Default::default(),
        }
    }

    fn dummy<T>(&self) -> CollectionResult<T> {
        Err(CollectionError::service_error(self.message.to_string()))
    }
}

#[async_trait]
impl ShardOperation for DummyShard {
    async fn info(&self) -> CollectionResult<CollectionInfo> {
        self.dummy()
    }

    async fn update(
        &self,
        _: CollectionUpdateOperations,
        _: bool,
    ) -> CollectionResult<UpdateResult> {
        self.dummy()
    }

    /// Forward read-only `scroll_by` to `wrapped_shard`
    async fn scroll_by(
        &self,
        _: Option<ExtendedPointId>,
        _: usize,
        _: &WithPayloadInterface,
        _: &WithVector,
        _: Option<&Filter>,
        _: &Handle,
    ) -> CollectionResult<Vec<Record>> {
        self.dummy()
    }

    async fn search(
        &self,
        _: Arc<SearchRequestBatch>,
        _: &Handle,
    ) -> CollectionResult<Vec<Vec<ScoredPoint>>> {
        self.dummy()
    }

    async fn count(&self, _: Arc<CountRequest>) -> CollectionResult<CountResult> {
        self.dummy()
    }

    async fn retrieve(
        &self,
        _: Arc<PointRequest>,
        _: &WithPayload,
        _: &WithVector,
    ) -> CollectionResult<Vec<Record>> {
        self.dummy()
    }
}
