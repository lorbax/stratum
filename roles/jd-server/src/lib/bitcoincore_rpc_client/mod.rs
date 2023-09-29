pub mod error;

use crate::lib::jsonrpc_core::client::Client as JsonRpcClient;
use std::path::PathBuf;
use std::{fmt, result};
use crate::lib::bitcoincore_rpc_client::error::Error;


  pub type Result<T> = result::Result<T, Error>;


  /// The different authentication methods for the client.
  #[derive(Clone, Debug, Hash, Eq, PartialEq, Ord, PartialOrd)]
  pub enum Auth {
      None,
      UserPass(String, String),
      //CookieFile(PathBuf),
  }

  impl Auth {
      /// Convert into the arguments that jsonrpc::Client needs.
      pub fn get_user_pass(self) -> Result<(Option<String>, Option<String>)> {
          use std::io::Read;
          match self {
              Auth::None => Ok((None, None)),
              Auth::UserPass(u, p) => Ok((Some(u), Some(p))),
              //Auth::CookieFile(path) => {
              //    let mut file = File::open(path)?;
              //    let mut contents = String::new();
              //    file.read_to_string(&mut contents)?;
              //    let mut split = contents.splitn(2, ":");
              //    Ok((
              //        Some(split.next().ok_or(Error::InvalidCookieFile)?.into()),
              //        Some(split.next().ok_or(Error::InvalidCookieFile)?.into()),
              //    ))
              //}
          }
      }
  }



/// Client implements a JSON-RPC client for the Bitcoin Core daemon or compatible APIs.
  pub struct BtcRpcClient {
      client: JsonRpcClient,
  }


  impl fmt::Debug for BtcRpcClient {
      fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
          write!(f, "bitcoincore_rpc::Client({:?})", self.client)
      }
  }


  impl BtcRpcClient {
      /// Creates a client to a bitcoind JSON-RPC server.
      ///
      /// Can only return [Err] when using cookie authentication.
      pub fn new(url: &str, auth: Auth) -> Result<Self> {
          let (user, pass) = auth.get_user_pass()?;
          JsonRpcClient::simple_http(url, user, pass)
              .map(|client| BtcRpcClient {
                  client,
              })
              .map_err(|e| crate::lib::bitcoincore_rpc_client::error::Error::JsonRpc(e.into()))
      }

      /// Create a new Client using the given [jsonrpc::Client].
      pub fn from_jsonrpc(client: JsonRpcClient) -> BtcRpcClient {
          BtcRpcClient {
              client,
          }
      }

      /// Get the underlying JSONRPC client.
      pub fn get_jsonrpc_client(&self) -> &JsonRpcClient {
          &self.client
      }
  }

