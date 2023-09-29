use bitcoin::hashes::hex;
use crate::lib::jsonrpc_core::error::Error as JsonRpc_Error;
use bitcoin::secp256k1;
use std::io;
use bitcoin;

  /// The error type for errors produced in this library.
  #[derive(Debug)]
  pub enum Error {
      JsonRpc(JsonRpc_Error),
      Hex(hex::Error),
      Json(serde_json::error::Error),
      BitcoinSerialization(bitcoin::consensus::encode::Error),
      Secp256k1(secp256k1::Error),
      Io(io::Error),
      InvalidAmount(bitcoin::util::amount::ParseAmountError),
      InvalidCookieFile,
      /// The JSON result had an unexpected structure.
      UnexpectedStructure,
      /// The daemon returned an error string.
      ReturnedError(String),
  }

