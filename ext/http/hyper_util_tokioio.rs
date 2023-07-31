// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

// Copyright 2023 Sean McArthur <sean@seanmonstar.com>
// MIT licensed copy of unreleased hyper-util code from
// https://raw.githubusercontent.com/hyperium/hyper-util/master/src/rt/tokio_io.rs

#![allow(dead_code)]
//! Tokio IO integration for hyper
use hyper1 as hyper;
use std::pin::Pin;
use std::task::Context;
use std::task::Poll;

use pin_project::pin_project;

/// A wrapping implementing hyper IO traits for a type that
/// implements Tokio's IO traits.
#[pin_project]
#[derive(Debug)]
pub struct TokioIo<T> {
  #[pin]
  inner: T,
}

impl<T> TokioIo<T> {
  /// Wrap a type implementing Tokio's IO traits.
  pub fn new(inner: T) -> Self {
    Self { inner }
  }

  /// Borrow the inner type.
  pub fn inner(&self) -> &T {
    &self.inner
  }

  /// Consume this wrapper and get the inner type.
  pub fn into_inner(self) -> T {
    self.inner
  }
}

impl<T> hyper::rt::Read for TokioIo<T>
where
  T: tokio::io::AsyncRead,
{
  fn poll_read(
    self: Pin<&mut Self>,
    cx: &mut Context<'_>,
    mut buf: hyper::rt::ReadBufCursor<'_>,
  ) -> Poll<Result<(), std::io::Error>> {
    // SAFETY: Imported code from hyper-util
    let n = unsafe {
      let mut tbuf = tokio::io::ReadBuf::uninit(buf.as_mut());
      match tokio::io::AsyncRead::poll_read(self.project().inner, cx, &mut tbuf)
      {
        Poll::Ready(Ok(())) => tbuf.filled().len(),
        other => return other,
      }
    };

    // SAFETY: Imported code from hyper-util
    unsafe {
      buf.advance(n);
    }
    Poll::Ready(Ok(()))
  }
}

impl<T> hyper::rt::Write for TokioIo<T>
where
  T: tokio::io::AsyncWrite,
{
  fn poll_write(
    self: Pin<&mut Self>,
    cx: &mut Context<'_>,
    buf: &[u8],
  ) -> Poll<Result<usize, std::io::Error>> {
    tokio::io::AsyncWrite::poll_write(self.project().inner, cx, buf)
  }

  fn poll_flush(
    self: Pin<&mut Self>,
    cx: &mut Context<'_>,
  ) -> Poll<Result<(), std::io::Error>> {
    tokio::io::AsyncWrite::poll_flush(self.project().inner, cx)
  }

  fn poll_shutdown(
    self: Pin<&mut Self>,
    cx: &mut Context<'_>,
  ) -> Poll<Result<(), std::io::Error>> {
    tokio::io::AsyncWrite::poll_shutdown(self.project().inner, cx)
  }

  fn is_write_vectored(&self) -> bool {
    tokio::io::AsyncWrite::is_write_vectored(&self.inner)
  }

  fn poll_write_vectored(
    self: Pin<&mut Self>,
    cx: &mut Context<'_>,
    bufs: &[std::io::IoSlice<'_>],
  ) -> Poll<Result<usize, std::io::Error>> {
    tokio::io::AsyncWrite::poll_write_vectored(self.project().inner, cx, bufs)
  }
}

impl<T> tokio::io::AsyncRead for TokioIo<T>
where
  T: hyper::rt::Read,
{
  fn poll_read(
    self: Pin<&mut Self>,
    cx: &mut Context<'_>,
    tbuf: &mut tokio::io::ReadBuf<'_>,
  ) -> Poll<Result<(), std::io::Error>> {
    //let init = tbuf.initialized().len();
    let filled = tbuf.filled().len();
    // SAFETY: Imported code from hyper-util
    let sub_filled = unsafe {
      let mut buf = hyper::rt::ReadBuf::uninit(tbuf.unfilled_mut());

      match hyper::rt::Read::poll_read(self.project().inner, cx, buf.unfilled())
      {
        Poll::Ready(Ok(())) => buf.filled().len(),
        other => return other,
      }
    };

    let n_filled = filled + sub_filled;
    // At least sub_filled bytes had to have been initialized.
    let n_init = sub_filled;
    // SAFETY: Imported code from hyper-util
    unsafe {
      tbuf.assume_init(n_init);
      tbuf.set_filled(n_filled);
    }

    Poll::Ready(Ok(()))
  }
}

impl<T> tokio::io::AsyncWrite for TokioIo<T>
where
  T: hyper::rt::Write,
{
  fn poll_write(
    self: Pin<&mut Self>,
    cx: &mut Context<'_>,
    buf: &[u8],
  ) -> Poll<Result<usize, std::io::Error>> {
    hyper::rt::Write::poll_write(self.project().inner, cx, buf)
  }

  fn poll_flush(
    self: Pin<&mut Self>,
    cx: &mut Context<'_>,
  ) -> Poll<Result<(), std::io::Error>> {
    hyper::rt::Write::poll_flush(self.project().inner, cx)
  }

  fn poll_shutdown(
    self: Pin<&mut Self>,
    cx: &mut Context<'_>,
  ) -> Poll<Result<(), std::io::Error>> {
    hyper::rt::Write::poll_shutdown(self.project().inner, cx)
  }

  fn is_write_vectored(&self) -> bool {
    hyper::rt::Write::is_write_vectored(&self.inner)
  }

  fn poll_write_vectored(
    self: Pin<&mut Self>,
    cx: &mut Context<'_>,
    bufs: &[std::io::IoSlice<'_>],
  ) -> Poll<Result<usize, std::io::Error>> {
    hyper::rt::Write::poll_write_vectored(self.project().inner, cx, bufs)
  }
}

/// A wrapping implementing Tokio IO traits for a type that
/// implements Hyper's IO traits.
#[pin_project]
#[derive(Debug)]
pub struct TokioIoForHyper<T> {
  #[pin]
  inner: T,
}

impl<T> TokioIoForHyper<T> {
  /// Wrap a type implementing Tokio's IO traits.
  pub fn new(inner: T) -> Self {
    Self { inner }
  }

  /// Borrow the inner type.
  pub fn inner(&self) -> &T {
    &self.inner
  }

  /// Consume this wrapper and get the inner type.
  pub fn into_inner(self) -> T {
    self.inner
  }
}
