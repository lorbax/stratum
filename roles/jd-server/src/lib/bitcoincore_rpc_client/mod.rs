use crate::lib::jsonrpc_core::client::Client;

/// Client implements a JSON-RPC client for the Bitcoin Core daemon or compatible APIs.
pub struct RpcClient {
    client: Client,
}
