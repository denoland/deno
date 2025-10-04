// Copyright 2018-2025 the Deno authors. MIT license.

// Partially extracted / adapted from https://github.com/microsoft/libsyncrpc
// Copyright 2024 Microsoft Corporation. MIT license.

use std::io::BufRead;
use std::io::Result;
use std::io::Write;
use std::io::{self};

/// Lower-level wrapper around RPC-related messaging and process management.
pub struct RpcConnection<R: BufRead, W: Write> {
  reader: R,
  writer: W,
}

impl<R: BufRead, W: Write> RpcConnection<R, W> {
  pub fn new(reader: R, writer: W) -> Result<Self> {
    Ok(Self { reader, writer })
  }

  pub fn write(&mut self, ty: u8, name: &[u8], payload: &[u8]) -> Result<()> {
    let w = &mut self.writer;
    rmp::encode::write_array_len(w, 3)?;
    rmp::encode::write_u8(w, ty)?;
    rmp::encode::write_bin(w, name)?;
    rmp::encode::write_bin(w, payload)?;
    w.flush()?;
    Ok(())
  }

  pub fn read(&mut self) -> Result<(u8, Vec<u8>, Vec<u8>)> {
    let r = &mut self.reader;
    assert_eq!(
      rmp::decode::read_array_len(r).map_err(to_io)?,
      3,
      "Message components must be a valid 3-part messagepack array."
    );
    Ok((
      rmp::decode::read_int(r).map_err(to_io)?,
      self.read_bin()?,
      self.read_bin()?,
    ))
  }

  fn read_bin(&mut self) -> Result<Vec<u8>> {
    let r = &mut self.reader;
    let payload_len = rmp::decode::read_bin_len(r).map_err(to_io)?;
    let mut payload = vec![0u8; payload_len as usize];
    r.read_exact(&mut payload)?;
    Ok(payload)
  }

  // Helper method to create an error
  pub fn create_error(
    &self,
    name: &str,
    payload: Vec<u8>,
    expected_method: &str,
  ) -> io::Error {
    if name == expected_method {
      let payload = match String::from_utf8(payload) {
        Ok(payload) => payload,
        Err(err) => return io::Error::other(format!("{err}")),
      };
      io::Error::other(payload)
    } else {
      io::Error::other(format!(
        "name mismatch for response: expected `{expected_method}`, got `{name}`"
      ))
    }
  }
}

fn to_io<T: std::error::Error>(err: T) -> io::Error {
  io::Error::other(format!("{err}"))
}
