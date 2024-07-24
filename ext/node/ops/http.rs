// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

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
use http::header::HeaderMap;
use http::header::HeaderName;
use http::header::HeaderValue;
use http::header::AUTHORIZATION;
use http::header::CONTENT_LENGTH;
use http::Method;
use http_body_util::BodyExt;

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
  let mut url = Url::parse(&url)?;
  let maybe_authority = deno_fetch::extract_authority(&mut url);

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

  let (body, con_len) = if let Some(body) = body {
    (
      ResourceToBodyAdapter::new(state.resource_table.take_any(body)?).boxed(),
      None,
    )
  } else {
    // POST and PUT requests should always have a 0 length content-length,
    // if there is no body. https://fetch.spec.whatwg.org/#http-network-or-cache-fetch
    let len = if matches!(method, Method::POST | Method::PUT) {
      Some(0)
    } else {
      None
    };
    (
      http_body_util::Empty::new()
        .map_err(|never| match never {})
        .boxed(),
      len,
    )
  };

  let mut request = http::Request::new(body);
  *request.method_mut() = method.clone();
  *request.uri_mut() = url
    .as_str()
    .parse()
    .map_err(|_| type_error("Invalid URL"))?;
  *request.headers_mut() = header_map;

  if let Some((username, password)) = maybe_authority {
    let value =
      deno_fetch::basic_auth(&username, password.as_ref().map(|x| x.as_str()));
    let mut header_value = HeaderValue::try_from(value)?;
    header_value.set_sensitive(true);
    request.headers_mut().insert(AUTHORIZATION, header_value);
  }
  if let Some(len) = con_len {
    request.headers_mut().insert(CONTENT_LENGTH, len.into());
  }

  let cancel_handle = CancelHandle::new_rc();
  let cancel_handle_ = cancel_handle.clone();

  let fut = async move {
    client
      .send(request)
      .or_cancel(cancel_handle_)
      .await
      .map(|res| res.map_err(|err| type_error(err.to_string())))
  };

  let request_rid = state.resource_table.add(FetchRequestResource {
    future: Box::pin(fut),
    url,
  });

  let cancel_handle_rid =
    state.resource_table.add(FetchCancelHandle(cancel_handle));

  Ok(FetchReturn {
    request_rid,
    cancel_handle_rid: Some(cancel_handle_rid),
  })
}
