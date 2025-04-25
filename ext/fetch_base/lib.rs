use std::cell::RefCell;
use std::convert::From;
use std::path::PathBuf;
use std::rc::Rc;

use deno_core::futures::Future;
use deno_core::op2;
use deno_core::v8;
use deno_core::ByteString;
use deno_core::JsBuffer;
use deno_core::OpState;
use deno_core::ResourceId;
use deno_error::JsErrorClass;
use serde::Deserialize;
use serde::Serialize;

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
        scope: &mut v8::HandleScope,
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

#[op2(stack_trace)]
#[serde]
#[allow(clippy::too_many_arguments)]
pub fn op_fetch<FH>(
  scope: &mut v8::HandleScope,
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
    scope,
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

#[op2(async)]
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
