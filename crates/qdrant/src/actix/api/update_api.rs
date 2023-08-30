use actix_web::rt::time::Instant;
use actix_web::{delete, post, put, web, Responder};
use actix_web_validator::{Json, Path, Query};
use collection::operations::payload_ops::{DeletePayload, SetPayload};
use collection::operations::point_ops::{PointInsertOperations, PointsSelector, WriteOrdering};
use collection::operations::vector_ops::{DeleteVectors, UpdateVectors};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use storage::content_manager::toc::TableOfContent;
use validator::Validate;

use super::CollectionPath;
use crate::actix::helpers::process_response;
use crate::common::points::{
    do_batch_update_points, do_clear_payload, do_create_index, do_delete_index, do_delete_payload,
    do_delete_points, do_delete_vectors, do_overwrite_payload, do_set_payload, do_update_vectors,
    do_upsert_points, CreateFieldIndex, UpdateOperations,
};

#[derive(Deserialize, Validate)]
struct FieldPath {
    #[serde(rename = "field_name")]
    #[validate(length(min = 1))]
    name: String,
}

#[derive(Deserialize, Serialize, JsonSchema, Validate)]
pub struct UpdateParam {
    pub wait: Option<bool>,
    pub ordering: Option<WriteOrdering>,
}

#[put("/collections/{name}/points")]
async fn upsert_points(
    toc: web::Data<TableOfContent>,
    collection: Path<CollectionPath>,
    operation: Json<PointInsertOperations>,
    params: Query<UpdateParam>,
) -> impl Responder {
    let timing = Instant::now();
    let operation = operation.into_inner();
    let wait = params.wait.unwrap_or(false);
    let ordering = params.ordering.unwrap_or_default();

    let response = do_upsert_points(
        toc.get_ref(),
        &collection.name,
        operation,
        None,
        wait,
        ordering,
    )
    .await;
    process_response(response, timing)
}

#[post("/collections/{name}/points/delete")]
async fn delete_points(
    toc: web::Data<TableOfContent>,
    collection: Path<CollectionPath>,
    operation: Json<PointsSelector>,
    params: Query<UpdateParam>,
) -> impl Responder {
    let timing = Instant::now();
    let operation = operation.into_inner();
    let wait = params.wait.unwrap_or(false);
    let ordering = params.ordering.unwrap_or_default();

    let response = do_delete_points(
        toc.get_ref(),
        &collection.name,
        operation,
        None,
        wait,
        ordering,
    )
    .await;
    process_response(response, timing)
}

#[put("/collections/{name}/points/vectors")]
async fn update_vectors(
    toc: web::Data<TableOfContent>,
    collection: Path<CollectionPath>,
    operation: Json<UpdateVectors>,
    params: Query<UpdateParam>,
) -> impl Responder {
    let timing = Instant::now();
    let operation = operation.into_inner();
    let wait = params.wait.unwrap_or(false);
    let ordering = params.ordering.unwrap_or_default();

    let response = do_update_vectors(
        toc.get_ref(),
        &collection.name,
        operation,
        None,
        wait,
        ordering,
    )
    .await;
    process_response(response, timing)
}

#[post("/collections/{name}/points/vectors/delete")]
async fn delete_vectors(
    toc: web::Data<TableOfContent>,
    collection: Path<CollectionPath>,
    operation: Json<DeleteVectors>,
    params: Query<UpdateParam>,
) -> impl Responder {
    let timing = Instant::now();
    let operation = operation.into_inner();
    let wait = params.wait.unwrap_or(false);
    let ordering = params.ordering.unwrap_or_default();

    let response = do_delete_vectors(
        toc.get_ref(),
        &collection.name,
        operation,
        None,
        wait,
        ordering,
    )
    .await;
    process_response(response, timing)
}

#[post("/collections/{name}/points/payload")]
async fn set_payload(
    toc: web::Data<TableOfContent>,
    collection: Path<CollectionPath>,
    operation: Json<SetPayload>,
    params: Query<UpdateParam>,
) -> impl Responder {
    let timing = Instant::now();
    let operation = operation.into_inner();
    let wait = params.wait.unwrap_or(false);
    let ordering = params.ordering.unwrap_or_default();

    let response = do_set_payload(
        toc.get_ref(),
        &collection.name,
        operation,
        None,
        wait,
        ordering,
    )
    .await;
    process_response(response, timing)
}

#[put("/collections/{name}/points/payload")]
async fn overwrite_payload(
    toc: web::Data<TableOfContent>,
    collection: Path<CollectionPath>,
    operation: Json<SetPayload>,
    params: Query<UpdateParam>,
) -> impl Responder {
    let timing = Instant::now();
    let operation = operation.into_inner();
    let wait = params.wait.unwrap_or(false);
    let ordering = params.ordering.unwrap_or_default();

    let response = do_overwrite_payload(
        toc.get_ref(),
        &collection.name,
        operation,
        None,
        wait,
        ordering,
    )
    .await;
    process_response(response, timing)
}

#[post("/collections/{name}/points/payload/delete")]
async fn delete_payload(
    toc: web::Data<TableOfContent>,
    collection: Path<CollectionPath>,
    operation: Json<DeletePayload>,
    params: Query<UpdateParam>,
) -> impl Responder {
    let timing = Instant::now();
    let operation = operation.into_inner();
    let wait = params.wait.unwrap_or(false);
    let ordering = params.ordering.unwrap_or_default();

    let response = do_delete_payload(
        toc.get_ref(),
        &collection.name,
        operation,
        None,
        wait,
        ordering,
    )
    .await;
    process_response(response, timing)
}

#[post("/collections/{name}/points/payload/clear")]
async fn clear_payload(
    toc: web::Data<TableOfContent>,
    collection: Path<CollectionPath>,
    operation: Json<PointsSelector>,
    params: Query<UpdateParam>,
) -> impl Responder {
    let timing = Instant::now();
    let operation = operation.into_inner();
    let wait = params.wait.unwrap_or(false);
    let ordering = params.ordering.unwrap_or_default();

    let response = do_clear_payload(
        toc.get_ref(),
        &collection.name,
        operation,
        None,
        wait,
        ordering,
    )
    .await;
    process_response(response, timing)
}

#[post("/collections/{name}/points/batch")]
async fn update_batch(
    toc: web::Data<TableOfContent>,
    collection: Path<CollectionPath>,
    operations: Json<UpdateOperations>,
    params: Query<UpdateParam>,
) -> impl Responder {
    let timing = Instant::now();
    let operations = operations.into_inner();
    let wait = params.wait.unwrap_or(false);
    let ordering = params.ordering.unwrap_or_default();

    let response = do_batch_update_points(
        &toc,
        &collection.name,
        operations.operations,
        None,
        wait,
        ordering,
    )
    .await;
    process_response(response, timing)
}
#[put("/collections/{name}/index")]
async fn create_field_index(
    toc: web::Data<TableOfContent>,
    collection: Path<CollectionPath>,
    operation: Json<CreateFieldIndex>,
    params: Query<UpdateParam>,
) -> impl Responder {
    let timing = Instant::now();
    let operation = operation.into_inner();
    let wait = params.wait.unwrap_or(false);
    let ordering = params.ordering.unwrap_or_default();

    let response = do_create_index(
        toc.get_ref(),
        &collection.name,
        operation,
        None,
        wait,
        ordering,
    )
    .await;
    process_response(response, timing)
}

#[delete("/collections/{name}/index/{field_name}")]
async fn delete_field_index(
    toc: web::Data<TableOfContent>,
    collection: Path<CollectionPath>,
    field: Path<FieldPath>,
    params: Query<UpdateParam>,
) -> impl Responder {
    let timing = Instant::now();
    let wait = params.wait.unwrap_or(false);
    let ordering = params.ordering.unwrap_or_default();

    let response = do_delete_index(
        toc.get_ref(),
        &collection.name,
        field.name.clone(),
        None,
        wait,
        ordering,
    )
    .await;
    process_response(response, timing)
}

// Configure services
pub fn config_update_api(cfg: &mut web::ServiceConfig) {
    cfg.service(upsert_points)
        .service(delete_points)
        .service(update_vectors)
        .service(delete_vectors)
        .service(set_payload)
        .service(overwrite_payload)
        .service(delete_payload)
        .service(clear_payload)
        .service(create_field_index)
        .service(delete_field_index)
        .service(update_batch);
}
