// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::cell::RefCell;
use std::marker::PhantomData;
use std::rc::Rc;
use std::sync::Arc;

use crate::DatabaseHandler;
use anyhow::Context;
use async_trait::async_trait;
use bytes::Bytes;
use deno_core::error::type_error;
use deno_core::error::AnyError;
use deno_core::futures::Stream;
use deno_core::OpState;
use deno_fetch::create_http_client;
use deno_fetch::CreateHttpClientOptions;
use deno_permissions::PermissionCheckError;
use deno_tls::rustls::RootCertStore;
use deno_tls::Proxy;
use deno_tls::RootCertStoreProvider;
use deno_tls::TlsKeys;
use denokv_remote::MetadataEndpoint;
use denokv_remote::Remote;
use denokv_remote::RemoteResponse;
use denokv_remote::RemoteTransport;
use http_body_util::BodyExt;
use url::Url;

#[derive(Clone)]
pub struct HttpOptions {
  pub user_agent: String,
  pub root_cert_store_provider: Option<Arc<dyn RootCertStoreProvider>>,
  pub proxy: Option<Proxy>,
  pub unsafely_ignore_certificate_errors: Option<Vec<String>>,
  pub client_cert_chain_and_key: TlsKeys,
}

impl HttpOptions {
  pub fn root_cert_store(&self) -> Result<Option<RootCertStore>, AnyError> {
    Ok(match &self.root_cert_store_provider {
      Some(provider) => Some(provider.get_or_try_init()?.clone()),
      None => None,
    })
  }
}

pub trait RemoteDbHandlerPermissions {
  fn check_env(&mut self, var: &str) -> Result<(), PermissionCheckError>;
  fn check_net_url(
    &mut self,
    url: &Url,
    api_name: &str,
  ) -> Result<(), PermissionCheckError>;
}

impl RemoteDbHandlerPermissions for deno_permissions::PermissionsContainer {
  #[inline(always)]
  fn check_env(&mut self, var: &str) -> Result<(), PermissionCheckError> {
    deno_permissions::PermissionsContainer::check_env(self, var)
  }

  #[inline(always)]
  fn check_net_url(
    &mut self,
    url: &Url,
    api_name: &str,
  ) -> Result<(), PermissionCheckError> {
    deno_permissions::PermissionsContainer::check_net_url(self, url, api_name)
  }
}

pub struct RemoteDbHandler<P: RemoteDbHandlerPermissions + 'static> {
  http_options: HttpOptions,
  _p: std::marker::PhantomData<P>,
}

impl<P: RemoteDbHandlerPermissions> RemoteDbHandler<P> {
  pub fn new(http_options: HttpOptions) -> Self {
    Self {
      http_options,
      _p: PhantomData,
    }
  }
}

pub struct PermissionChecker<P: RemoteDbHandlerPermissions> {
  state: Rc<RefCell<OpState>>,
  _permissions: PhantomData<P>,
}

impl<P: RemoteDbHandlerPermissions> Clone for PermissionChecker<P> {
  fn clone(&self) -> Self {
    Self {
      state: self.state.clone(),
      _permissions: PhantomData,
    }
  }
}

impl<P: RemoteDbHandlerPermissions + 'static> denokv_remote::RemotePermissions
  for PermissionChecker<P>
{
  fn check_net_url(&self, url: &Url) -> Result<(), anyhow::Error> {
    let mut state = self.state.borrow_mut();
    let permissions = state.borrow_mut::<P>();
    permissions
      .check_net_url(url, "Deno.openKv")
      .map_err(Into::into)
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
  ) -> Result<(Url, http::StatusCode, Self::Response), anyhow::Error> {
    let body = http_body_util::Full::new(body)
      .map_err(|never| match never {})
      .boxed();
    let mut req = http::Request::new(body);
    *req.method_mut() = http::Method::POST;
    *req.uri_mut() = url.as_str().parse()?;
    *req.headers_mut() = headers;

    let res = self.0.clone().send(req).await?;
    let status = res.status();
    Ok((url, status, FetchResponse(res)))
  }
}

impl RemoteResponse for FetchResponse {
  async fn bytes(self) -> Result<Bytes, anyhow::Error> {
    Ok(self.0.collect().await?.to_bytes())
  }
  fn stream(
    self,
  ) -> impl Stream<Item = Result<Bytes, anyhow::Error>> + Send + Sync {
    self.0.into_body().into_data_stream()
  }
  async fn text(self) -> Result<String, anyhow::Error> {
    let bytes = self.bytes().await?;
    Ok(std::str::from_utf8(&bytes)?.into())
  }
}

#[async_trait(?Send)]
impl<P: RemoteDbHandlerPermissions + 'static> DatabaseHandler
  for RemoteDbHandler<P>
{
  type DB = Remote<PermissionChecker<P>, FetchClient>;

  async fn open(
    &self,
    state: Rc<RefCell<OpState>>,
    path: Option<String>,
  ) -> Result<Self::DB, AnyError> {
    const ENV_VAR_NAME: &str = "DENO_KV_ACCESS_TOKEN";

    let Some(url) = path else {
      return Err(type_error("Missing database url"));
    };

    let Ok(parsed_url) = Url::parse(&url) else {
      return Err(type_error(format!("Invalid database url: {}", url)));
    };

    {
      let mut state = state.borrow_mut();
      let permissions = state.borrow_mut::<P>();
      permissions.check_env(ENV_VAR_NAME)?;
      permissions.check_net_url(&parsed_url, "Deno.openKv")?;
    }

    let access_token = std::env::var(ENV_VAR_NAME)
      .map_err(anyhow::Error::from)
      .with_context(|| {
        "Missing DENO_KV_ACCESS_TOKEN environment variable. Please set it to your access token from https://dash.deno.com/account."
      })?;

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
        client_builder_hook: None,
      },
    )?;
    let fetch_client = FetchClient(client);

    let permissions = PermissionChecker {
      state: state.clone(),
      _permissions: PhantomData,
    };

    let remote = Remote::new(fetch_client, permissions, metadata_endpoint);

    Ok(remote)
  }
}
