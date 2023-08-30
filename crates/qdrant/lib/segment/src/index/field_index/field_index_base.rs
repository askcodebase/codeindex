use serde_json::Value;
use smol_str::SmolStr;

use crate::common::utils::MultiValue;
use crate::common::Flusher;
use crate::entry::entry_point::OperationResult;
use crate::index::field_index::binary_index::BinaryIndex;
use crate::index::field_index::full_text_index::text_index::FullTextIndex;
use crate::index::field_index::geo_index::GeoMapIndex;
use crate::index::field_index::map_index::MapIndex;
use crate::index::field_index::numeric_index::NumericIndex;
use crate::index::field_index::{CardinalityEstimation, PayloadBlockCondition};
use crate::telemetry::PayloadIndexTelemetry;
use crate::types::{
    FieldCondition, FloatPayloadType, IntPayloadType, Match, MatchText, PayloadKeyType,
    PointOffsetType,
};

pub trait PayloadFieldIndex {
    /// Return number of points with at least one value indexed in here
    fn count_indexed_points(&self) -> usize;

    /// Load index from disk.
    fn load(&mut self) -> OperationResult<bool>;

    /// Remove db content of the current payload index
    fn clear(self) -> OperationResult<()>;

    /// Return function that flushes all pending updates to disk.
    fn flusher(&self) -> Flusher;

    /// Get iterator over points fitting given `condition`
    /// Return `None` if condition does not match the index type
    fn filter<'a>(
        &'a self,
        condition: &'a FieldCondition,
    ) -> Option<Box<dyn Iterator<Item = PointOffsetType> + 'a>>;

    /// Return estimation of points amount which satisfy given condition
    fn estimate_cardinality(&self, condition: &FieldCondition) -> Option<CardinalityEstimation>;

    /// Iterate conditions for payload blocks with minimum size of `threshold`
    /// Required for building HNSW index
    fn payload_blocks(
        &self,
        threshold: usize,
        key: PayloadKeyType,
    ) -> Box<dyn Iterator<Item = PayloadBlockCondition> + '_>;
}

pub trait ValueIndexer<T> {
    /// Add multiple values associated with a single point
    /// This function should be called only once for each point
    fn add_many(&mut self, id: PointOffsetType, values: Vec<T>) -> OperationResult<()>;

    /// Extract index-able value from payload `Value`
    fn get_value(&self, value: &Value) -> Option<T>;

    /// Try to extract index-able values from payload `Value`, even if it is an array
    fn get_values(&self, value: &Value) -> Vec<T> {
        match value {
            Value::Array(values) => values.iter().flat_map(|x| self.get_value(x)).collect(),
            _ => self.get_value(value).map(|x| vec![x]).unwrap_or_default(),
        }
    }

    /// Add point with payload to index
    fn add_point(
        &mut self,
        id: PointOffsetType,
        payload: &MultiValue<&Value>,
    ) -> OperationResult<()> {
        match payload {
            MultiValue::Multiple(values) => {
                self.remove_point(id)?;
                let mut flatten_values: Vec<_> = vec![];

                for value in values {
                    match value {
                        Value::Array(values) => {
                            flatten_values.extend(values.iter().flat_map(|x| self.get_value(x)));
                        }
                        _ => {
                            if let Some(x) = self.get_value(value) {
                                flatten_values.push(x);
                            }
                        }
                    }
                }
                self.add_many(id, flatten_values)
            }
            MultiValue::Single(Some(Value::Array(values))) => {
                self.remove_point(id)?;
                self.add_many(id, values.iter().flat_map(|x| self.get_value(x)).collect())
            }
            MultiValue::Single(Some(value)) => {
                if let Some(x) = self.get_value(value) {
                    self.remove_point(id)?;
                    self.add_many(id, vec![x])
                } else {
                    Ok(())
                }
            }
            MultiValue::Single(None) => Ok(()),
        }
    }

    /// remove a point from the index
    fn remove_point(&mut self, id: PointOffsetType) -> OperationResult<()>;
}

/// Common interface for all possible types of field indexes
/// Enables polymorphism on field indexes
/// TODO: Rename with major release
#[allow(clippy::enum_variant_names)]
pub enum FieldIndex {
    IntIndex(NumericIndex<IntPayloadType>),
    IntMapIndex(MapIndex<IntPayloadType>),
    KeywordIndex(MapIndex<SmolStr>),
    FloatIndex(NumericIndex<FloatPayloadType>),
    GeoIndex(GeoMapIndex),
    FullTextIndex(FullTextIndex),
    BinaryIndex(BinaryIndex),
}

impl FieldIndex {
    /// Try to check condition for a payload given a field index.
    /// Required because some index parameters may influence the condition checking logic.
    /// For example, full text index may have different tokenizers.
    ///
    /// Returns `None` if there is no special logic for the given index
    /// returns `Some(true)` if condition is satisfied
    /// returns `Some(false)` if condition is not satisfied
    pub fn check_condition(
        &self,
        condition: &FieldCondition,
        payload_value: &Value,
    ) -> Option<bool> {
        match self {
            FieldIndex::IntIndex(_) => None,
            FieldIndex::IntMapIndex(_) => None,
            FieldIndex::KeywordIndex(_) => None,
            FieldIndex::FloatIndex(_) => None,
            FieldIndex::GeoIndex(_) => None,
            FieldIndex::BinaryIndex(_) => None,
            FieldIndex::FullTextIndex(full_text_index) => match &condition.r#match {
                Some(Match::Text(MatchText { text })) => {
                    let query = full_text_index.parse_query(text);
                    for value in full_text_index.get_values(payload_value) {
                        let document = full_text_index.parse_document(&value);
                        if query.check_match(&document) {
                            return Some(true);
                        }
                    }
                    Some(false)
                }
                _ => None,
            },
        }
    }

    fn get_payload_field_index(&self) -> &dyn PayloadFieldIndex {
        match self {
            FieldIndex::IntIndex(payload_field_index) => payload_field_index,
            FieldIndex::IntMapIndex(payload_field_index) => payload_field_index,
            FieldIndex::KeywordIndex(payload_field_index) => payload_field_index,
            FieldIndex::FloatIndex(payload_field_index) => payload_field_index,
            FieldIndex::GeoIndex(payload_field_index) => payload_field_index,
            FieldIndex::BinaryIndex(payload_field_index) => payload_field_index,
            FieldIndex::FullTextIndex(payload_field_index) => payload_field_index,
        }
    }

    #[allow(dead_code)]
    fn get_payload_field_index_mut(&mut self) -> &mut dyn PayloadFieldIndex {
        match self {
            FieldIndex::IntIndex(ref mut payload_field_index) => payload_field_index,
            FieldIndex::IntMapIndex(ref mut payload_field_index) => payload_field_index,
            FieldIndex::KeywordIndex(ref mut payload_field_index) => payload_field_index,
            FieldIndex::FloatIndex(ref mut payload_field_index) => payload_field_index,
            FieldIndex::GeoIndex(ref mut payload_field_index) => payload_field_index,
            FieldIndex::BinaryIndex(ref mut payload_field_index) => payload_field_index,
            FieldIndex::FullTextIndex(ref mut payload_field_index) => payload_field_index,
        }
    }

    pub fn load(&mut self) -> OperationResult<bool> {
        match self {
            FieldIndex::IntIndex(ref mut payload_field_index) => payload_field_index.load(),
            FieldIndex::IntMapIndex(ref mut payload_field_index) => payload_field_index.load(),
            FieldIndex::KeywordIndex(ref mut payload_field_index) => payload_field_index.load(),
            FieldIndex::FloatIndex(ref mut payload_field_index) => payload_field_index.load(),
            FieldIndex::GeoIndex(ref mut payload_field_index) => payload_field_index.load(),
            FieldIndex::BinaryIndex(ref mut payload_field_index) => payload_field_index.load(),
            FieldIndex::FullTextIndex(ref mut payload_field_index) => payload_field_index.load(),
        }
    }

    pub fn clear(self) -> OperationResult<()> {
        match self {
            FieldIndex::IntIndex(index) => index.clear(),
            FieldIndex::IntMapIndex(index) => index.clear(),
            FieldIndex::KeywordIndex(index) => index.clear(),
            FieldIndex::FloatIndex(index) => index.clear(),
            FieldIndex::GeoIndex(index) => index.clear(),
            FieldIndex::BinaryIndex(index) => index.clear(),
            FieldIndex::FullTextIndex(index) => index.clear(),
        }
    }

    pub fn recreate(&self) -> OperationResult<()> {
        match self {
            FieldIndex::IntIndex(index) => index.recreate(),
            FieldIndex::IntMapIndex(index) => index.recreate(),
            FieldIndex::KeywordIndex(index) => index.recreate(),
            FieldIndex::FloatIndex(index) => index.recreate(),
            FieldIndex::GeoIndex(index) => index.recreate(),
            FieldIndex::BinaryIndex(index) => index.recreate(),
            FieldIndex::FullTextIndex(index) => index.recreate(),
        }
    }

    pub fn count_indexed_points(&self) -> usize {
        self.get_payload_field_index().count_indexed_points()
    }

    pub fn flusher(&self) -> Flusher {
        self.get_payload_field_index().flusher()
    }

    pub fn filter<'a>(
        &'a self,
        condition: &'a FieldCondition,
    ) -> Option<Box<dyn Iterator<Item = PointOffsetType> + 'a>> {
        self.get_payload_field_index().filter(condition)
    }

    pub fn estimate_cardinality(
        &self,
        condition: &FieldCondition,
    ) -> Option<CardinalityEstimation> {
        self.get_payload_field_index()
            .estimate_cardinality(condition)
    }

    pub fn payload_blocks(
        &self,
        threshold: usize,
        key: PayloadKeyType,
    ) -> Box<dyn Iterator<Item = PayloadBlockCondition> + '_> {
        self.get_payload_field_index()
            .payload_blocks(threshold, key)
    }

    pub fn add_point(
        &mut self,
        id: PointOffsetType,
        payload: &MultiValue<&Value>,
    ) -> OperationResult<()> {
        match self {
            FieldIndex::IntIndex(ref mut payload_field_index) => {
                payload_field_index.add_point(id, payload)
            }
            FieldIndex::IntMapIndex(ref mut payload_field_index) => {
                payload_field_index.add_point(id, payload)
            }
            FieldIndex::KeywordIndex(ref mut payload_field_index) => {
                payload_field_index.add_point(id, payload)
            }
            FieldIndex::FloatIndex(ref mut payload_field_index) => {
                payload_field_index.add_point(id, payload)
            }
            FieldIndex::GeoIndex(ref mut payload_field_index) => {
                payload_field_index.add_point(id, payload)
            }
            FieldIndex::BinaryIndex(ref mut payload_field_index) => {
                payload_field_index.add_point(id, payload)
            }
            FieldIndex::FullTextIndex(ref mut payload_field_index) => {
                payload_field_index.add_point(id, payload)
            }
        }
    }

    pub fn remove_point(&mut self, point_id: PointOffsetType) -> OperationResult<()> {
        match self {
            FieldIndex::IntIndex(index) => index.remove_point(point_id),
            FieldIndex::IntMapIndex(index) => index.remove_point(point_id),
            FieldIndex::KeywordIndex(index) => index.remove_point(point_id),
            FieldIndex::FloatIndex(index) => index.remove_point(point_id),
            FieldIndex::GeoIndex(index) => index.remove_point(point_id),
            FieldIndex::BinaryIndex(index) => index.remove_point(point_id),
            FieldIndex::FullTextIndex(index) => index.remove_point(point_id),
        }
    }

    pub fn get_telemetry_data(&self) -> PayloadIndexTelemetry {
        match self {
            FieldIndex::IntIndex(index) => index.get_telemetry_data(),
            FieldIndex::IntMapIndex(index) => index.get_telemetry_data(),
            FieldIndex::KeywordIndex(index) => index.get_telemetry_data(),
            FieldIndex::FloatIndex(index) => index.get_telemetry_data(),
            FieldIndex::GeoIndex(index) => index.get_telemetry_data(),
            FieldIndex::BinaryIndex(index) => index.get_telemetry_data(),
            FieldIndex::FullTextIndex(index) => index.get_telemetry_data(),
        }
    }

    pub fn values_count(&self, point_id: PointOffsetType) -> usize {
        match self {
            FieldIndex::IntIndex(index) => index.values_count(point_id),
            FieldIndex::IntMapIndex(index) => index.values_count(point_id),
            FieldIndex::KeywordIndex(index) => index.values_count(point_id),
            FieldIndex::FloatIndex(index) => index.values_count(point_id),
            FieldIndex::GeoIndex(index) => index.values_count(point_id),
            FieldIndex::BinaryIndex(index) => index.values_count(point_id),
            FieldIndex::FullTextIndex(index) => index.values_count(point_id),
        }
    }

    pub fn values_is_empty(&self, point_id: PointOffsetType) -> bool {
        match self {
            FieldIndex::IntIndex(index) => index.values_is_empty(point_id),
            FieldIndex::IntMapIndex(index) => index.values_is_empty(point_id),
            FieldIndex::KeywordIndex(index) => index.values_is_empty(point_id),
            FieldIndex::FloatIndex(index) => index.values_is_empty(point_id),
            FieldIndex::GeoIndex(index) => index.values_is_empty(point_id),
            FieldIndex::BinaryIndex(index) => index.values_is_empty(point_id),
            FieldIndex::FullTextIndex(index) => index.values_is_empty(point_id),
        }
    }
}
