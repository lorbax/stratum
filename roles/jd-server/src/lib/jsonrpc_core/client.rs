use crate::lib::jsonrpc_core::{error::Error, Request, Response};
use std::{fmt, sync::atomic};

/// An interface for a transport over which to use the JSONRPC protocol.
pub trait Transport: Send + Sync + 'static {
    /// Sends an RPC request over the transport.
    fn send_request(&self, _: Request) -> Result<Response, Error>;
    /// Sends a batch of RPC requests over the transport.
    fn send_batch(&self, _: &[Request]) -> Result<Vec<Response>, Error>;
    /// Formats the target of this transport. I.e. the URL/socket/...
    fn fmt_target(&self, f: &mut fmt::Formatter) -> fmt::Result;
}

/// A JSON-RPC client.
///
/// Creates a new Client using one of the transport-specific constructors e.g.,
/// [`Client::simple_http`] for a bare-minimum HTTP transport.
///
pub struct Client {
    pub(crate) transport: Box<dyn Transport>,
    nonce: atomic::AtomicUsize,
}

  impl Client {
      /// Creates a new client with the given transport.
      pub fn with_transport<T: Transport>(transport: T) -> Client {
          Client {
              transport: Box::new(transport),
              nonce: atomic::AtomicUsize::new(1),
          }
      }
  }

  impl fmt::Debug for Client {
      fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
          write!(f, "jsonrpc::Client(")?;
          self.transport.fmt_target(f)?;
          write!(f, ")")
      }
  }

