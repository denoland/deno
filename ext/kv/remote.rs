// Copyright 2018-2025 the Deno authors. MIT license.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use anyhow::Context;
use async_trait::async_trait;
use bytes::Bytes;
use deno_core::OpState;
use deno_core::futures::Stream;
use deno_error::JsErrorBox;
use deno_fetch::CreateHttpClientOptions;
use deno_fetch::create_http_client;
use deno_permissions::PermissionsContainer;
use deno_tls::Proxy;
use deno_tls::RootCertStoreProvider;
use deno_tls::TlsKeys;
use deno_tls::rustls::RootCertStore;
use denokv_remote::MetadataEndpoint;
use denokv_remote::Remote;
use denokv_remote::RemoteResponse;
use denokv_remote::RemoteTransport;
use http_body_util::BodyExt;
use url::Url;

use crate::DatabaseHandler;

#[derive(Clone)]
pub struct HttpOptions {
  pub user_agent: String,
  pub root_cert_store_provider: Option<Arc<dyn RootCertStoreProvider>>,
  pub proxy: Option<Proxy>,
  pub unsafely_ignore_certificate_errors: Option<Vec<String>>,
  pub client_cert_chain_and_key: TlsKeys,
}

impl HttpOptions {
  pub fn root_cert_store(&self) -> Result<Option<RootCertStore>, JsErrorBox> {
    Ok(match &self.root_cert_store_provider {
      Some(provider) => Some(provider.get_or_try_init()?.clone()),
      None => None,
    })
  }
}

pub struct RemoteDbHandler {
  http_options: HttpOptions,
}

impl RemoteDbHandler {
  pub fn new(http_options: HttpOptions) -> Self {
    Self { http_options }
  }
}

pub struct PermissionChecker {
  state: Rc<RefCell<OpState>>,
}

impl Clone for PermissionChecker {
  fn clone(&self) -> Self {
    Self {
      state: self.state.clone(),
    }
  }
}

impl denokv_remote::RemotePermissions for PermissionChecker {
  fn check_net_url(&self, url: &Url) -> Result<(), JsErrorBox> {
    let mut state = self.state.borrow_mut();
    let permissions = state.borrow_mut::<PermissionsContainer>();
    permissions
      .check_net_url(url, "Deno.openKv")
      .map_err(JsErrorBox::from_err)
  }
}

#[derive(Clone)]
pub struct FetchClient(deno_fetch::Client);
pub struct FetchResponse(http::Response<deno_fetch::ResBody>);

impl RemoteTransport for FetchClient {
  type Response = FetchResponse;
  async fn post(
    &self,
    url: Url,
    headers: http::HeaderMap,
    body: Bytes,
  ) -> Result<(Url, http::StatusCode, Self::Response), JsErrorBox> {
    let body = deno_fetch::ReqBody::full(body);
    let mut req = http::Request::new(body);
    *req.method_mut() = http::Method::POST;
    *req.uri_mut() =
      url.as_str().parse().map_err(|e: http::uri::InvalidUri| {
        JsErrorBox::type_error(e.to_string())
      })?;
    *req.headers_mut() = headers;

    let res = self
      .0
      .clone()
      .send(req)
      .await
      .map_err(JsErrorBox::from_err)?;
    let status = res.status();
    Ok((url, status, FetchResponse(res)))
  }
}

impl RemoteResponse for FetchResponse {
  async fn bytes(self) -> Result<Bytes, JsErrorBox> {
    Ok(self.0.collect().await?.to_bytes())
  }
  fn stream(
    self,
  ) -> impl Stream<Item = Result<Bytes, JsErrorBox>> + Send + Sync {
    self.0.into_body().into_data_stream()
  }
  async fn text(self) -> Result<String, JsErrorBox> {
    let bytes = self.bytes().await?;
    Ok(
      std::str::from_utf8(&bytes)
        .map_err(JsErrorBox::from_err)?
        .into(),
    )
  }
}

#[async_trait(?Send)]
impl DatabaseHandler for RemoteDbHandler {
  type DB = Remote<PermissionChecker, FetchClient>;

  async fn open(
    &self,
    state: Rc<RefCell<OpState>>,
    path: Option<String>,
  ) -> Result<Self::DB, JsErrorBox> {
    const ENV_VAR_NAME: &str = "DENO_KV_ACCESS_TOKEN";

    let Some(url) = path else {
      return Err(JsErrorBox::type_error("Missing database url"));
    };

    let Ok(parsed_url) = Url::parse(&url) else {
      return Err(JsErrorBox::type_error(format!(
        "Invalid database url: {}",
        url
      )));
    };

    {
      let mut state = state.borrow_mut();
      let permissions = state.borrow_mut::<PermissionsContainer>();
      permissions
        .check_env(ENV_VAR_NAME)
        .map_err(JsErrorBox::from_err)?;
      permissions
        .check_net_url(&parsed_url, "Deno.openKv")
        .map_err(JsErrorBox::from_err)?;
    }

    let access_token = std::env::var(ENV_VAR_NAME)
      .map_err(anyhow::Error::from)
      .with_context(|| {
        "Missing DENO_KV_ACCESS_TOKEN environment variable. Please set it to your access token from https://dash.deno.com/account."
      }).map_err(|e| JsErrorBox::generic(e.to_string()))?;

    let metadata_endpoint = MetadataEndpoint {
      url: parsed_url.clone(),
      access_token: access_token.clone(),
    };

    let options = &self.http_options;
    let client = create_http_client(
      &options.user_agent,
      CreateHttpClientOptions {
        root_cert_store: options.root_cert_store()?,
        ca_certs: vec![],
        proxy: options.proxy.clone(),
        dns_resolver: Default::default(),
        unsafely_ignore_certificate_errors: options
          .unsafely_ignore_certificate_errors
          .clone(),
        client_cert_chain_and_key: options
          .client_cert_chain_and_key
          .clone()
          .try_into()
          .unwrap(),
        pool_max_idle_per_host: None,
        pool_idle_timeout: None,
        http1: false,
        http2: true,
        local_address: None,
        client_builder_hook: None,
      },
    )
    .map_err(JsErrorBox::from_err)?;
    let fetch_client = FetchClient(client);

    let permissions = PermissionChecker {
      state: state.clone(),
    };

    let remote = Remote::new(fetch_client, permissions, metadata_endpoint);

    Ok(remote)
  }
}
