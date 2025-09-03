// Copyright 2018-2025 the Deno authors. MIT license.

use std::marker::PhantomData;

use bytes::Bytes;
use bytes::BytesMut;
use httparse::Status;
use hyper::Response;
use hyper::header::HeaderName;
use hyper::header::HeaderValue;
use once_cell::sync::OnceCell;

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum RawUpgradeError {
  #[class("Http")]
  #[error("invalid headers")]
  InvalidHeaders,
  #[class(generic)]
  #[error("{0}")]
  HttpParse(#[from] httparse::Error),
  #[class(generic)]
  #[error("{0}")]
  Http(#[from] http::Error),
  #[class(generic)]
  #[error("{0}")]
  Utf8(#[from] std::str::Utf8Error),
  #[class(generic)]
  #[error("{0}")]
  InvalidHeaderName(#[from] http::header::InvalidHeaderName),
  #[class(generic)]
  #[error("{0}")]
  InvalidHeaderValue(#[from] http::header::InvalidHeaderValue),
  #[class("Http")]
  #[error("invalid HTTP status line")]
  InvalidHttpStatusLine,
  #[class("Http")]
  #[error("attempted to write to completed upgrade buffer")]
  UpgradeBufferAlreadyCompleted,
}

/// Given a buffer that ends in `\n\n` or `\r\n\r\n`, returns a parsed [`Request<Body>`].
fn parse_response<T: Default>(
  header_bytes: &[u8],
) -> Result<(usize, Response<T>), RawUpgradeError> {
  let mut headers = [httparse::EMPTY_HEADER; 16];
  let status = httparse::parse_headers(header_bytes, &mut headers)?;
  match status {
    Status::Complete((index, parsed)) => {
      let mut resp = Response::builder().status(101).body(T::default())?;
      for header in parsed.iter() {
        resp.headers_mut().append(
          HeaderName::from_bytes(header.name.as_bytes())?,
          HeaderValue::from_str(std::str::from_utf8(header.value)?)?,
        );
      }
      Ok((index, resp))
    }
    _ => Err(RawUpgradeError::InvalidHeaders),
  }
}

/// Find a newline in a slice.
fn find_newline(slice: &[u8]) -> Option<usize> {
  for (i, byte) in slice.iter().enumerate() {
    if *byte == b'\n' {
      return Some(i);
    }
  }
  None
}

/// Raw upgrade state machine states.
#[derive(Default)]
enum RawUpgradeState {
  #[default]
  Initial,
  StatusLine,
  Headers,
  Complete,
}

static HEADER_SEARCHER: OnceCell<memchr::memmem::Finder> = OnceCell::new();
static HEADER_SEARCHER2: OnceCell<memchr::memmem::Finder> = OnceCell::new();

#[derive(Default)]
pub struct RawUpgrade<T: Default> {
  state: RawUpgradeState,
  buf: BytesMut,
  _t: PhantomData<T>,
}

impl<T: Default> RawUpgrade<T> {
  /// Ensures that the status line starts with "HTTP/1.1 101 " which matches all of the node.js
  /// WebSocket libraries that are known. We don't care about the trailing status text.
  fn validate_status(&self, status: &[u8]) -> Result<(), RawUpgradeError> {
    if status.starts_with(b"HTTP/1.1 101 ") {
      Ok(())
    } else {
      Err(RawUpgradeError::InvalidHttpStatusLine)
    }
  }

  /// Writes bytes to our upgrade buffer, returning [`Ok(None)`] if we need to keep feeding it data,
  /// [`Ok(Some(Response))`] if we got a valid upgrade header, or [`Err`] if something went badly.
  pub fn write(
    &mut self,
    bytes: &[u8],
  ) -> Result<Option<(Response<T>, Bytes)>, RawUpgradeError> {
    use RawUpgradeState::*;

    match self.state {
      Initial => {
        if let Some(index) = find_newline(bytes) {
          let (status, rest) = bytes.split_at(index + 1);
          self.validate_status(status)?;

          // Fast path for the most common node.js WebSocket libraries that use \r\n as the
          // separator between header lines and send the whole response in one packet.
          if rest.ends_with(b"\r\n\r\n") {
            let (index, response) = parse_response(rest)?;
            if index == rest.len() {
              return Ok(Some((response, Bytes::default())));
            } else {
              let bytes = Bytes::copy_from_slice(&rest[index..]);
              return Ok(Some((response, bytes)));
            }
          }

          self.state = Headers;
          self.write(rest)
        } else {
          self.state = StatusLine;
          self.buf.extend_from_slice(bytes);
          Ok(None)
        }
      }
      StatusLine => {
        if let Some(index) = find_newline(bytes) {
          let (status, rest) = bytes.split_at(index + 1);
          self.buf.extend_from_slice(status);
          self.validate_status(&self.buf)?;
          self.buf.clear();
          // Recursively process this write
          self.state = Headers;
          self.write(rest)
        } else {
          self.buf.extend_from_slice(bytes);
          Ok(None)
        }
      }
      Headers => {
        self.buf.extend_from_slice(bytes);
        let header_searcher = HEADER_SEARCHER
          .get_or_init(|| memchr::memmem::Finder::new(b"\r\n\r\n"));
        let header_searcher2 =
          HEADER_SEARCHER2.get_or_init(|| memchr::memmem::Finder::new(b"\n\n"));
        if header_searcher.find(&self.buf).is_some()
          || header_searcher2.find(&self.buf).is_some()
        {
          let (index, response) = parse_response(&self.buf)?;
          let mut buf = std::mem::take(&mut self.buf);
          self.state = Complete;
          Ok(Some((response, buf.split_off(index).freeze())))
        } else {
          Ok(None)
        }
      }
      Complete => Err(RawUpgradeError::UpgradeBufferAlreadyCompleted),
    }
  }
}

#[cfg(test)]
mod tests {
  use hyper_v014::Body;

  use super::*;

  type ExpectedResponseAndHead = Option<(Response<Body>, &'static [u8])>;

  fn assert_response(
    result: Result<Option<(Response<Body>, Bytes)>, RawUpgradeError>,
    expected: Result<ExpectedResponseAndHead, RawUpgradeError>,
    chunk_info: Option<(usize, usize)>,
  ) {
    let formatted = format!("{result:?}");
    match expected {
      Ok(Some((resp1, remainder1))) => match result {
        Ok(Some((resp2, remainder2))) => {
          assert_eq!(format!("{resp1:?}"), format!("{resp2:?}"));
          if let Some((byte_len, chunk_size)) = chunk_info {
            // We need to compute how many bytes should be in the trailing data

            // We know how many bytes of header data we had
            let last_packet_header_size =
              (byte_len - remainder1.len() + chunk_size - 1) % chunk_size + 1;

            // Which means we can compute how much was in the remainder
            let remaining =
              (chunk_size - last_packet_header_size).min(remainder1.len());

            assert_eq!(remainder1[..remaining], remainder2);
          } else {
            assert_eq!(remainder1, remainder2);
          }
        }
        _ => panic!("Expected Ok(Some(...)), was {formatted}"),
      },
      Ok(None) => assert!(
        result.ok().unwrap().is_none(),
        "Expected Ok(None), was {formatted}",
      ),
      Err(e) => assert_eq!(
        format!("{e:?}"),
        format!("{:?}", result.unwrap_err()),
        "Expected error, was {formatted}",
      ),
    }
  }

  fn validate_upgrade_all_at_once(
    s: &str,
    expected: Result<ExpectedResponseAndHead, RawUpgradeError>,
  ) {
    let mut upgrade = RawUpgrade::default();
    let res = upgrade.write(s.as_bytes());

    assert_response(res, expected, None);
  }

  fn validate_upgrade_chunks(
    s: &str,
    size: usize,
    expected: Result<ExpectedResponseAndHead, RawUpgradeError>,
  ) {
    let chunk_info = Some((s.len(), size));
    let mut upgrade = RawUpgrade::default();
    let mut result = Ok(None);
    for chunk in s.as_bytes().chunks(size) {
      result = upgrade.write(chunk);
      if let Ok(Some(..)) = &result {
        assert_response(result, expected, chunk_info);
        return;
      }
    }
    assert_response(result, expected, chunk_info);
  }

  fn validate_upgrade(
    s: &str,
    expected: fn() -> Result<ExpectedResponseAndHead, RawUpgradeError>,
  ) {
    validate_upgrade_all_at_once(s, expected());
    validate_upgrade_chunks(s, 1, expected());
    validate_upgrade_chunks(s, 2, expected());
    validate_upgrade_chunks(s, 10, expected());

    // Replace \n with \r\n, but only in headers
    let (headers, trailing) = s.split_once("\n\n").unwrap();
    let s = headers.replace('\n', "\r\n") + "\r\n\r\n" + trailing;
    let s = s.as_ref();

    validate_upgrade_all_at_once(s, expected());
    validate_upgrade_chunks(s, 1, expected());
    validate_upgrade_chunks(s, 2, expected());
    validate_upgrade_chunks(s, 10, expected());
  }

  #[test]
  fn upgrade1() {
    validate_upgrade(
      "HTTP/1.1 101 Switching Protocols\nConnection: Upgrade\n\n",
      || {
        let mut expected =
          Response::builder().status(101).body(Body::empty()).unwrap();
        expected.headers_mut().append(
          HeaderName::from_static("connection"),
          HeaderValue::from_static("Upgrade"),
        );
        Ok(Some((expected, b"")))
      },
    );
  }

  #[test]
  fn upgrade_trailing() {
    validate_upgrade(
      "HTTP/1.1 101 Switching Protocols\nConnection: Upgrade\n\ntrailing data",
      || {
        let mut expected =
          Response::builder().status(101).body(Body::empty()).unwrap();
        expected.headers_mut().append(
          HeaderName::from_static("connection"),
          HeaderValue::from_static("Upgrade"),
        );
        Ok(Some((expected, b"trailing data")))
      },
    );
  }

  #[test]
  fn upgrade_trailing_with_newlines() {
    validate_upgrade(
      "HTTP/1.1 101 Switching Protocols\nConnection: Upgrade\n\ntrailing data\r\n\r\n",
      || {
        let mut expected =
          Response::builder().status(101).body(Body::empty()).unwrap();
        expected.headers_mut().append(
          HeaderName::from_static("connection"),
          HeaderValue::from_static("Upgrade"),
        );
        Ok(Some((expected, b"trailing data\r\n\r\n")))
      },
    );
  }

  #[test]
  fn upgrade2() {
    validate_upgrade(
      "HTTP/1.1 101 Switching Protocols\nConnection: Upgrade\nOther: 123\n\n",
      || {
        let mut expected =
          Response::builder().status(101).body(Body::empty()).unwrap();
        expected.headers_mut().append(
          HeaderName::from_static("connection"),
          HeaderValue::from_static("Upgrade"),
        );
        expected.headers_mut().append(
          HeaderName::from_static("other"),
          HeaderValue::from_static("123"),
        );
        Ok(Some((expected, b"")))
      },
    );
  }

  #[test]
  fn upgrade_invalid_status() {
    validate_upgrade("HTTP/1.1 200 OK\nConnection: Upgrade\n\n", || {
      Err(RawUpgradeError::InvalidHttpStatusLine)
    });
  }

  #[test]
  fn upgrade_too_many_headers() {
    let headers = (0..20)
      .map(|i| format!("h{i}: {i}"))
      .collect::<Vec<_>>()
      .join("\n");
    validate_upgrade(
      &format!("HTTP/1.1 101 Switching Protocols\n{headers}\n\n"),
      || Err(RawUpgradeError::HttpParse(httparse::Error::TooManyHeaders)),
    );
  }
}
