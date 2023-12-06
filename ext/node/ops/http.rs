// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use deno_core::error::type_error;
use deno_core::error::AnyError;
use deno_core::op2;
use deno_core::url::Url;
use deno_core::ByteString;
use deno_core::CancelFuture;
use deno_core::CancelHandle;
use deno_core::OpState;
use deno_core::ResourceId;
use deno_fetch::get_or_create_client_from_state;
use deno_fetch::FetchCancelHandle;
use deno_fetch::FetchRequestResource;
use deno_fetch::FetchReturn;
use deno_fetch::HttpClientResource;
use deno_fetch::ResourceToBodyAdapter;
use reqwest::header::HeaderMap;
use reqwest::header::HeaderName;
use reqwest::header::HeaderValue;
use reqwest::header::CONTENT_LENGTH;
use reqwest::Body;
use reqwest::Method;

#[op2]
#[serde]
pub fn op_node_http_request<P>(
  state: &mut OpState,
  #[serde] method: ByteString,
  #[string] url: String,
  #[serde] headers: Vec<(ByteString, ByteString)>,
  #[smi] client_rid: Option<u32>,
  #[smi] body: Option<ResourceId>,
) -> Result<FetchReturn, AnyError>
where
  P: crate::NodePermissions + 'static,
{
  let client = if let Some(rid) = client_rid {
    let r = state.resource_table.get::<HttpClientResource>(rid)?;
    r.client.clone()
  } else {
    get_or_create_client_from_state(state)?
  };

  let method = Method::from_bytes(&method)?;
  let url = Url::parse(&url)?;

  {
    let permissions = state.borrow_mut::<P>();
    permissions.check_net_url(&url, "ClientRequest")?;
  }

  let mut header_map = HeaderMap::new();
  for (key, value) in headers {
    let name = HeaderName::from_bytes(&key)
      .map_err(|err| type_error(err.to_string()))?;
    let v = HeaderValue::from_bytes(&value)
      .map_err(|err| type_error(err.to_string()))?;

    header_map.append(name, v);
  }

  let mut request = client.request(method.clone(), url).headers(header_map);

  if let Some(body) = body {
    request = request.body(Body::wrap_stream(ResourceToBodyAdapter::new(
      state.resource_table.take_any(body)?,
    )));
  } else {
    // POST and PUT requests should always have a 0 length content-length,
    // if there is no body. https://fetch.spec.whatwg.org/#http-network-or-cache-fetch
    if matches!(method, Method::POST | Method::PUT) {
      request = request.header(CONTENT_LENGTH, HeaderValue::from(0));
    }
  };

  let cancel_handle = CancelHandle::new_rc();
  let cancel_handle_ = cancel_handle.clone();

  let fut = async move {
    request
      .send()
      .or_cancel(cancel_handle_)
      .await
      .map(|res| res.map_err(|err| type_error(err.to_string())))
  };

  let request_rid = state
    .resource_table
    .add(FetchRequestResource(Box::pin(fut)));

  let cancel_handle_rid =
    state.resource_table.add(FetchCancelHandle(cancel_handle));

  Ok(FetchReturn {
    request_rid,
    cancel_handle_rid: Some(cancel_handle_rid),
  })
}
