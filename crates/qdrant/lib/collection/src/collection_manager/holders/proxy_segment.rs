use std::cmp::max;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use parking_lot::{RwLock, RwLockUpgradableReadGuard};
use segment::data_types::named_vectors::NamedVectors;
use segment::data_types::vectors::VectorElementType;
use segment::entry::entry_point::{OperationResult, SegmentEntry, SegmentFailedState};
use segment::index::field_index::CardinalityEstimation;
use segment::telemetry::SegmentTelemetry;
use segment::types::{
    Condition, Filter, Payload, PayloadFieldSchema, PayloadKeyType, PayloadKeyTypeRef, PointIdType,
    ScoredPoint, SearchParams, SegmentConfig, SegmentInfo, SegmentType, SeqNumberType, WithPayload,
    WithVector,
};

use crate::collection_manager::holders::segment_holder::LockedSegment;

type LockedRmSet = Arc<RwLock<HashSet<PointIdType>>>;
type LockedFieldsSet = Arc<RwLock<HashSet<PayloadKeyType>>>;
type LockedFieldsMap = Arc<RwLock<HashMap<PayloadKeyType, PayloadFieldSchema>>>;

/// This object is a wrapper around read-only segment.
/// It could be used to provide all read and write operations while wrapped segment is being optimized (i.e. not available for writing)
/// It writes all changed records into a temporary `write_segment` and keeps track on changed points
pub struct ProxySegment {
    pub write_segment: LockedSegment,
    pub wrapped_segment: LockedSegment,
    /// Points which should not longer used from wrapped_segment
    /// May contain points which are not in wrapped_segment,
    /// because the set is shared among all proxy segments
    deleted_points: LockedRmSet,
    deleted_indexes: LockedFieldsSet,
    created_indexes: LockedFieldsMap,
    last_flushed_version: Arc<RwLock<Option<SeqNumberType>>>,
}

impl ProxySegment {
    pub fn new(
        segment: LockedSegment,
        write_segment: LockedSegment,
        deleted_points: LockedRmSet,
        created_indexes: LockedFieldsMap,
        deleted_indexes: LockedFieldsSet,
    ) -> Self {
        ProxySegment {
            write_segment,
            wrapped_segment: segment,
            deleted_points,
            created_indexes,
            deleted_indexes,
            last_flushed_version: Arc::new(RwLock::new(None)),
        }
    }

    /// Ensure that write segment have same indexes as wrapped segment
    pub fn replicate_field_indexes(&mut self, op_num: SeqNumberType) -> OperationResult<()> {
        let existing_indexes = self.write_segment.get().read().get_indexed_fields();
        let expected_indexes = self.wrapped_segment.get().read().get_indexed_fields();
        // create missing indexes
        for (expected_field, expected_schema) in &expected_indexes {
            let existing_schema = existing_indexes.get(expected_field);

            if existing_schema != Some(expected_schema) {
                if existing_schema.is_some() {
                    self.write_segment
                        .get()
                        .write()
                        .delete_field_index(op_num, expected_field)?;
                }
                self.write_segment.get().write().create_field_index(
                    op_num,
                    expected_field,
                    Some(expected_schema),
                )?;
            }
        }
        // remove extra indexes
        for existing_field in existing_indexes.keys() {
            if !expected_indexes.contains_key(existing_field) {
                self.write_segment
                    .get()
                    .write()
                    .delete_field_index(op_num, existing_field)?;
            }
        }
        Ok(())
    }

    fn move_if_exists(
        &self,
        op_num: SeqNumberType,
        point_id: PointIdType,
    ) -> OperationResult<bool> {
        let deleted_points_guard = self.deleted_points.upgradable_read();
        if deleted_points_guard.contains(&point_id) {
            // Point is already removed from wrapped segment
            return Ok(false);
        }
        let wrapped_segment = self.wrapped_segment.get();
        let wrapped_segment_guard = wrapped_segment.read();
        if !wrapped_segment_guard.has_point(point_id) {
            // Point is not in wrapped segment
            return Ok(false);
        }

        let (all_vectors, payload) = (
            wrapped_segment_guard.all_vectors(point_id)?,
            wrapped_segment_guard.payload(point_id)?,
        );

        {
            let mut deleted_points_write = RwLockUpgradableReadGuard::upgrade(deleted_points_guard);
            deleted_points_write.insert(point_id);
        }

        let segment_arc = self.write_segment.get();
        let mut write_segment = segment_arc.write();

        write_segment.upsert_point(op_num, point_id, all_vectors)?;
        write_segment.set_full_payload(op_num, point_id, &payload)?;

        Ok(true)
    }

    fn add_deleted_points_condition_to_filter(
        &self,
        filter: Option<&Filter>,
        deleted_points: &HashSet<PointIdType>,
    ) -> Filter {
        let wrapper_condition = Condition::HasId(deleted_points.clone().into());
        match filter {
            None => Filter::new_must_not(wrapper_condition),
            Some(f) => {
                let mut new_filter = f.clone();
                let must_not = new_filter.must_not;

                let new_must_not = match must_not {
                    None => Some(vec![wrapper_condition]),
                    Some(mut conditions) => {
                        conditions.push(wrapper_condition);
                        Some(conditions)
                    }
                };
                new_filter.must_not = new_must_not;
                new_filter
            }
        }
    }
}

impl SegmentEntry for ProxySegment {
    fn version(&self) -> SeqNumberType {
        max(
            self.wrapped_segment.get().read().version(),
            self.write_segment.get().read().version(),
        )
    }

    fn point_version(&self, point_id: PointIdType) -> Option<SeqNumberType> {
        // Write version is always higher if presence
        self.write_segment
            .get()
            .read()
            .point_version(point_id)
            .or_else(|| self.wrapped_segment.get().read().point_version(point_id))
    }

    fn search(
        &self,
        vector_name: &str,
        vector: &[VectorElementType],
        with_payload: &WithPayload,
        with_vector: &WithVector,
        filter: Option<&Filter>,
        top: usize,
        params: Option<&SearchParams>,
        is_stopped: &AtomicBool,
    ) -> OperationResult<Vec<ScoredPoint>> {
        let deleted_points = self.deleted_points.read();

        // Some point might be deleted after temporary segment creation
        // We need to prevent them from being found by search request
        // That is why we need to pass additional filter for deleted points
        let do_update_filter = !deleted_points.is_empty();
        let mut wrapped_result = if do_update_filter {
            // ToDo: Come up with better way to pass deleted points into Filter
            // e.g. implement AtomicRefCell for Serializer.
            // This copy might slow process down if there will be a lot of deleted points
            let wrapped_filter =
                self.add_deleted_points_condition_to_filter(filter, &deleted_points);

            self.wrapped_segment.get().read().search(
                vector_name,
                vector,
                with_payload,
                with_vector,
                Some(&wrapped_filter),
                top,
                params,
                is_stopped,
            )?
        } else {
            self.wrapped_segment.get().read().search(
                vector_name,
                vector,
                with_payload,
                with_vector,
                filter,
                top,
                params,
                is_stopped,
            )?
        };

        let mut write_result = self.write_segment.get().read().search(
            vector_name,
            vector,
            with_payload,
            with_vector,
            filter,
            top,
            params,
            is_stopped,
        )?;

        wrapped_result.append(&mut write_result);
        Ok(wrapped_result)
    }

    fn search_batch(
        &self,
        vector_name: &str,
        vectors: &[&[VectorElementType]],
        with_payload: &WithPayload,
        with_vector: &WithVector,
        filter: Option<&Filter>,
        top: usize,
        params: Option<&SearchParams>,
        is_stopped: &AtomicBool,
    ) -> OperationResult<Vec<Vec<ScoredPoint>>> {
        let deleted_points = self.deleted_points.read();

        // Some point might be deleted after temporary segment creation
        // We need to prevent them from being found by search request
        // That is why we need to pass additional filter for deleted points
        let do_update_filter = !deleted_points.is_empty();
        let mut wrapped_results = if do_update_filter {
            // ToDo: Come up with better way to pass deleted points into Filter
            // e.g. implement AtomicRefCell for Serializer.
            // This copy might slow process down if there will be a lot of deleted points
            let wrapped_filter =
                self.add_deleted_points_condition_to_filter(filter, &deleted_points);

            self.wrapped_segment.get().read().search_batch(
                vector_name,
                vectors,
                with_payload,
                with_vector,
                Some(&wrapped_filter),
                top,
                params,
                is_stopped,
            )?
        } else {
            self.wrapped_segment.get().read().search_batch(
                vector_name,
                vectors,
                with_payload,
                with_vector,
                filter,
                top,
                params,
                is_stopped,
            )?
        };
        let mut write_results = self.write_segment.get().read().search_batch(
            vector_name,
            vectors,
            with_payload,
            with_vector,
            filter,
            top,
            params,
            is_stopped,
        )?;
        for (index, write_result) in write_results.iter_mut().enumerate() {
            wrapped_results[index].append(write_result)
        }
        Ok(wrapped_results)
    }

    fn upsert_point(
        &mut self,
        op_num: SeqNumberType,
        point_id: PointIdType,
        vectors: NamedVectors,
    ) -> OperationResult<bool> {
        self.move_if_exists(op_num, point_id)?;
        self.write_segment
            .get()
            .write()
            .upsert_point(op_num, point_id, vectors)
    }

    fn delete_point(
        &mut self,
        op_num: SeqNumberType,
        point_id: PointIdType,
    ) -> OperationResult<bool> {
        let mut was_deleted = false;
        if self.wrapped_segment.get().read().has_point(point_id) {
            was_deleted = self.deleted_points.write().insert(point_id);
        }
        let was_deleted_in_writable = self
            .write_segment
            .get()
            .write()
            .delete_point(op_num, point_id)?;

        Ok(was_deleted || was_deleted_in_writable)
    }

    fn update_vectors(
        &mut self,
        op_num: SeqNumberType,
        point_id: PointIdType,
        vectors: NamedVectors,
    ) -> OperationResult<bool> {
        self.move_if_exists(op_num, point_id)?;
        self.write_segment
            .get()
            .write()
            .update_vectors(op_num, point_id, vectors)
    }

    fn delete_vector(
        &mut self,
        op_num: SeqNumberType,
        point_id: PointIdType,
        vector_name: &str,
    ) -> OperationResult<bool> {
        self.move_if_exists(op_num, point_id)?;
        self.write_segment
            .get()
            .write()
            .delete_vector(op_num, point_id, vector_name)
    }

    fn set_full_payload(
        &mut self,
        op_num: SeqNumberType,
        point_id: PointIdType,
        full_payload: &Payload,
    ) -> OperationResult<bool> {
        self.move_if_exists(op_num, point_id)?;
        self.write_segment
            .get()
            .write()
            .set_full_payload(op_num, point_id, full_payload)
    }

    fn set_payload(
        &mut self,
        op_num: SeqNumberType,
        point_id: PointIdType,
        payload: &Payload,
    ) -> OperationResult<bool> {
        self.move_if_exists(op_num, point_id)?;
        self.write_segment
            .get()
            .write()
            .set_payload(op_num, point_id, payload)
    }

    fn delete_payload(
        &mut self,
        op_num: SeqNumberType,
        point_id: PointIdType,
        key: PayloadKeyTypeRef,
    ) -> OperationResult<bool> {
        self.move_if_exists(op_num, point_id)?;
        self.write_segment
            .get()
            .write()
            .delete_payload(op_num, point_id, key)
    }

    fn clear_payload(
        &mut self,
        op_num: SeqNumberType,
        point_id: PointIdType,
    ) -> OperationResult<bool> {
        self.move_if_exists(op_num, point_id)?;
        self.write_segment
            .get()
            .write()
            .clear_payload(op_num, point_id)
    }

    fn vector(
        &self,
        vector_name: &str,
        point_id: PointIdType,
    ) -> OperationResult<Option<Vec<VectorElementType>>> {
        return if self.deleted_points.read().contains(&point_id) {
            self.write_segment
                .get()
                .read()
                .vector(vector_name, point_id)
        } else {
            {
                let write_segment = self.write_segment.get();
                let segment_guard = write_segment.read();
                if segment_guard.has_point(point_id) {
                    return segment_guard.vector(vector_name, point_id);
                }
            }
            self.wrapped_segment
                .get()
                .read()
                .vector(vector_name, point_id)
        };
    }

    fn all_vectors(&self, point_id: PointIdType) -> OperationResult<NamedVectors> {
        let mut result = NamedVectors::default();
        for vector_name in self
            .wrapped_segment
            .get()
            .read()
            .config()
            .vector_data
            .keys()
        {
            if let Some(vector) = self.vector(vector_name, point_id)? {
                result.insert(vector_name.clone(), vector);
            }
        }
        Ok(result)
    }

    fn payload(&self, point_id: PointIdType) -> OperationResult<Payload> {
        return if self.deleted_points.read().contains(&point_id) {
            self.write_segment.get().read().payload(point_id)
        } else {
            {
                let write_segment = self.write_segment.get();
                let segment_guard = write_segment.read();
                if segment_guard.has_point(point_id) {
                    return segment_guard.payload(point_id);
                }
            }
            self.wrapped_segment.get().read().payload(point_id)
        };
    }

    /// Not implemented for proxy
    fn iter_points(&self) -> Box<dyn Iterator<Item = PointIdType> + '_> {
        // iter_points is not available for Proxy implementation
        // Due to internal locks it is almost impossible to return iterator with proper owning, lifetimes, e.t.c.
        unimplemented!("call to iter_points is not implemented for Proxy segment")
    }

    fn read_filtered<'a>(
        &'a self,
        offset: Option<PointIdType>,
        limit: Option<usize>,
        filter: Option<&'a Filter>,
    ) -> Vec<PointIdType> {
        let deleted_points = self.deleted_points.read();
        let mut read_points = if deleted_points.is_empty() {
            self.wrapped_segment
                .get()
                .read()
                .read_filtered(offset, limit, filter)
        } else {
            let wrapped_filter =
                self.add_deleted_points_condition_to_filter(filter, &deleted_points);
            self.wrapped_segment
                .get()
                .read()
                .read_filtered(offset, limit, Some(&wrapped_filter))
        };
        let mut write_segment_points = self
            .write_segment
            .get()
            .read()
            .read_filtered(offset, limit, filter);
        read_points.append(&mut write_segment_points);
        read_points.sort_unstable();
        read_points
    }

    /// Read points in [from; to) range
    fn read_range(&self, from: Option<PointIdType>, to: Option<PointIdType>) -> Vec<PointIdType> {
        let deleted_points = self.deleted_points.read();
        let mut read_points = self.wrapped_segment.get().read().read_range(from, to);
        if !deleted_points.is_empty() {
            read_points.retain(|idx| !deleted_points.contains(idx))
        }
        let mut write_segment_points = self.write_segment.get().read().read_range(from, to);
        read_points.append(&mut write_segment_points);
        read_points.sort_unstable();
        read_points
    }

    fn has_point(&self, point_id: PointIdType) -> bool {
        return if self.deleted_points.read().contains(&point_id) {
            self.write_segment.get().read().has_point(point_id)
        } else {
            self.write_segment.get().read().has_point(point_id)
                || self.wrapped_segment.get().read().has_point(point_id)
        };
    }

    fn available_point_count(&self) -> usize {
        let deleted_points_count = self.deleted_points.read().len();
        let wrapped_segment_count = self.wrapped_segment.get().read().available_point_count();
        let write_segment_count = self.write_segment.get().read().available_point_count();
        (wrapped_segment_count + write_segment_count).saturating_sub(deleted_points_count)
    }

    fn deleted_point_count(&self) -> usize {
        self.write_segment.get().read().deleted_point_count()
    }

    fn estimate_point_count<'a>(&'a self, filter: Option<&'a Filter>) -> CardinalityEstimation {
        let deleted_point_count = self.deleted_points.read().len();

        let (wrapped_segment_est, total_wrapped_size) = {
            let wrapped_segment = self.wrapped_segment.get();
            let wrapped_segment_guard = wrapped_segment.read();
            (
                wrapped_segment_guard.estimate_point_count(filter),
                wrapped_segment_guard.available_point_count(),
            )
        };

        let write_segment_est = self.write_segment.get().read().estimate_point_count(filter);

        let expected_deleted_count = if total_wrapped_size > 0 {
            (wrapped_segment_est.exp as f64
                * (deleted_point_count as f64 / total_wrapped_size as f64)) as usize
        } else {
            0
        };

        let primary_clauses =
            if wrapped_segment_est.primary_clauses == write_segment_est.primary_clauses {
                wrapped_segment_est.primary_clauses
            } else {
                vec![]
            };

        CardinalityEstimation {
            primary_clauses,
            min: wrapped_segment_est.min.saturating_sub(deleted_point_count)
                + write_segment_est.min,
            exp: (wrapped_segment_est.exp + write_segment_est.exp)
                .saturating_sub(expected_deleted_count),
            max: wrapped_segment_est.max + write_segment_est.max,
        }
    }

    fn segment_type(&self) -> SegmentType {
        SegmentType::Special
    }

    fn info(&self) -> SegmentInfo {
        let wrapped_info = self.wrapped_segment.get().read().info();
        let write_info = self.write_segment.get().read().info();

        // This is a best estimate
        let num_vectors = {
            let vector_name_count = self.config().vector_data.len();
            let deleted_points_count = self.deleted_points.read().len();
            (wrapped_info.num_vectors + write_info.num_vectors)
                .saturating_sub(deleted_points_count * vector_name_count)
        };

        SegmentInfo {
            segment_type: SegmentType::Special,
            num_vectors,
            num_points: self.available_point_count(),
            num_deleted_vectors: write_info.num_deleted_vectors,
            ram_usage_bytes: wrapped_info.ram_usage_bytes + write_info.ram_usage_bytes,
            disk_usage_bytes: wrapped_info.disk_usage_bytes + write_info.disk_usage_bytes,
            is_appendable: false,
            index_schema: wrapped_info.index_schema,
        }
    }

    fn config(&self) -> SegmentConfig {
        self.wrapped_segment.get().read().config()
    }

    fn is_appendable(&self) -> bool {
        true
    }

    fn flush(&self, sync: bool) -> OperationResult<SeqNumberType> {
        let deleted_points_guard = self.deleted_points.read();
        let deleted_indexes_guard = self.deleted_indexes.read();
        let created_indexes_guard = self.created_indexes.read();

        if deleted_points_guard.is_empty()
            && deleted_indexes_guard.is_empty()
            && created_indexes_guard.is_empty()
        {
            // Proxy changes are empty, therefore it is safe to flush write segment
            // This workaround only makes sense in a context of batch update of new points:
            //  - initial upload
            //  - incremental updates
            let wrapped_version = self.wrapped_segment.get().read().flush(sync)?;
            let write_segment_version = self.write_segment.get().read().flush(sync)?;
            let flushed_version = max(wrapped_version, write_segment_version);
            *self.last_flushed_version.write() = Some(flushed_version);
            Ok(flushed_version)
        } else {
            // If intermediate state is not empty - that is possible that some changes are not persisted
            Ok(self
                .last_flushed_version
                .read()
                .unwrap_or_else(|| self.wrapped_segment.get().read().version()))
        }
    }

    fn drop_data(self) -> OperationResult<()> {
        self.wrapped_segment.drop_data()
    }

    fn data_path(&self) -> PathBuf {
        self.wrapped_segment.get().read().data_path()
    }

    fn delete_field_index(&mut self, op_num: u64, key: PayloadKeyTypeRef) -> OperationResult<bool> {
        if self.version() > op_num {
            return Ok(false);
        }
        self.deleted_indexes.write().insert(key.into());
        self.created_indexes.write().remove(key);
        self.write_segment
            .get()
            .write()
            .delete_field_index(op_num, key)
    }

    fn create_field_index(
        &mut self,
        op_num: u64,
        key: PayloadKeyTypeRef,
        field_schema: Option<&PayloadFieldSchema>,
    ) -> OperationResult<bool> {
        if self.version() > op_num {
            return Ok(false);
        }

        self.write_segment
            .get()
            .write()
            .create_field_index(op_num, key, field_schema)?;
        let indexed_fields = self.write_segment.get().read().get_indexed_fields();

        let payload_schema = match indexed_fields.get(key) {
            Some(schema_type) => schema_type,
            None => return Ok(false),
        };

        self.created_indexes
            .write()
            .insert(key.into(), payload_schema.to_owned());
        self.deleted_indexes.write().remove(key);

        Ok(true)
    }

    fn get_indexed_fields(&self) -> HashMap<PayloadKeyType, PayloadFieldSchema> {
        let indexed_fields = self.wrapped_segment.get().read().get_indexed_fields();
        indexed_fields
            .into_iter()
            .chain(
                self.created_indexes
                    .read()
                    .iter()
                    .map(|(k, v)| (k.to_owned(), v.to_owned())),
            )
            .filter(|(key, _)| !self.deleted_indexes.read().contains(key))
            .collect()
    }

    fn check_error(&self) -> Option<SegmentFailedState> {
        self.write_segment.get().read().check_error()
    }

    fn delete_filtered<'a>(
        &'a mut self,
        op_num: SeqNumberType,
        filter: &'a Filter,
    ) -> OperationResult<usize> {
        let mut deleted_points = 0;

        let points_to_delete =
            self.wrapped_segment
                .get()
                .read()
                .read_filtered(None, None, Some(filter));
        if !points_to_delete.is_empty() {
            deleted_points += points_to_delete.len();
            let mut deleted_points_guard = self.deleted_points.write();
            deleted_points_guard.extend(points_to_delete);
        }

        deleted_points += self
            .write_segment
            .get()
            .write()
            .delete_filtered(op_num, filter)?;

        Ok(deleted_points)
    }

    fn vector_dim(&self, vector_name: &str) -> OperationResult<usize> {
        self.write_segment.get().read().vector_dim(vector_name)
    }

    fn vector_dims(&self) -> HashMap<String, usize> {
        self.write_segment.get().read().vector_dims()
    }

    fn take_snapshot(
        &self,
        temp_path: &Path,
        snapshot_dir_path: &Path,
    ) -> OperationResult<PathBuf> {
        log::info!(
            "Taking a snapshot of a proxy segment into {:?}",
            snapshot_dir_path
        );

        let archive_path = {
            let wrapped_segment_arc = self.wrapped_segment.get();
            let wrapped_segment_guard = wrapped_segment_arc.read();

            // snapshot wrapped segment data into the temporary dir
            wrapped_segment_guard.take_snapshot(temp_path, snapshot_dir_path)?
        };

        // snapshot write_segment
        let write_segment_rw = self.write_segment.get();
        let write_segment_guard = write_segment_rw.read();

        // Write segment is not unique to the proxy segment, therefore it might overwrite an existing snapshot.
        write_segment_guard.take_snapshot(temp_path, snapshot_dir_path)?;

        Ok(archive_path)
    }

    fn get_telemetry_data(&self) -> SegmentTelemetry {
        self.wrapped_segment.get().read().get_telemetry_data()
    }
}

#[cfg(test)]
mod tests {
    use std::fs::read_dir;

    use segment::data_types::vectors::{only_default_vector, DEFAULT_VECTOR_NAME};
    use segment::types::{FieldCondition, PayloadSchemaType};
    use serde_json::json;
    use tempfile::{Builder, TempDir};

    use super::*;
    use crate::collection_manager::fixtures::{
        build_segment_1, build_segment_2, empty_segment, random_segment,
    };

    #[test]
    fn test_writing() {
        let dir = Builder::new().prefix("segment_dir").tempdir().unwrap();
        let original_segment = LockedSegment::new(build_segment_1(dir.path()));
        let write_segment = LockedSegment::new(empty_segment(dir.path()));
        let deleted_points = Arc::new(RwLock::new(HashSet::<PointIdType>::new()));

        let deleted_indexes = Arc::new(RwLock::new(HashSet::<PayloadKeyType>::new()));
        let created_indexes = Arc::new(RwLock::new(
            HashMap::<PayloadKeyType, PayloadFieldSchema>::new(),
        ));

        let mut proxy_segment = ProxySegment::new(
            original_segment,
            write_segment,
            deleted_points,
            created_indexes,
            deleted_indexes,
        );

        let vec4 = vec![1.1, 1.0, 0.0, 1.0];
        proxy_segment
            .upsert_point(100, 4.into(), only_default_vector(&vec4))
            .unwrap();
        let vec6 = vec![1.0, 1.0, 0.5, 1.0];
        proxy_segment
            .upsert_point(101, 6.into(), only_default_vector(&vec6))
            .unwrap();
        proxy_segment.delete_point(102, 1.into()).unwrap();

        let query_vector = vec![1.0, 1.0, 1.0, 1.0];
        let search_result = proxy_segment
            .search(
                DEFAULT_VECTOR_NAME,
                &query_vector,
                &WithPayload::default(),
                &false.into(),
                None,
                10,
                None,
                &false.into(),
            )
            .unwrap();

        eprintln!("search_result = {search_result:#?}");

        let mut seen_points: HashSet<PointIdType> = Default::default();
        for res in search_result {
            if seen_points.contains(&res.id) {
                panic!("point {} appears multiple times", res.id);
            }
            seen_points.insert(res.id);
        }

        assert!(seen_points.contains(&4.into()));
        assert!(seen_points.contains(&6.into()));
        assert!(!seen_points.contains(&1.into()));

        assert!(!proxy_segment.write_segment.get().read().has_point(2.into()));

        let payload_key = "color".to_owned();
        proxy_segment
            .delete_payload(103, 2.into(), &payload_key)
            .unwrap();

        assert!(proxy_segment.write_segment.get().read().has_point(2.into()))
    }

    #[test]
    fn test_search_batch_equivalence_single() {
        let dir = Builder::new().prefix("segment_dir").tempdir().unwrap();
        let original_segment = LockedSegment::new(build_segment_1(dir.path()));
        let write_segment = LockedSegment::new(empty_segment(dir.path()));
        let deleted_points = Arc::new(RwLock::new(HashSet::<PointIdType>::new()));

        let deleted_indexes = Arc::new(RwLock::new(HashSet::<PayloadKeyType>::new()));
        let created_indexes = Arc::new(RwLock::new(
            HashMap::<PayloadKeyType, PayloadFieldSchema>::new(),
        ));

        let mut proxy_segment = ProxySegment::new(
            original_segment,
            write_segment,
            deleted_points,
            created_indexes,
            deleted_indexes,
        );

        let vec4 = vec![1.1, 1.0, 0.0, 1.0];
        proxy_segment
            .upsert_point(100, 4.into(), only_default_vector(&vec4))
            .unwrap();
        let vec6 = vec![1.0, 1.0, 0.5, 1.0];
        proxy_segment
            .upsert_point(101, 6.into(), only_default_vector(&vec6))
            .unwrap();
        proxy_segment.delete_point(102, 1.into()).unwrap();

        let query_vector = vec![1.0, 1.0, 1.0, 1.0];
        let search_result = proxy_segment
            .search(
                DEFAULT_VECTOR_NAME,
                &query_vector,
                &WithPayload::default(),
                &false.into(),
                None,
                10,
                None,
                &false.into(),
            )
            .unwrap();

        eprintln!("search_result = {search_result:#?}");

        let search_batch_result = proxy_segment
            .search_batch(
                DEFAULT_VECTOR_NAME,
                &[&query_vector],
                &WithPayload::default(),
                &false.into(),
                None,
                10,
                None,
                &false.into(),
            )
            .unwrap();

        eprintln!("search_batch_result = {search_batch_result:#?}");

        assert!(!search_result.is_empty());
        assert_eq!(search_result, search_batch_result[0].clone())
    }

    #[test]
    fn test_search_batch_equivalence_single_random() {
        let dir = Builder::new().prefix("segment_dir").tempdir().unwrap();
        let original_segment = LockedSegment::new(random_segment(dir.path(), 100, 200, 4));
        let write_segment = LockedSegment::new(empty_segment(dir.path()));
        let deleted_points = Arc::new(RwLock::new(HashSet::<PointIdType>::new()));

        let deleted_indexes = Arc::new(RwLock::new(HashSet::<PayloadKeyType>::new()));
        let created_indexes = Arc::new(RwLock::new(
            HashMap::<PayloadKeyType, PayloadFieldSchema>::new(),
        ));

        let proxy_segment = ProxySegment::new(
            original_segment,
            write_segment,
            deleted_points,
            created_indexes,
            deleted_indexes,
        );

        let query_vector = vec![1.0, 1.0, 1.0, 1.0];
        let search_result = proxy_segment
            .search(
                DEFAULT_VECTOR_NAME,
                &query_vector,
                &WithPayload::default(),
                &false.into(),
                None,
                10,
                None,
                &false.into(),
            )
            .unwrap();

        eprintln!("search_result = {search_result:#?}");

        let search_batch_result = proxy_segment
            .search_batch(
                DEFAULT_VECTOR_NAME,
                &[&query_vector],
                &WithPayload::default(),
                &false.into(),
                None,
                10,
                None,
                &false.into(),
            )
            .unwrap();

        eprintln!("search_batch_result = {search_batch_result:#?}");

        assert!(!search_result.is_empty());
        assert_eq!(search_result, search_batch_result[0].clone())
    }

    #[test]
    fn test_search_batch_equivalence_multi_random() {
        let dir = Builder::new().prefix("segment_dir").tempdir().unwrap();
        let original_segment = LockedSegment::new(random_segment(dir.path(), 100, 200, 4));
        let write_segment = LockedSegment::new(empty_segment(dir.path()));
        let deleted_points = Arc::new(RwLock::new(HashSet::<PointIdType>::new()));

        let deleted_indexes = Arc::new(RwLock::new(HashSet::<PayloadKeyType>::new()));
        let created_indexes = Arc::new(RwLock::new(
            HashMap::<PayloadKeyType, PayloadFieldSchema>::new(),
        ));

        let proxy_segment = ProxySegment::new(
            original_segment,
            write_segment,
            deleted_points,
            created_indexes,
            deleted_indexes,
        );

        let q1 = vec![1.0, 1.0, 1.0, 0.1];
        let q2 = vec![1.0, 1.0, 0.1, 0.1];
        let q3 = vec![1.0, 0.1, 1.0, 0.1];
        let q4 = vec![0.1, 1.0, 1.0, 0.1];

        let query_vectors: &[&[VectorElementType]] =
            &[q1.as_slice(), q2.as_slice(), q3.as_slice(), q4.as_slice()];

        let mut all_single_results = Vec::with_capacity(query_vectors.len());
        for query_vector in query_vectors {
            let res = proxy_segment
                .search(
                    DEFAULT_VECTOR_NAME,
                    query_vector,
                    &WithPayload::default(),
                    &false.into(),
                    None,
                    10,
                    None,
                    &false.into(),
                )
                .unwrap();
            all_single_results.push(res);
        }

        eprintln!("search_result = {all_single_results:#?}");

        let search_batch_result = proxy_segment
            .search_batch(
                DEFAULT_VECTOR_NAME,
                query_vectors,
                &WithPayload::default(),
                &false.into(),
                None,
                10,
                None,
                &false.into(),
            )
            .unwrap();

        eprintln!("search_batch_result = {search_batch_result:#?}");

        assert_eq!(all_single_results, search_batch_result)
    }

    fn wrap_proxy(dir: &TempDir, original_segment: LockedSegment) -> ProxySegment {
        let write_segment = LockedSegment::new(empty_segment(dir.path()));
        let deleted_points = Arc::new(RwLock::new(HashSet::<PointIdType>::new()));

        let deleted_indexes = Arc::new(RwLock::new(HashSet::<PayloadKeyType>::new()));
        let created_indexes = Arc::new(RwLock::new(
            HashMap::<PayloadKeyType, PayloadFieldSchema>::new(),
        ));

        ProxySegment::new(
            original_segment,
            write_segment,
            deleted_points,
            created_indexes,
            deleted_indexes,
        )
    }

    #[test]
    fn test_read_filter() {
        let dir = Builder::new().prefix("segment_dir").tempdir().unwrap();
        let original_segment = LockedSegment::new(build_segment_1(dir.path()));

        let filter = Filter::new_must_not(Condition::Field(FieldCondition::new_match(
            "color".to_string(),
            "blue".to_string().into(),
        )));

        let original_points = original_segment
            .get()
            .read()
            .read_filtered(None, Some(100), None);

        let original_points_filtered =
            original_segment
                .get()
                .read()
                .read_filtered(None, Some(100), Some(&filter));

        let mut proxy_segment = wrap_proxy(&dir, original_segment);

        proxy_segment.delete_point(100, 2.into()).unwrap();

        let proxy_res = proxy_segment.read_filtered(None, Some(100), None);
        let proxy_res_filtered = proxy_segment.read_filtered(None, Some(100), Some(&filter));

        assert_eq!(original_points_filtered.len() - 1, proxy_res_filtered.len());
        assert_eq!(original_points.len() - 1, proxy_res.len());
    }

    #[test]
    fn test_read_range() {
        let dir = Builder::new().prefix("segment_dir").tempdir().unwrap();
        let original_segment = LockedSegment::new(build_segment_1(dir.path()));

        let original_points = original_segment
            .get()
            .read()
            .read_range(None, Some(10.into()));

        let mut proxy_segment = wrap_proxy(&dir, original_segment);

        proxy_segment.delete_point(100, 2.into()).unwrap();

        proxy_segment
            .set_payload(
                101,
                3.into(),
                &json!({ "color": vec!["red".to_owned()] }).into(),
            )
            .unwrap();
        let proxy_res = proxy_segment.read_range(None, Some(10.into()));

        assert_eq!(original_points.len() - 1, proxy_res.len());
    }

    #[test]
    fn test_sync_indexes() {
        let dir = Builder::new().prefix("segment_dir").tempdir().unwrap();
        let original_segment = LockedSegment::new(build_segment_1(dir.path()));
        let write_segment = LockedSegment::new(empty_segment(dir.path()));

        let deleted_points = Arc::new(RwLock::new(HashSet::<PointIdType>::new()));
        let deleted_indexes = Arc::new(RwLock::new(HashSet::<PayloadKeyType>::new()));
        let created_indexes = Arc::new(RwLock::new(
            HashMap::<PayloadKeyType, PayloadFieldSchema>::new(),
        ));

        original_segment
            .get()
            .write()
            .create_field_index(10, "color", Some(&PayloadSchemaType::Keyword.into()))
            .unwrap();

        let mut proxy_segment = ProxySegment::new(
            original_segment.clone(),
            write_segment.clone(),
            deleted_points,
            created_indexes,
            deleted_indexes,
        );

        proxy_segment.replicate_field_indexes(0).unwrap();

        assert!(write_segment
            .get()
            .read()
            .get_indexed_fields()
            .contains_key("color"));

        original_segment
            .get()
            .write()
            .create_field_index(11, "location", Some(&PayloadSchemaType::Geo.into()))
            .unwrap();

        original_segment
            .get()
            .write()
            .delete_field_index(12, "color")
            .unwrap();

        proxy_segment.replicate_field_indexes(0).unwrap();

        assert!(write_segment
            .get()
            .read()
            .get_indexed_fields()
            .contains_key("location"));
        assert!(!write_segment
            .get()
            .read()
            .get_indexed_fields()
            .contains_key("color"));
    }

    #[test]
    fn test_take_snapshot() {
        let dir = Builder::new().prefix("segment_dir").tempdir().unwrap();
        let original_segment = LockedSegment::new(build_segment_1(dir.path()));
        let original_segment_2 = LockedSegment::new(build_segment_2(dir.path()));
        let write_segment = LockedSegment::new(empty_segment(dir.path()));
        let deleted_points = Arc::new(RwLock::new(HashSet::<PointIdType>::new()));

        let deleted_indexes = Arc::new(RwLock::new(HashSet::<PayloadKeyType>::new()));
        let created_indexes = Arc::new(RwLock::new(
            HashMap::<PayloadKeyType, PayloadFieldSchema>::new(),
        ));

        let mut proxy_segment = ProxySegment::new(
            original_segment,
            write_segment.clone(),
            deleted_points.clone(),
            created_indexes.clone(),
            deleted_indexes.clone(),
        );

        let mut proxy_segment2 = ProxySegment::new(
            original_segment_2,
            write_segment,
            deleted_points,
            created_indexes,
            deleted_indexes,
        );

        let vec4 = vec![1.1, 1.0, 0.0, 1.0];
        proxy_segment
            .upsert_point(100, 4.into(), only_default_vector(&vec4))
            .unwrap();
        let vec6 = vec![1.0, 1.0, 0.5, 1.0];
        proxy_segment
            .upsert_point(101, 6.into(), only_default_vector(&vec6))
            .unwrap();
        proxy_segment.delete_point(102, 1.into()).unwrap();

        proxy_segment2
            .upsert_point(201, 11.into(), only_default_vector(&vec6))
            .unwrap();

        let snapshot_dir = Builder::new().prefix("snapshot_dir").tempdir().unwrap();
        eprintln!("Snapshot into {:?}", snapshot_dir.path());

        let temp_dir = Builder::new().prefix("temp_dir").tempdir().unwrap();
        let temp_dir2 = Builder::new().prefix("temp_dir").tempdir().unwrap();

        proxy_segment
            .take_snapshot(temp_dir.path(), snapshot_dir.path())
            .unwrap();
        proxy_segment2
            .take_snapshot(temp_dir2.path(), snapshot_dir.path())
            .unwrap();

        // validate that 3 archives were created:
        // wrapped_segment1, wrapped_segment2 & shared write_segment
        let archive_count = read_dir(&snapshot_dir).unwrap().count();
        assert_eq!(archive_count, 3);

        for archive in read_dir(&snapshot_dir).unwrap() {
            let archive_path = archive.unwrap().path();
            let archive_extension = archive_path.extension().unwrap();
            // correct file extension
            assert_eq!(archive_extension, "tar");
        }
    }

    #[test]
    fn test_point_vector_count() {
        let dir = Builder::new().prefix("segment_dir").tempdir().unwrap();
        let original_segment = LockedSegment::new(build_segment_1(dir.path()));
        let write_segment = LockedSegment::new(empty_segment(dir.path()));
        let deleted_points = Arc::new(RwLock::new(HashSet::<PointIdType>::new()));

        let deleted_indexes = Arc::new(RwLock::new(HashSet::<PayloadKeyType>::new()));
        let created_indexes = Arc::new(RwLock::new(
            HashMap::<PayloadKeyType, PayloadFieldSchema>::new(),
        ));

        let mut proxy_segment = ProxySegment::new(
            original_segment,
            write_segment,
            deleted_points,
            created_indexes,
            deleted_indexes,
        );

        // We have 5 points by default, assert counts
        let segment_info = proxy_segment.info();
        assert_eq!(segment_info.num_points, 5);
        assert_eq!(segment_info.num_vectors, 5);

        // Delete non-existent point, counts should remain the same
        proxy_segment.delete_point(101, 99999.into()).unwrap();
        let segment_info = proxy_segment.info();
        assert_eq!(segment_info.num_points, 5);
        assert_eq!(segment_info.num_vectors, 5);

        // Delete point 1, counts should derease by 1
        proxy_segment.delete_point(102, 4.into()).unwrap();
        let segment_info = proxy_segment.info();
        assert_eq!(segment_info.num_points, 4);
        assert_eq!(segment_info.num_vectors, 4);

        // Delete vector of point 2, vector count should now be zero
        proxy_segment
            .delete_vector(103, 2.into(), DEFAULT_VECTOR_NAME)
            .unwrap();
        let segment_info = proxy_segment.info();
        assert_eq!(segment_info.num_points, 4);
        assert_eq!(segment_info.num_vectors, 3);
    }

    #[test]
    fn test_point_vector_count_multivec() {
        use segment::segment_constructor::build_segment;
        use segment::types::{Distance, Indexes, VectorDataConfig, VectorStorageType};

        // Create proxyied multivec segment
        let dir = Builder::new().prefix("segment_dir").tempdir().unwrap();
        let dim = 1;
        let config = SegmentConfig {
            vector_data: HashMap::from([
                (
                    "a".into(),
                    VectorDataConfig {
                        size: dim,
                        distance: Distance::Dot,
                        storage_type: VectorStorageType::Memory,
                        index: Indexes::Plain {},
                        quantization_config: None,
                    },
                ),
                (
                    "b".into(),
                    VectorDataConfig {
                        size: dim,
                        distance: Distance::Dot,
                        storage_type: VectorStorageType::Memory,
                        index: Indexes::Plain {},
                        quantization_config: None,
                    },
                ),
            ]),
            payload_storage_type: Default::default(),
        };
        let mut original_segment = build_segment(dir.path(), &config, true).unwrap();
        let write_segment = build_segment(dir.path(), &config, true).unwrap();

        original_segment
            .upsert_point(
                100,
                4.into(),
                NamedVectors::from([("a".into(), vec![0.4]), ("b".into(), vec![0.5])]),
            )
            .unwrap();
        original_segment
            .upsert_point(
                101,
                6.into(),
                NamedVectors::from([("a".into(), vec![0.6]), ("b".into(), vec![0.7])]),
            )
            .unwrap();

        let original_segment = LockedSegment::new(original_segment);
        let write_segment = LockedSegment::new(write_segment);
        let deleted_points = Arc::new(RwLock::new(HashSet::<PointIdType>::new()));

        let deleted_indexes = Arc::new(RwLock::new(HashSet::<PayloadKeyType>::new()));
        let created_indexes = Arc::new(RwLock::new(
            HashMap::<PayloadKeyType, PayloadFieldSchema>::new(),
        ));

        let mut proxy_segment = ProxySegment::new(
            original_segment,
            write_segment,
            deleted_points,
            created_indexes,
            deleted_indexes,
        );

        // Assert counts from original segment
        let segment_info = proxy_segment.info();
        assert_eq!(segment_info.num_points, 2);
        assert_eq!(segment_info.num_vectors, 4);

        // Insert point ID 8 and 10 partially, assert counts
        proxy_segment
            .upsert_point(102, 8.into(), NamedVectors::from([("a".into(), vec![0.0])]))
            .unwrap();
        proxy_segment
            .upsert_point(
                103,
                10.into(),
                NamedVectors::from([("b".into(), vec![1.0])]),
            )
            .unwrap();
        let segment_info = proxy_segment.info();
        assert_eq!(segment_info.num_points, 4);
        assert_eq!(segment_info.num_vectors, 6);

        // Delete non-existent point, counts should remain the same
        proxy_segment.delete_point(104, 1.into()).unwrap();
        let segment_info = proxy_segment.info();
        assert_eq!(segment_info.num_points, 4);
        assert_eq!(segment_info.num_vectors, 6);

        // Delete point 4, counts should derease by 1
        proxy_segment.delete_point(105, 4.into()).unwrap();
        let segment_info = proxy_segment.info();
        assert_eq!(segment_info.num_points, 3);
        assert_eq!(segment_info.num_vectors, 4);

        // Delete vector 'a' of point 6, vector count should decrease by 1
        proxy_segment.delete_vector(106, 6.into(), "a").unwrap();
        let segment_info = proxy_segment.info();
        assert_eq!(segment_info.num_points, 3);
        assert_eq!(segment_info.num_vectors, 3);

        // Deleting it again shouldn't chain anything
        proxy_segment.delete_vector(107, 6.into(), "a").unwrap();
        let segment_info = proxy_segment.info();
        assert_eq!(segment_info.num_points, 3);
        assert_eq!(segment_info.num_vectors, 3);

        // Replace vector 'a' for point 8, counts should remain the same
        proxy_segment
            .upsert_point(108, 8.into(), NamedVectors::from([("a".into(), vec![0.0])]))
            .unwrap();
        let segment_info = proxy_segment.info();
        assert_eq!(segment_info.num_points, 3);
        assert_eq!(segment_info.num_vectors, 3);

        // Replace both vectors for point 8, adding a new vector
        proxy_segment
            .upsert_point(
                109,
                8.into(),
                NamedVectors::from([("a".into(), vec![0.0]), ("b".into(), vec![0.0])]),
            )
            .unwrap();
        let segment_info = proxy_segment.info();
        assert_eq!(segment_info.num_points, 3);
        assert_eq!(segment_info.num_vectors, 4);
    }
}
