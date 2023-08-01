// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
use bytes::Bytes;
use deno_core::error::AnyError;
use deno_core::futures::Stream;
use deno_core::BufView;
use deno_core::ResourceBuilder;
use deno_core::ResourceBuilderImpl;
use hyper::body::SizeHint;
use hyper1::body::Body;
use hyper1::body::Incoming;
use std::pin::Pin;

/// Converts a hyper incoming body stream into a stream of [`Bytes`] that we can use to read in V8.
pub struct HyperIncomingStream(pub Incoming);

impl Stream for HyperIncomingStream {
  type Item = Result<BufView, AnyError>;

  fn poll_next(
    self: Pin<&mut Self>,
    cx: &mut std::task::Context<'_>,
  ) -> std::task::Poll<Option<Self::Item>> {
    let mut this = &mut self.get_mut().0;
    loop {
      let res = Pin::new(&mut this).poll_frame(cx);
      println!("res={res:?}");
      match res {
        std::task::Poll::Ready(Some(Ok(frame))) => {
          if let Ok(data) = frame.into_data() {
            // Ensure that we never yield an empty frame
            // TODO(mmastrac): We can use NoData in here eventually
            if !data.is_empty() {
              return std::task::Poll::Ready(Some(Ok(data.into())));
            }
          }
          // Loop around
          continue;
        }
        std::task::Poll::Ready(Some(Err(err))) => {
          return std::task::Poll::Ready(Some(Err(err.into())))
        }
        std::task::Poll::Ready(None) => return std::task::Poll::Ready(None),
        std::task::Poll::Pending => return std::task::Poll::Pending,
      }
    }
  }

  fn size_hint(&self) -> (usize, Option<usize>) {
    let hint = self.0.size_hint();
    (hint.lower() as _, hint.upper().map(|v| v as _))
  }
}

pub const HTTP_REQUEST_BODY_RESOURCE: ResourceBuilder<HyperIncomingStream> =
  ResourceBuilderImpl::new("serveRequestBody")
    .with_stream()
    .build();
