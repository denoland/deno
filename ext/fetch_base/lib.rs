use std::borrow::Cow;
use std::cell::RefCell;
use std::cmp::min;
use std::convert::From;
use std::path::PathBuf;
use std::pin::Pin;
use std::rc::Rc;

use bytes::Bytes;
use deno_core::futures::stream::Peekable;
use deno_core::futures::Future;
use deno_core::futures::Stream;
use deno_core::futures::StreamExt;
use deno_core::op2;
use deno_core::v8;
use deno_core::AsyncRefCell;
use deno_core::AsyncResult;
use deno_core::BufView;
use deno_core::ByteString;
use deno_core::CancelHandle;
use deno_core::CancelTryFuture;
use deno_core::JsBuffer;
use deno_core::OpState;
use deno_core::RcRef;
use deno_core::Resource;
use deno_core::ResourceId;
use deno_error::JsErrorBox;
use deno_error::JsErrorClass;
use serde::Deserialize;
use serde::Serialize;

pub type BytesStream =
  Pin<Box<dyn Stream<Item = Result<Bytes, std::io::Error>> + Unpin>>;

pub enum FetchResponseReader<T: Into<BytesStream>> {
  Start(T),
  BodyReader(Peekable<BytesStream>),
}

impl<T: Into<BytesStream>> Default for FetchResponseReader<T> {
  fn default() -> Self {
    let stream: BytesStream = Box::pin(deno_core::futures::stream::empty());
    Self::BodyReader(stream.peekable())
  }
}
#[derive(Debug)]
pub struct FetchResponseResource<T: Into<BytesStream>> {
  pub response_reader: AsyncRefCell<FetchResponseReader<T>>,
  pub cancel: CancelHandle,
  pub size: Option<u64>,
}

impl<T: Into<BytesStream> + 'static> FetchResponseResource<T> {
  pub fn new(s: T, size: Option<u64>) -> Self {
    Self {
      response_reader: AsyncRefCell::new(FetchResponseReader::Start(s)),
      cancel: CancelHandle::default(),
      size,
    }
  }
}

impl<T: Into<BytesStream> + 'static> Resource for FetchResponseResource<T> {
  fn name(&self) -> Cow<str> {
    "fetchResponse".into()
  }

  fn read(self: Rc<Self>, limit: usize) -> AsyncResult<BufView> {
    Box::pin(async move {
      let mut reader =
        RcRef::map(&self, |r| &r.response_reader).borrow_mut().await;

      let body = loop {
        match &mut *reader {
          FetchResponseReader::BodyReader(reader) => break reader,
          FetchResponseReader::Start(_) => {}
        }

        match std::mem::take(&mut *reader) {
          FetchResponseReader::Start(resp) => {
            *reader = FetchResponseReader::BodyReader(resp.into().peekable());
          }
          FetchResponseReader::BodyReader(_) => unreachable!(),
        }
      };
      let fut = async move {
        let mut reader = Pin::new(body);
        loop {
          match reader.as_mut().peek_mut().await {
            Some(Ok(chunk)) if !chunk.is_empty() => {
              let len = min(limit, chunk.len());
              let chunk = chunk.split_to(len);
              break Ok(chunk.into());
            }
            // This unwrap is safe because `peek_mut()` returned `Some`, and thus
            // currently has a peeked value that can be synchronously returned
            // from `next()`.
            //
            // The future returned from `next()` is always ready, so we can
            // safely call `await` on it without creating a race condition.
            Some(_) => match reader.as_mut().next().await.unwrap() {
              Ok(chunk) => assert!(chunk.is_empty()),
              Err(err) => break Err(JsErrorBox::type_error(err.to_string())),
            },
            None => break Ok(BufView::empty()),
          }
        }
      };

      let cancel_handle = RcRef::map(self, |r| &r.cancel);
      fut
        .try_or_cancel(cancel_handle)
        .await
        .map_err(JsErrorBox::from_err)
    })
  }

  fn size_hint(&self) -> (u64, Option<u64>) {
    (self.size.unwrap_or(0), self.size)
  }

  fn close(self: Rc<Self>) {
    self.cancel.cancel()
  }
}

/// Marker trait for options allow passing it in as a concrete type
pub trait Options {}

#[cfg(not(feature = "sandbox"))]
deno_core::extension!(deno_fetch,
  deps = [ deno_webidl, deno_web, deno_url, deno_console ],
  parameters = [FH: FetchHandler],
  ops = [
    op_fetch<FH>,
    op_fetch_send<FH>,
    op_utf8_to_byte_string,
    op_fetch_custom_client<FH>,
    op_fetch_promise_is_settled,
  ],
  esm = [
    "20_headers.js",
    "21_formdata.js",
    "22_body.js",
    "22_http_client.js",
    "23_request.js",
    "23_response.js",
    "26_fetch.js",
    "27_handle_wasm_streaming.js",
    "27_eventsource.js"
  ],
  options = {
    options: FH::Options,
  },
  state = |state, options| {
    state.put::<FH::Options>(options.options);
  },
);

#[cfg(feature = "sandbox")]
deno_core::extension!(deno_fetch,
  deps = [ deno_webidl, deno_web, deno_url, deno_console ],
  parameters = [FH: FetchHandler],
  ops = [
    op_fetch<FH>,
    op_fetch_send<FH>,
    op_utf8_to_byte_string,
    op_fetch_custom_client<FH>,
    op_fetch_promise_is_settled,
  ],
  esm = [
    "20_headers.js",
    "21_formdata.js",
    "22_body.js",
    "ext:deno_fetch/22_http_client.js" = "22_http_client_no_tls.js",
    "23_request.js",
    "23_response.js",
    "26_fetch.js"
  ],
  options = {
    options: FH::Options,
  },
  state = |state, options| {
    state.put::<FH::Options>(options.options);
  },
);

pub trait FetchHandler: 'static {
    type CreateHttpClientArgs: Deserialize<'static>;
    type FetchError: JsErrorClass + 'static;
    type Options;

    fn fetch(
        state: &mut deno_core::OpState,
        method: ByteString,
        url: String,
        headers: Vec<(ByteString, ByteString)>,
        client_rid: Option<u32>,
        has_body: bool,
        data: Option<JsBuffer>,
        resource: Option<ResourceId>,
    ) -> Result<FetchReturn, Self::FetchError>;

    fn fetch_send(
        state: Rc<RefCell<deno_core::OpState>>,
        rid: ResourceId,
    ) -> impl Future<Output = Result<FetchResponse, Self::FetchError>>;

    fn custom_client(
        state: &mut deno_core::OpState,
        args: Self::CreateHttpClientArgs,
        #[cfg(not(feature = "sandbox"))]
        tls_keys: &deno_tls::TlsKeysHolder,
    ) -> Result<ResourceId, Self::FetchError>;

}

pub fn get_declaration() -> PathBuf {
  PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("lib.deno_fetch.d.ts")
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FetchReturn {
  pub request_rid: ResourceId,
  pub cancel_handle_rid: Option<ResourceId>,
}

#[op2(reentrant, stack_trace)]
#[serde]
#[allow(clippy::too_many_arguments)]
pub fn op_fetch<FH>(
  state: &mut OpState,
  #[serde] method: ByteString,
  #[string] url: String,
  #[serde] headers: Vec<(ByteString, ByteString)>,
  #[smi] client_rid: Option<u32>,
  has_body: bool,
  #[buffer] data: Option<JsBuffer>,
  #[smi] resource: Option<ResourceId>,
) -> Result<FetchReturn, FH::FetchError>
where
  FH: FetchHandler + 'static
{
  FH::fetch(
    state,
    method,
    url,
    headers,
    client_rid,
    has_body,
    data,
    resource
  )
}


#[derive(Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FetchResponse {
  pub status: u16,
  pub status_text: String,
  pub headers: Vec<(ByteString, ByteString)>,
  pub url: String,
  pub response_rid: ResourceId,
  pub content_length: Option<u64>,
  pub remote_addr_ip: Option<String>,
  pub remote_addr_port: Option<u16>,
  /// This field is populated if some error occurred which needs to be
  /// reconstructed in the JS side to set the error _cause_.
  /// In the tuple, the first element is an error message and the second one is
  /// an error cause.
  pub error: Option<(String, String)>,
}

#[op2(async, reentrant)]
#[serde]
pub async fn op_fetch_send<FH>(
  state: Rc<RefCell<OpState>>,
  #[smi] rid: ResourceId,
) -> Result<FetchResponse, FH::FetchError> where FH: FetchHandler + 'static {
  FH::fetch_send(state, rid).await
}


#[cfg(not(feature = "sandbox"))]
#[op2(stack_trace)]
#[smi]
pub fn op_fetch_custom_client<FH>(
  state: &mut OpState,
  #[serde] args: FH::CreateHttpClientArgs,
  #[cppgc] tls_keys: &deno_tls::TlsKeysHolder,
) -> Result<ResourceId, FH::FetchError>
where
  FH: FetchHandler + 'static
{
    FH::custom_client(state, args, tls_keys)
}

#[cfg(feature = "sandbox")]
#[op2(stack_trace)]
#[smi]
pub fn op_fetch_custom_client<FH>(
  state: &mut OpState,
  #[serde] args: FH::CreateHttpClientArgs,
) -> Result<ResourceId, FH::FetchError>
where
  FH: FetchHandler + 'static
{
    FH::custom_client(state, args)
}

#[op2]
#[serde]
pub fn op_utf8_to_byte_string(#[string] input: String) -> ByteString {
  input.into()
}

#[op2(fast)]
fn op_fetch_promise_is_settled(promise: v8::Local<v8::Promise>) -> bool {
  promise.state() != v8::PromiseState::Pending
}
