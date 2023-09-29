use std::net;
use std::time::Duration;
use std::io::BufReader;
use std::sync::{Arc, Mutex};
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use super::DEFAULT_PORT;
use std::{error, fmt, io, num};


  const DEFAULT_TIMEOUT: Duration = Duration::from_secs(15);



/// Simple HTTP transport that implements the necessary subset of HTTP for
  /// running a bitcoind RPC client.
  #[derive(Clone, Debug)]
  pub struct SimpleHttpTransport {
      addr: net::SocketAddr,
      path: String,
      timeout: Duration,
      /// The value of the `Authorization` HTTP header.
      basic_auth: Option<String>,
      //#[cfg(feature = "proxy")]
      //proxy_addr: net::SocketAddr,
      //#[cfg(feature = "proxy")]
      //proxy_auth: Option<(String, String)>,
      //sock: Arc<Mutex<Option<BufReader<TcpStream>>>>,
  }

  impl Default for SimpleHttpTransport {
      fn default() -> Self {
          SimpleHttpTransport {
              addr: net::SocketAddr::new(
                  net::IpAddr::V4(net::Ipv4Addr::new(127, 0, 0, 1)),
                  DEFAULT_PORT,
              ),
              path: "/".to_owned(),
              timeout: DEFAULT_TIMEOUT,
              basic_auth: None,
              //#[cfg(feature = "proxy")]     ■ code is inactive due to #[cfg] directives: feature = "proxy" is disabled
              //proxy_addr: net::SocketAddr::new(
              //    net::IpAddr::V4(net::Ipv4Addr::new(127, 0, 0, 1)),
              //    DEFAULT_PROXY_PORT,
              //),
              //#[cfg(feature = "proxy")]     ■ code is inactive due to #[cfg] directives: feature = "proxy" is disabled
              //proxy_auth: None,
              //sock: Arc::new(Mutex::new(None)),
          }
      }
  }

  impl SimpleHttpTransport {
      /// Constructs a new [`SimpleHttpTransport`] with default parameters.
      pub fn new() -> Self {
          SimpleHttpTransport::default()
      }

            /// Replaces the URL of the transport.
      pub fn set_url(&mut self, url: &str) -> Result<(), Error> {
          let url = check_url(url)?;
          self.addr = url.0;
          self.path = url.1;
          Ok(())
      }
  }


  /// Does some very basic manual URL parsing because the uri/url crates
  /// all have unicode-normalization as a dependency and that's broken.
  fn check_url(url: &str) -> Result<(SocketAddr, String), Error> {
      // The fallback port in case no port was provided.
      // This changes when the http or https scheme was provided.
      let mut fallback_port = DEFAULT_PORT;

      // We need to get the hostname and the port.
      // (1) Split scheme
      let after_scheme = {
          let mut split = url.splitn(2, "://");
          let s = split.next().unwrap();
          match split.next() {
              None => s, // no scheme present
              Some(after) => {
                  // Check if the scheme is http or https.
                  if s == "http" {
                      fallback_port = 80;
                  } else if s == "https" {
                      fallback_port = 443;
                  } else {
                      //return Err(Error::url(url, "scheme should be http or https"));
                      todo!()
                  }
                  after
              }
          }
      };
      // (2) split off path
      let (before_path, path) = {
          if let Some(slash) = after_scheme.find('/') {
              (&after_scheme[0..slash], &after_scheme[slash..])
          } else {
              (after_scheme, "/")
          }
      };
      // (3) split off auth part
      let after_auth = {
          let mut split = before_path.splitn(2, '@');
          let s = split.next().unwrap();
          split.next().unwrap_or(s)
      };

      // (4) Parse into socket address.
      // At this point we either have <host_name> or <host_name_>:<port>
      // `std::net::ToSocketAddrs` requires `&str` to have <host_name_>:<port> format.
      let mut addr = match after_auth.to_socket_addrs() {
          Ok(addr) => addr,
          Err(_) => {
              //// Invalid socket address. Try to add port.
              //format!("{}:{}", after_auth, fallback_port).to_socket_addrs()?
              todo!();
          }
      };

      match addr.next() {
          Some(a) => Ok((a, path.to_owned())),
          None => todo!(),
          //Err(Error::url(url, "invalid hostname: error extracting socket address")),
      }
  }






impl crate::lib::jsonrpc_core::client::Client {
    /// Creates a new JSON-RPC client using a bare-minimum HTTP transport.
    pub fn simple_http(
        url: &str,
        user: Option<String>,
        pass: Option<String>,
    ) -> Result<crate::lib::jsonrpc_core::client::Client, Error> {
        let mut builder = Builder::new().url(url)?;
        if let Some(user) = user {
            builder = builder.auth(user, pass);
        }
        Ok(crate::lib::jsonrpc_core::client::Client::with_transport(builder.build()))
    }

    ///// Creates a new JSON_RPC client using a HTTP-Socks5 proxy transport.
    //#[cfg(feature = "proxy")]
    //pub fn http_proxy(
    //    url: &str,
    //    user: Option<String>,
    //    pass: Option<String>,
    //    proxy_addr: &str,
    //    proxy_auth: Option<(&str, &str)>,
    //) -> Result<crate::Client, Error> {
    //    let mut builder = Builder::new().url(url)?;
    //    if let Some(user) = user {
    //        builder = builder.auth(user, pass);
    //    }
    //    builder = builder.proxy_addr(proxy_addr)?;
    //    if let Some((user, pass)) = proxy_auth {
    //        builder = builder.proxy_auth(user, pass);
    //    }
    //    let tp = builder.build();
    //    Ok(crate::Client::with_transport(tp))
    //}
}


  /// Builder for simple bitcoind [`SimpleHttpTransport`].
  #[derive(Clone, Debug)]
  pub struct Builder {
      tp: SimpleHttpTransport,
  }

  impl Builder {
      /// Constructs a new [`Builder`] with default configuration.
      pub fn new() -> Builder {
          Builder {
              tp: SimpleHttpTransport::new(),
          }
      }

            /// Sets the URL of the server to the transport.
      pub fn url(mut self, url: &str) -> Result<Self, Error> {
          self.tp.set_url(url)?;
          Ok(self)
      }

            /// Adds authentication information to the transport.
      pub fn auth<S: AsRef<str>>(mut self, user: S, pass: Option<S>) -> Self {
          let mut auth = user.as_ref().to_owned();
          auth.push(':');
          if let Some(ref pass) = pass {
              auth.push_str(pass.as_ref());
          }
          self.tp.basic_auth = Some(format!("Basic {}", &base64::encode(auth.as_bytes())));
          self
      }


  }

  /// Error that can happen when sending requests.
  #[derive(Debug)]
  pub enum Error {
      /// An invalid URL was passed.
      InvalidUrl {
          /// The URL passed.
          url: String,
          /// The reason the URL is invalid.
          reason: &'static str,
      },
      /// An error occurred on the socket layer.
      SocketError(io::Error),
      /// The HTTP response was too short to even fit a HTTP 1.1 header.
      HttpResponseTooShort {
          /// The total length of the response.
          actual: usize,
          /// Minimum length we can parse.
          needed: usize,
      },
      /// The HTTP response started with a HTTP/1.1 line which was not ASCII.
      HttpResponseNonAsciiHello(Vec<u8>),
      /// The HTTP response did not start with HTTP/1.1
      HttpResponseBadHello {
          /// Actual HTTP-whatever string.
          actual: String,
          /// The hello string of the HTTP version we support.
          expected: String,
      },
      /// Could not parse the status value as a number.
      HttpResponseBadStatus(String, num::ParseIntError),
      /// Could not parse the status value as a number.
      HttpResponseBadContentLength(String, num::ParseIntError),
      /// The indicated content-length header exceeded our maximum.
      HttpResponseContentLengthTooLarge {
          /// The length indicated in the content-length header.
          length: u64,
          /// Our hard maximum on number of bytes we'll try to read.
          max: u64,
      },
      /// Unexpected HTTP error code (non-200).
      HttpErrorCode(u16),
      /// Received EOF before getting as many bytes as were indicated by the content-length header.
      IncompleteResponse {
          /// The content-length header.
          content_length: u64,
          /// The number of bytes we actually read.
          n_read: u64,
      },
      /// JSON parsing error.
      Json(serde_json::Error),
  }

