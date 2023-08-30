use actix_web::rt::time::Instant;
use actix_web::{get, post, web, Responder};
use actix_web_validator::{Json, Path, Query};
use collection::operations::consistency_params::ReadConsistency;
use collection::operations::types::{PointRequest, Record, ScrollRequest, ScrollResult};
use segment::types::{PointIdType, WithPayloadInterface};
use serde::Deserialize;
use storage::content_manager::errors::StorageError;
use storage::content_manager::toc::TableOfContent;
use validator::Validate;

use super::read_params::ReadParams;
use super::CollectionPath;
use crate::actix::helpers::process_response;
use crate::common::points::do_get_points;

#[derive(Deserialize, Validate)]
struct PointPath {
    #[validate(length(min = 1))]
    // TODO: validate this is a valid ID type (usize or UUID)? Does currently error on deserialize.
    id: String,
}

async fn do_get_point(
    toc: &TableOfContent,
    collection_name: &str,
    point_id: PointIdType,
    read_consistency: Option<ReadConsistency>,
) -> Result<Option<Record>, StorageError> {
    let request = PointRequest {
        ids: vec![point_id],
        with_payload: Some(WithPayloadInterface::Bool(true)),
        with_vector: true.into(),
    };

    toc.retrieve(collection_name, request, read_consistency, None)
        .await
        .map(|points| points.into_iter().next())
}

async fn scroll_get_points(
    toc: &TableOfContent,
    collection_name: &str,
    request: ScrollRequest,
    read_consistency: Option<ReadConsistency>,
) -> Result<ScrollResult, StorageError> {
    toc.scroll(collection_name, request, read_consistency, None)
        .await
}

#[get("/collections/{name}/points/{id}")]
async fn get_point(
    toc: web::Data<TableOfContent>,
    collection: Path<CollectionPath>,
    point: Path<PointPath>,
    params: Query<ReadParams>,
) -> impl Responder {
    let timing = Instant::now();

    let point_id: PointIdType = {
        let parse_res = point.id.parse();
        match parse_res {
            Ok(x) => x,
            Err(_) => {
                let error = Err(StorageError::BadInput {
                    description: format!("Can not recognize \"{}\" as point id", point.id),
                });
                return process_response::<()>(error, timing);
            }
        }
    };

    let response = do_get_point(
        toc.get_ref(),
        &collection.name,
        point_id,
        params.consistency,
    )
    .await;

    let response = match response {
        Ok(record) => match record {
            None => Err(StorageError::NotFound {
                description: format!("Point with id {point_id} does not exists!"),
            }),
            Some(record) => Ok(record),
        },
        Err(e) => Err(e),
    };
    process_response(response, timing)
}

#[post("/collections/{name}/points")]
async fn get_points(
    toc: web::Data<TableOfContent>,
    collection: Path<CollectionPath>,
    request: Json<PointRequest>,
    params: Query<ReadParams>,
) -> impl Responder {
    let timing = Instant::now();

    let response = do_get_points(
        toc.get_ref(),
        &collection.name,
        request.into_inner(),
        params.consistency,
        None,
    )
    .await;
    process_response(response, timing)
}

#[post("/collections/{name}/points/scroll")]
async fn scroll_points(
    toc: web::Data<TableOfContent>,
    collection: Path<CollectionPath>,
    request: Json<ScrollRequest>,
    params: Query<ReadParams>,
) -> impl Responder {
    let timing = Instant::now();

    let response = scroll_get_points(
        toc.get_ref(),
        &collection.name,
        request.into_inner(),
        params.consistency,
    )
    .await;
    process_response(response, timing)
}
