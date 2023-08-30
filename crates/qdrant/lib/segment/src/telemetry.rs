use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::common::anonymize::Anonymize;
use crate::common::operation_time_statistics::OperationDurationStatistics;
use crate::types::{PayloadIndexInfo, SegmentConfig, SegmentInfo, VectorDataConfig};

#[derive(Serialize, Deserialize, Clone, Debug, JsonSchema)]
pub struct VectorIndexesTelemetry {
    vector_index_searches: Vec<VectorIndexSearchesTelemetry>,
}

#[derive(Serialize, Deserialize, Clone, Debug, JsonSchema)]
pub struct SegmentTelemetry {
    pub info: SegmentInfo,
    pub config: SegmentConfig,
    pub vector_index_searches: Vec<VectorIndexSearchesTelemetry>,
    pub payload_field_indices: Vec<PayloadIndexTelemetry>,
}

#[derive(Serialize, Deserialize, Clone, Debug, JsonSchema)]
pub struct PayloadIndexTelemetry {
    pub field_name: Option<String>,
    pub points_values_count: usize,
    pub points_count: usize,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub histogram_bucket_size: Option<usize>,
}

impl PayloadIndexTelemetry {
    pub fn set_name(mut self, name: String) -> Self {
        self.field_name = Some(name);
        self
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, JsonSchema, Default)]
pub struct VectorIndexSearchesTelemetry {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub index_name: Option<String>,

    #[serde(skip_serializing_if = "OperationDurationStatistics::is_empty")]
    pub unfiltered_plain: OperationDurationStatistics,

    #[serde(skip_serializing_if = "OperationDurationStatistics::is_empty")]
    pub unfiltered_hnsw: OperationDurationStatistics,

    #[serde(skip_serializing_if = "OperationDurationStatistics::is_empty")]
    pub filtered_plain: OperationDurationStatistics,

    #[serde(skip_serializing_if = "OperationDurationStatistics::is_empty")]
    pub filtered_small_cardinality: OperationDurationStatistics,

    #[serde(skip_serializing_if = "OperationDurationStatistics::is_empty")]
    pub filtered_large_cardinality: OperationDurationStatistics,

    #[serde(skip_serializing_if = "OperationDurationStatistics::is_empty")]
    pub filtered_exact: OperationDurationStatistics,

    #[serde(skip_serializing_if = "OperationDurationStatistics::is_empty")]
    pub unfiltered_exact: OperationDurationStatistics,
}

impl Anonymize for SegmentTelemetry {
    fn anonymize(&self) -> Self {
        Self {
            info: self.info.anonymize(),
            config: self.config.anonymize(),
            vector_index_searches: self.vector_index_searches.anonymize(),
            payload_field_indices: self.payload_field_indices.anonymize(),
        }
    }
}

impl Anonymize for SegmentInfo {
    fn anonymize(&self) -> Self {
        SegmentInfo {
            segment_type: self.segment_type,
            num_vectors: self.num_vectors.anonymize(),
            num_points: self.num_points.anonymize(),
            num_deleted_vectors: self.num_deleted_vectors.anonymize(),
            ram_usage_bytes: self.ram_usage_bytes.anonymize(),
            disk_usage_bytes: self.disk_usage_bytes.anonymize(),
            is_appendable: self.is_appendable,
            index_schema: self
                .index_schema
                .iter()
                .map(|(k, v)| (k.anonymize(), v.anonymize()))
                .collect(),
        }
    }
}

impl Anonymize for PayloadIndexInfo {
    fn anonymize(&self) -> Self {
        PayloadIndexInfo {
            data_type: self.data_type,
            params: self.params.clone(),
            points: self.points.anonymize(),
        }
    }
}

impl Anonymize for SegmentConfig {
    fn anonymize(&self) -> Self {
        SegmentConfig {
            vector_data: self.vector_data.anonymize(),
            payload_storage_type: self.payload_storage_type,
        }
    }
}

impl Anonymize for VectorDataConfig {
    fn anonymize(&self) -> Self {
        VectorDataConfig {
            size: self.size.anonymize(),
            distance: self.distance,
            storage_type: self.storage_type,
            index: self.index.clone(),
            quantization_config: None,
        }
    }
}

impl Anonymize for VectorIndexSearchesTelemetry {
    fn anonymize(&self) -> Self {
        VectorIndexSearchesTelemetry {
            index_name: None,
            unfiltered_plain: self.unfiltered_plain.anonymize(),
            unfiltered_hnsw: self.unfiltered_hnsw.anonymize(),
            filtered_plain: self.filtered_plain.anonymize(),
            filtered_small_cardinality: self.filtered_small_cardinality.anonymize(),
            filtered_large_cardinality: self.filtered_large_cardinality.anonymize(),
            filtered_exact: self.filtered_exact.anonymize(),
            unfiltered_exact: self.filtered_exact.anonymize(),
        }
    }
}

impl Anonymize for PayloadIndexTelemetry {
    fn anonymize(&self) -> Self {
        PayloadIndexTelemetry {
            field_name: None,
            points_count: self.points_count.anonymize(),
            points_values_count: self.points_values_count.anonymize(),
            histogram_bucket_size: self.histogram_bucket_size,
        }
    }
}
