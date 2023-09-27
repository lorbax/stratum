use serde::{Deserialize, Serialize};
use serde_json;
use std::error;

/// A library error
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    /// A transport error
    Transport(Box<dyn error::Error + Send + Sync>),
    /// Json error
    Json(serde_json::Error),
    /// Error response
    Rpc(RpcError),
    /// Response to a request did not have the expected nonce
    NonceMismatch,
    /// Response to a request had a jsonrpc field other than "2.0"
    VersionMismatch,
    /// Batches can't be empty
    EmptyBatch,
    /// Too many responses returned in batch
    WrongBatchResponseSize,
    /// Batch response contained a duplicate ID
    BatchDuplicateResponseId(serde_json::Value),
    /// Batch response contained an ID that didn't correspond to any request ID
    WrongBatchResponseId(serde_json::Value),
}

/// A JSONRPC error object
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RpcError {
    /// The integer identifier of the error
    pub code: i32,
    /// A string describing the error
    pub message: String,
    /// Additional data specific to the error
    pub data: Option<Box<serde_json::value::RawValue>>,
}
