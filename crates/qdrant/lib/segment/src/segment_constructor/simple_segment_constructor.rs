use std::collections::HashMap;
use std::path::Path;

use crate::data_types::vectors::DEFAULT_VECTOR_NAME;
use crate::entry::entry_point::OperationResult;
use crate::segment::Segment;
use crate::segment_constructor::build_segment;
use crate::types::{Distance, Indexes, SegmentConfig, VectorDataConfig, VectorStorageType};

/// Build new segment with plain index in given directory
///
/// # Arguments
///
/// * `path` - path to collection\`s segment directory
///
pub fn build_simple_segment(
    path: &Path,
    dim: usize,
    distance: Distance,
) -> OperationResult<Segment> {
    build_segment(
        path,
        &SegmentConfig {
            vector_data: HashMap::from([(
                DEFAULT_VECTOR_NAME.to_owned(),
                VectorDataConfig {
                    size: dim,
                    distance,
                    storage_type: VectorStorageType::Memory,
                    index: Indexes::Plain {},
                    quantization_config: None,
                },
            )]),
            payload_storage_type: Default::default(),
        },
        true,
    )
}

pub fn build_multivec_segment(
    path: &Path,
    dim1: usize,
    dim2: usize,
    distance: Distance,
) -> OperationResult<Segment> {
    let mut vectors_config = HashMap::new();
    vectors_config.insert(
        "vector1".to_owned(),
        VectorDataConfig {
            size: dim1,
            distance,
            storage_type: VectorStorageType::Memory,
            index: Indexes::Plain {},
            quantization_config: None,
        },
    );
    vectors_config.insert(
        "vector2".to_owned(),
        VectorDataConfig {
            size: dim2,
            distance,
            storage_type: VectorStorageType::Memory,
            index: Indexes::Plain {},
            quantization_config: None,
        },
    );

    build_segment(
        path,
        &SegmentConfig {
            vector_data: vectors_config,
            payload_storage_type: Default::default(),
        },
        true,
    )
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use tempfile::Builder;

    use super::*;
    use crate::data_types::vectors::only_default_vector;
    use crate::entry::entry_point::{OperationError, SegmentEntry};

    #[test]
    fn test_create_simple_segment() {
        let dir = Builder::new().prefix("segment_dir").tempdir().unwrap();
        let segment = build_simple_segment(dir.path(), 100, Distance::Dot).unwrap();
        eprintln!(" = {:?}", segment.version());
    }

    #[test]
    fn test_add_and_search() {
        let dir = Builder::new().prefix("segment_dir").tempdir().unwrap();
        let mut segment = build_simple_segment(dir.path(), 4, Distance::Dot).unwrap();

        let wrong_vec = vec![1.0, 1.0, 1.0];

        let vec1 = vec![1.0, 0.0, 1.0, 1.0];
        let vec2 = vec![1.0, 0.0, 1.0, 0.0];
        let vec3 = vec![1.0, 1.0, 1.0, 1.0];
        let vec4 = vec![1.0, 1.0, 0.0, 1.0];
        let vec5 = vec![1.0, 0.0, 0.0, 0.0];

        match segment.upsert_point(1, 120.into(), only_default_vector(&wrong_vec)) {
            Err(OperationError::WrongVector { .. }) => (),
            Err(_) => panic!("Wrong error"),
            Ok(_) => panic!("Operation with wrong vector should fail"),
        };

        segment
            .upsert_point(2, 1.into(), only_default_vector(&vec1))
            .unwrap();
        segment
            .upsert_point(2, 2.into(), only_default_vector(&vec2))
            .unwrap();
        segment
            .upsert_point(2, 3.into(), only_default_vector(&vec3))
            .unwrap();
        segment
            .upsert_point(2, 4.into(), only_default_vector(&vec4))
            .unwrap();
        segment
            .upsert_point(2, 5.into(), only_default_vector(&vec5))
            .unwrap();

        segment
            .set_payload(
                3,
                1.into(),
                &json!({ "color": vec!["red".to_owned(), "green".to_owned()] }).into(),
            )
            .unwrap();

        segment
            .set_payload(
                3,
                2.into(),
                &json!({ "color": vec!["red".to_owned(), "blue".to_owned()] }).into(),
            )
            .unwrap();

        segment
            .set_payload(
                3,
                3.into(),
                &json!({ "color": vec!["red".to_owned(), "yellow".to_owned()] }).into(),
            )
            .unwrap();

        segment
            .set_payload(
                3,
                4.into(),
                &json!({ "color": vec!["red".to_owned(), "green".to_owned()] }).into(),
            )
            .unwrap();

        // Replace vectors
        segment
            .upsert_point(4, 1.into(), only_default_vector(&vec1))
            .unwrap();
        segment
            .upsert_point(5, 2.into(), only_default_vector(&vec2))
            .unwrap();
        segment
            .upsert_point(6, 3.into(), only_default_vector(&vec3))
            .unwrap();
        segment
            .upsert_point(7, 4.into(), only_default_vector(&vec4))
            .unwrap();
        segment
            .upsert_point(8, 5.into(), only_default_vector(&vec5))
            .unwrap();

        assert_eq!(segment.version(), 8);

        let declined = segment
            .upsert_point(3, 5.into(), only_default_vector(&vec5))
            .unwrap();

        // Should not be processed due to operation number
        assert!(!declined);
    }

    // ToDo: More tests
}
