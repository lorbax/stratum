use crate::lib::jsonrpc_core::error::RpcError;
use serde_json::value::RawValue;
use std::fmt;

pub mod client;
pub mod error;
pub mod http;

/// A JSONRPC request object.                                                                      
pub struct Request<'a> {
    /// The name of the RPC call.                                                                  
    pub method: &'a str,
    /// Parameters to the RPC call.                                                                
    pub params: &'a [Box<RawValue>],
    /// Identifier for this request, which should appear in the response.                          
    pub id: serde_json::Value,
    /// jsonrpc field, MUST be "2.0".                                                              
    pub jsonrpc: Option<&'a str>,
}
/// A JSONRPC response object.                                                                     
pub struct Response {
    /// A result if there is one, or [`None`].                                                     
    pub result: Option<Box<RawValue>>,
    /// An error if there is one, or [`None`].                                                     
    pub error: Option<RpcError>,
    /// Identifier for this response, which should match that of the request.                      
    pub id: serde_json::Value,
    /// jsonrpc field, MUST be "2.0".                                                              
    pub jsonrpc: Option<String>,
}
