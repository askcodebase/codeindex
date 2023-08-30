use tonic::Status;

use crate::content_manager::collection_meta_ops::{
    AliasOperations, ChangeAliasesOperation, CollectionMetaOperations, CreateAlias,
    CreateAliasOperation, CreateCollection, CreateCollectionOperation, DeleteAlias,
    DeleteAliasOperation, DeleteCollectionOperation, InitFrom, RenameAlias, RenameAliasOperation,
    UpdateCollection, UpdateCollectionOperation,
};
use crate::content_manager::errors::StorageError;

pub fn error_to_status(error: StorageError) -> tonic::Status {
    let error_code = match &error {
        StorageError::BadInput { .. } => tonic::Code::InvalidArgument,
        StorageError::NotFound { .. } => tonic::Code::NotFound,
        StorageError::ServiceError { .. } => tonic::Code::Internal,
        StorageError::BadRequest { .. } => tonic::Code::InvalidArgument,
        StorageError::Locked { .. } => tonic::Code::FailedPrecondition,
        StorageError::Timeout { .. } => tonic::Code::DeadlineExceeded,
    };
    tonic::Status::new(error_code, format!("{error}"))
}

impl TryFrom<api::grpc::qdrant::CreateCollection> for CollectionMetaOperations {
    type Error = Status;

    fn try_from(value: api::grpc::qdrant::CreateCollection) -> Result<Self, Self::Error> {
        Ok(Self::CreateCollection(CreateCollectionOperation::new(
            value.collection_name,
            CreateCollection {
                vectors: match value.vectors_config.and_then(|config| config.config) {
                    Some(vector_config) => vector_config.try_into()?,
                    None => return Err(Status::invalid_argument("vectors config is required")),
                },
                hnsw_config: value.hnsw_config.map(|v| v.into()),
                wal_config: value.wal_config.map(|v| v.into()),
                optimizers_config: value.optimizers_config.map(|v| v.into()),
                shard_number: value.shard_number,
                on_disk_payload: value.on_disk_payload,
                replication_factor: value.replication_factor,
                write_consistency_factor: value.write_consistency_factor,
                init_from: value
                    .init_from_collection
                    .map(|v| InitFrom { collection: v }),
                quantization_config: value
                    .quantization_config
                    .map(TryInto::try_into)
                    .transpose()?,
            },
        )))
    }
}

impl TryFrom<api::grpc::qdrant::UpdateCollection> for CollectionMetaOperations {
    type Error = Status;

    fn try_from(value: api::grpc::qdrant::UpdateCollection) -> Result<Self, Self::Error> {
        Ok(Self::UpdateCollection(UpdateCollectionOperation::new(
            value.collection_name,
            UpdateCollection {
                vectors: value
                    .vectors_config
                    .and_then(|config| config.config)
                    .map(TryInto::try_into)
                    .transpose()?,
                hnsw_config: value.hnsw_config.map(Into::into),
                params: value.params.map(TryInto::try_into).transpose()?,
                optimizers_config: value.optimizers_config.map(Into::into),
                quantization_config: value
                    .quantization_config
                    .map(TryInto::try_into)
                    .transpose()?,
            },
        )))
    }
}

impl TryFrom<api::grpc::qdrant::DeleteCollection> for CollectionMetaOperations {
    type Error = Status;

    fn try_from(value: api::grpc::qdrant::DeleteCollection) -> Result<Self, Self::Error> {
        Ok(Self::DeleteCollection(DeleteCollectionOperation(
            value.collection_name,
        )))
    }
}

impl From<api::grpc::qdrant::CreateAlias> for AliasOperations {
    fn from(value: api::grpc::qdrant::CreateAlias) -> Self {
        Self::CreateAlias(CreateAliasOperation {
            create_alias: CreateAlias {
                collection_name: value.collection_name,
                alias_name: value.alias_name,
            },
        })
    }
}

impl From<api::grpc::qdrant::DeleteAlias> for AliasOperations {
    fn from(value: api::grpc::qdrant::DeleteAlias) -> Self {
        Self::DeleteAlias(DeleteAliasOperation {
            delete_alias: DeleteAlias {
                alias_name: value.alias_name,
            },
        })
    }
}

impl From<api::grpc::qdrant::RenameAlias> for AliasOperations {
    fn from(value: api::grpc::qdrant::RenameAlias) -> Self {
        Self::RenameAlias(RenameAliasOperation {
            rename_alias: RenameAlias {
                old_alias_name: value.old_alias_name,
                new_alias_name: value.new_alias_name,
            },
        })
    }
}

impl TryFrom<api::grpc::qdrant::AliasOperations> for AliasOperations {
    type Error = Status;

    fn try_from(value: api::grpc::qdrant::AliasOperations) -> Result<Self, Self::Error> {
        match value.action {
            Some(api::grpc::qdrant::alias_operations::Action::CreateAlias(create)) => {
                Ok(create.into())
            }
            Some(api::grpc::qdrant::alias_operations::Action::DeleteAlias(delete)) => {
                Ok(delete.into())
            }
            Some(api::grpc::qdrant::alias_operations::Action::RenameAlias(rename)) => {
                Ok(rename.into())
            }
            _ => Err(Status::invalid_argument("Malformed AliasOperation type")),
        }
    }
}

impl TryFrom<api::grpc::qdrant::ChangeAliases> for CollectionMetaOperations {
    type Error = Status;

    fn try_from(value: api::grpc::qdrant::ChangeAliases) -> Result<Self, Self::Error> {
        let actions: Vec<AliasOperations> = value
            .actions
            .into_iter()
            .map(|a| a.try_into())
            .collect::<Result<_, _>>()?;
        Ok(Self::ChangeAliases(ChangeAliasesOperation { actions }))
    }
}
