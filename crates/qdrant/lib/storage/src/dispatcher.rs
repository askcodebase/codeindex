use std::num::NonZeroU32;
use std::ops::Deref;
use std::sync::Arc;
use std::time::Duration;

use crate::{
    ClusterStatus, CollectionMetaOperations, ConsensusOperations, ConsensusStateRef, StorageError,
    TableOfContent,
};

pub struct Dispatcher {
    toc: Arc<TableOfContent>,
    consensus_state: Option<ConsensusStateRef>,
}

impl Dispatcher {
    pub fn new(toc: Arc<TableOfContent>) -> Self {
        Self {
            toc,
            consensus_state: None,
        }
    }

    pub fn with_consensus(self, state_ref: ConsensusStateRef) -> Self {
        Self {
            consensus_state: Some(state_ref),
            ..self
        }
    }

    pub fn toc(&self) -> &Arc<TableOfContent> {
        &self.toc
    }

    pub fn consensus_state(&self) -> Option<&ConsensusStateRef> {
        self.consensus_state.as_ref()
    }

    /// If `wait_timeout` is not supplied - then default duration will be used.
    /// This function needs to be called from a runtime with timers enabled.
    pub async fn submit_collection_meta_op(
        &self,
        operation: CollectionMetaOperations,
        wait_timeout: Option<Duration>,
    ) -> Result<bool, StorageError> {
        // if distributed deployment is enabled
        if let Some(state) = self.consensus_state.as_ref() {
            // List of operations to await for collection to be operational
            let mut expect_operations: Vec<ConsensusOperations> = vec![];

            let op = match operation {
                CollectionMetaOperations::CreateCollection(mut op) => {
                    self.toc.check_write_lock()?;
                    if !op.is_distribution_set() {
                        // Suggest even distribution of shards across nodes
                        let number_of_peers = state.0.peer_count();
                        let shard_distribution = self
                            .toc
                            .suggest_shard_distribution(
                                &op,
                                NonZeroU32::new(number_of_peers as u32)
                                    .expect("Peer count should be always >= 1"),
                            )
                            .await;

                        // Expect all replicas to become active eventually
                        for (shard_id, peer_ids) in &shard_distribution.distribution {
                            for peer_id in peer_ids {
                                expect_operations.push(ConsensusOperations::initialize_replica(
                                    op.collection_name.clone(),
                                    *shard_id,
                                    *peer_id,
                                ));
                            }
                        }

                        op.set_distribution(shard_distribution);
                    }
                    CollectionMetaOperations::CreateCollection(op)
                }
                op => op,
            };

            let operation_awaiter =
                // If explicit timeout is set - then we need to wait for all expected operations.
                // E.g. in case of `CreateCollection` we will explicitly wait for all replicas to be activated.
                // We need to register receivers(by calling the function) before submitting the operation.
                if !expect_operations.is_empty() {
                    Some(state.await_for_multiple_operations(expect_operations, wait_timeout))
                } else {
                    None
                };

            let res = state
                .propose_consensus_op_with_await(
                    ConsensusOperations::CollectionMeta(Box::new(op)),
                    wait_timeout,
                )
                .await?;

            if let Some(operation_awaiter) = operation_awaiter {
                // Actually await for expected operations to complete on the consensus
                match operation_awaiter.await {
                    Ok(Ok(())) => {} // all good
                    Ok(Err(err)) => {
                        log::warn!("Not all expected operations were completed: {}", err)
                    }
                    Err(err) => log::warn!("Awaiting for expected operations timed out: {}", err),
                }
            }

            Ok(res)
        } else {
            if let CollectionMetaOperations::CreateCollection(_) = &operation {
                self.toc.check_write_lock()?;
            }
            self.toc.perform_collection_meta_op(operation).await
        }
    }

    pub fn cluster_status(&self) -> ClusterStatus {
        match self.consensus_state.as_ref() {
            Some(state) => state.cluster_status(),
            None => ClusterStatus::Disabled,
        }
    }
}

impl Deref for Dispatcher {
    type Target = TableOfContent;

    fn deref(&self) -> &Self::Target {
        self.toc.deref()
    }
}

impl Clone for Dispatcher {
    fn clone(&self) -> Self {
        Self {
            toc: self.toc.clone(),
            consensus_state: self.consensus_state.clone(),
        }
    }
}
