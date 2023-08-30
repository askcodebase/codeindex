use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Write};
use std::num::NonZeroU32;
use std::path::Path;

use atomicwrites::AtomicFile;
use atomicwrites::OverwriteBehavior::AllowOverwrite;
use schemars::JsonSchema;
use segment::common::anonymize::Anonymize;
use segment::data_types::vectors::DEFAULT_VECTOR_NAME;
use segment::types::{
    HnswConfig, Indexes, QuantizationConfig, VectorDataConfig, VectorStorageType,
};
use serde::{Deserialize, Serialize};
use validator::Validate;
use wal::WalOptions;

use crate::operations::config_diff::{DiffConfig, QuantizationConfigDiff};
use crate::operations::types::{
    CollectionError, CollectionResult, VectorParams, VectorParamsDiff, VectorsConfig,
    VectorsConfigDiff,
};
use crate::operations::validation;
use crate::optimizers_builder::OptimizersConfig;

pub const COLLECTION_CONFIG_FILE: &str = "config.json";

#[derive(Debug, Deserialize, Serialize, JsonSchema, Validate, Clone, PartialEq, Eq)]
pub struct WalConfig {
    /// Size of a single WAL segment in MB
    #[validate(range(min = 1))]
    pub wal_capacity_mb: usize,
    /// Number of WAL segments to create ahead of actually used ones
    pub wal_segments_ahead: usize,
}

impl From<&WalConfig> for WalOptions {
    fn from(config: &WalConfig) -> Self {
        WalOptions {
            segment_capacity: config.wal_capacity_mb * 1024 * 1024,
            segment_queue_len: config.wal_segments_ahead,
        }
    }
}

impl Default for WalConfig {
    fn default() -> Self {
        WalConfig {
            wal_capacity_mb: 32,
            wal_segments_ahead: 0,
        }
    }
}

#[derive(Debug, Deserialize, Serialize, JsonSchema, Validate, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct CollectionParams {
    /// Configuration of the vector storage
    #[validate]
    pub vectors: VectorsConfig,
    /// Number of shards the collection has
    #[serde(default = "default_shard_number")]
    pub shard_number: NonZeroU32,
    /// Number of replicas for each shard
    #[serde(default = "default_replication_factor")]
    pub replication_factor: NonZeroU32,
    /// Defines how many replicas should apply the operation for us to consider it successful.
    /// Increasing this number will make the collection more resilient to inconsistencies, but will
    /// also make it fail if not enough replicas are available.
    /// Does not have any performance impact.
    #[serde(default = "default_write_consistency_factor")]
    pub write_consistency_factor: NonZeroU32,
    /// If true - point's payload will not be stored in memory.
    /// It will be read from the disk every time it is requested.
    /// This setting saves RAM by (slightly) increasing the response time.
    /// Note: those payload values that are involved in filtering and are indexed - remain in RAM.
    #[serde(default = "default_on_disk_payload")]
    pub on_disk_payload: bool,
}

impl Anonymize for CollectionParams {
    fn anonymize(&self) -> Self {
        CollectionParams {
            vectors: self.vectors.anonymize(),
            shard_number: self.shard_number,
            replication_factor: self.replication_factor,
            write_consistency_factor: self.write_consistency_factor,
            on_disk_payload: self.on_disk_payload,
        }
    }
}

fn default_shard_number() -> NonZeroU32 {
    NonZeroU32::new(1).unwrap()
}

pub fn default_replication_factor() -> NonZeroU32 {
    NonZeroU32::new(1).unwrap()
}

pub fn default_write_consistency_factor() -> NonZeroU32 {
    NonZeroU32::new(1).unwrap()
}

const fn default_on_disk_payload() -> bool {
    false
}

#[derive(Debug, Deserialize, Serialize, JsonSchema, Validate, Clone, PartialEq)]
pub struct CollectionConfig {
    #[validate]
    pub params: CollectionParams,
    #[validate]
    pub hnsw_config: HnswConfig,
    #[validate]
    pub optimizer_config: OptimizersConfig,
    #[validate]
    pub wal_config: WalConfig,
    #[serde(default)]
    pub quantization_config: Option<QuantizationConfig>,
}

impl CollectionConfig {
    pub fn save(&self, path: &Path) -> CollectionResult<()> {
        let config_path = path.join(COLLECTION_CONFIG_FILE);
        let af = AtomicFile::new(&config_path, AllowOverwrite);
        let state_bytes = serde_json::to_vec(self).unwrap();
        af.write(|f| f.write_all(&state_bytes)).map_err(|err| {
            CollectionError::service_error(format!("Can't write {config_path:?}, error: {err}"))
        })?;
        Ok(())
    }

    pub fn load(path: &Path) -> CollectionResult<Self> {
        let config_path = path.join(COLLECTION_CONFIG_FILE);
        let mut contents = String::new();
        let mut file = File::open(config_path)?;
        file.read_to_string(&mut contents)?;
        Ok(serde_json::from_str(&contents)?)
    }

    /// Check if collection config exists
    pub fn check(path: &Path) -> bool {
        let config_path = path.join(COLLECTION_CONFIG_FILE);
        config_path.exists()
    }

    pub fn validate_and_warn(&self) {
        if let Err(ref errs) = self.validate() {
            validation::warn_validation_errors("Collection configuration file", errs);
        }
    }
}

impl CollectionParams {
    pub fn get_vector_params(&self, vector_name: &str) -> CollectionResult<VectorParams> {
        self.vectors
            .get_params(vector_name)
            .cloned()
            .ok_or_else(|| CollectionError::BadInput {
                description: if vector_name == DEFAULT_VECTOR_NAME {
                    "Default vector params are not specified in config".into()
                } else {
                    format!("Vector params for {vector_name} are not specified in config")
                },
            })
    }

    fn get_vector_params_mut(&mut self, vector_name: &str) -> CollectionResult<&mut VectorParams> {
        self.vectors
            .get_params_mut(vector_name)
            .ok_or_else(|| CollectionError::BadInput {
                description: if vector_name == DEFAULT_VECTOR_NAME {
                    "Default vector params are not specified in config".into()
                } else {
                    format!("Vector params for {vector_name} are not specified in config")
                },
            })
    }

    /// Update collection vectors from the given update vectors config
    pub fn update_vectors_from_diff(
        &mut self,
        update_vectors_diff: &VectorsConfigDiff,
    ) -> CollectionResult<()> {
        for (vector_name, update_params) in update_vectors_diff.0.iter() {
            let vector_params = self.get_vector_params_mut(vector_name)?;

            let VectorParamsDiff {
                hnsw_config,
                quantization_config,
                on_disk,
            } = update_params.clone();

            if let Some(hnsw_diff) = hnsw_config {
                if let Some(existing_hnsw) = &vector_params.hnsw_config {
                    vector_params.hnsw_config = Some(hnsw_diff.update(existing_hnsw)?);
                } else {
                    vector_params.hnsw_config = Some(hnsw_diff);
                }
            }

            if let Some(quantization_diff) = quantization_config {
                vector_params.quantization_config = match quantization_diff.clone() {
                    QuantizationConfigDiff::Scalar(scalar) => {
                        Some(QuantizationConfig::Scalar(scalar))
                    }
                    QuantizationConfigDiff::Product(product) => {
                        Some(QuantizationConfig::Product(product))
                    }
                    QuantizationConfigDiff::Disabled(_) => None,
                }
            }

            if let Some(on_disk) = on_disk {
                vector_params.on_disk = Some(on_disk);
            }
        }
        Ok(())
    }

    /// Convert into unoptimized named vector data configs
    ///
    /// It is the job of the segment optimizer to change this configuration with optimized settings
    /// based on threshold configurations.
    pub fn into_base_vector_data(&self) -> CollectionResult<HashMap<String, VectorDataConfig>> {
        Ok(self
            .vectors
            .params_iter()
            .map(|(name, params)| {
                (
                    name.into(),
                    VectorDataConfig {
                        size: params.size.get() as usize,
                        distance: params.distance,
                        // Plain (disabled) index
                        index: Indexes::Plain {},
                        // Disabled quantization
                        quantization_config: None,
                        // Default to in memory storage
                        storage_type: if params.on_disk.unwrap_or_default() {
                            VectorStorageType::ChunkedMmap
                        } else {
                            VectorStorageType::Memory
                        },
                    },
                )
            })
            .collect())
    }
}
