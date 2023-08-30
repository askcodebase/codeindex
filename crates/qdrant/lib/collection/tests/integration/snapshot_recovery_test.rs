use std::num::{NonZeroU32, NonZeroU64};
use std::sync::Arc;

use collection::collection::Collection;
use collection::config::{CollectionConfig, CollectionParams, WalConfig};
use collection::operations::point_ops::{
    PointInsertOperations, PointOperations, PointStruct, WriteOrdering,
};
use collection::operations::shared_storage_config::SharedStorageConfig;
use collection::operations::types::{NodeType, SearchRequest, VectorParams, VectorsConfig};
use collection::operations::CollectionUpdateOperations;
use collection::shards::channel_service::ChannelService;
use collection::shards::collection_shard_distribution::CollectionShardDistribution;
use collection::shards::replica_set::ReplicaState;
use segment::types::{Distance, WithPayloadInterface, WithVector};
use tempfile::Builder;

use crate::common::{
    dummy_on_replica_failure, dummy_request_shard_transfer, TEST_OPTIMIZERS_CONFIG,
};

async fn _test_snapshot_and_recover_collection(node_type: NodeType) {
    let wal_config = WalConfig {
        wal_capacity_mb: 1,
        wal_segments_ahead: 0,
    };

    let collection_params = CollectionParams {
        vectors: VectorsConfig::Single(VectorParams {
            size: NonZeroU64::new(4).unwrap(),
            distance: Distance::Dot,
            hnsw_config: None,
            quantization_config: None,
            on_disk: None,
        }),
        shard_number: NonZeroU32::new(1).unwrap(),
        replication_factor: NonZeroU32::new(1).unwrap(),
        write_consistency_factor: NonZeroU32::new(1).unwrap(),
        on_disk_payload: false,
    };

    let config = CollectionConfig {
        params: collection_params,
        optimizer_config: TEST_OPTIMIZERS_CONFIG.clone(),
        wal_config,
        hnsw_config: Default::default(),
        quantization_config: Default::default(),
    };

    let snapshots_path = Builder::new().prefix("test_snapshots").tempdir().unwrap();
    let collection_dir = Builder::new().prefix("test_collection").tempdir().unwrap();
    let recover_dir = Builder::new()
        .prefix("test_collection_rec")
        .tempdir()
        .unwrap();
    let collection_name = "test".to_string();
    let collection_name_rec = "test_rec".to_string();

    let storage_config: SharedStorageConfig = SharedStorageConfig {
        node_type,
        ..Default::default()
    };

    let this_peer_id = 0;
    let shard_distribution = CollectionShardDistribution::all_local(
        Some(config.params.shard_number.into()),
        this_peer_id,
    );

    let collection = Collection::new(
        collection_name,
        this_peer_id,
        collection_dir.path(),
        snapshots_path.path(),
        &config,
        Arc::new(storage_config),
        shard_distribution,
        ChannelService::default(),
        dummy_on_replica_failure(),
        dummy_request_shard_transfer(),
        None,
        None,
    )
    .await
    .unwrap();

    let local_shards = collection.get_local_shards().await;
    for shard_id in local_shards {
        collection
            .set_shard_replica_state(shard_id, 0, ReplicaState::Active, None)
            .await
            .unwrap();
    }

    // Upload 1000 random vectors to the collection
    let mut points = Vec::new();
    for i in 0..100 {
        points.push(PointStruct {
            id: i.into(),
            vector: vec![i as f32, 0.0, 0.0, 0.0].into(),
            payload: Some(serde_json::from_str(r#"{"number": "John Doe"}"#).unwrap()),
        });
    }
    let insert_points = CollectionUpdateOperations::PointOperation(PointOperations::UpsertPoints(
        PointInsertOperations::PointsList(points),
    ));
    collection
        .update_from_client(insert_points, true, WriteOrdering::default())
        .await
        .unwrap();

    // Take a snapshot
    let snapshots_temp_dir = Builder::new().prefix("temp_dir").tempdir().unwrap();
    let snapshot_description = collection
        .create_snapshot(snapshots_temp_dir.path(), 0)
        .await
        .unwrap();

    if let Err(err) = Collection::restore_snapshot(
        &snapshots_path.path().join(snapshot_description.name),
        recover_dir.path(),
        0,
        false,
    ) {
        panic!("Failed to restore snapshot: {err}")
    }

    let recovered_collection = Collection::load(
        collection_name_rec,
        this_peer_id,
        recover_dir.path(),
        snapshots_path.path(),
        Default::default(),
        ChannelService::default(),
        dummy_on_replica_failure(),
        dummy_request_shard_transfer(),
        None,
        None,
    )
    .await;

    let query_vector = vec![1.0, 0.0, 0.0, 0.0];

    let full_search_request = SearchRequest {
        vector: query_vector.clone().into(),
        filter: None,
        limit: 100,
        offset: 0,
        with_payload: Some(WithPayloadInterface::Bool(true)),
        with_vector: Some(WithVector::Bool(true)),
        params: None,
        score_threshold: None,
    };

    let reference_result = collection
        .search(full_search_request.clone(), None, None)
        .await
        .unwrap();

    let recovered_result = recovered_collection
        .search(full_search_request, None, None)
        .await
        .unwrap();

    assert_eq!(reference_result.len(), recovered_result.len());

    for (reference, recovered) in reference_result.iter().zip(recovered_result.iter()) {
        assert_eq!(reference.id, recovered.id);
        assert_eq!(reference.payload, recovered.payload);
        assert_eq!(reference.vector, recovered.vector);
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn test_snapshot_and_recover_collection_normal() {
    _test_snapshot_and_recover_collection(NodeType::Normal).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_snapshot_and_recover_collection_listener() {
    _test_snapshot_and_recover_collection(NodeType::Listener).await;
}
