// Copyright 2018-2026 the Deno authors. MIT license.

//! A small mock ACME (RFC 8555) certificate authority used to test the
//! automatic TLS certificate provisioning in `Deno.serve`.
//!
//! It implements just enough of the protocol for a happy-path issuance:
//! directory, nonces, account registration, orders, `http-01` challenges and
//! certificate finalization. JWS signatures are NOT verified (this is a test
//! mock), but `http-01` challenges are actually validated by fetching
//! `http://localhost:{ACME_CHALLENGE_PORT}/.well-known/acme-challenge/{token}`
//! and comparing the key authorization against the account key thumbprint.
//!
//! Issued certificates are signed by the `tests/testdata/tls/RootCA` test CA,
//! so clients that trust that root can connect.

use std::collections::HashMap;
use std::net::SocketAddr;

use base64::Engine;
use http_body_util::BodyExt;
use hyper::Request;
use hyper::Response;
use hyper::StatusCode;
use once_cell::sync::Lazy;
use parking_lot::Mutex;
use sha2::Digest;

use super::ServerKind;
use super::ServerOptions;
use super::hyper_utils::HandlerOutput;
use super::run_server;
use super::string_body;
use super::testdata_path;

/// Port the deno-side `http-01` challenge server is expected to listen on.
/// Real ACME CAs validate on port 80; tests can't bind that, so the deno
/// process passes `challengePort: 4271` and the mock validates against it.
pub const ACME_CHALLENGE_PORT: u16 = 4271;

const BASE64_URL: base64::engine::GeneralPurpose =
  base64::engine::general_purpose::URL_SAFE_NO_PAD;

#[derive(Default)]
struct Account {
  thumbprint: String,
}

#[derive(Clone, Copy, PartialEq)]
enum AuthzStatus {
  Pending,
  Valid,
  Invalid,
}

struct Authz {
  domain: String,
  token: String,
  status: AuthzStatus,
  order_id: u64,
}

#[derive(Clone, Copy, PartialEq)]
enum OrderStatus {
  Pending,
  Ready,
  Valid,
  Invalid,
}

struct Order {
  account_id: u64,
  identifiers: Vec<String>,
  authz_ids: Vec<u64>,
  status: OrderStatus,
  cert_id: Option<u64>,
}

#[derive(Default)]
struct AcmeState {
  next_id: u64,
  port: u16,
  accounts: HashMap<u64, Account>,
  orders: HashMap<u64, Order>,
  authzs: HashMap<u64, Authz>,
  certs: HashMap<u64, String>,
}

impl AcmeState {
  fn id(&mut self) -> u64 {
    self.next_id += 1;
    self.next_id
  }

  fn url(&self, path: &str) -> String {
    format!("http://localhost:{}{}", self.port, path)
  }
}

static STATE: Lazy<Mutex<AcmeState>> =
  Lazy::new(|| Mutex::new(AcmeState::default()));

static NONCE_COUNTER: Lazy<Mutex<u64>> = Lazy::new(|| Mutex::new(0));

fn next_nonce() -> String {
  let mut counter = NONCE_COUNTER.lock();
  *counter += 1;
  format!("mock-nonce-{}", *counter)
}

pub async fn acme_server(port: u16) {
  STATE.lock().port = port;
  run_server(
    ServerOptions {
      addr: SocketAddr::from(([127, 0, 0, 1], port)),
      kind: ServerKind::Auto,
      error_msg: "ACME mock server error",
    },
    acme_handler,
  )
  .await
}

fn json_response(
  status: StatusCode,
  location: Option<String>,
  value: serde_json::Value,
) -> HandlerOutput {
  let mut builder = Response::builder()
    .status(status)
    .header("content-type", "application/json")
    .header("replay-nonce", next_nonce());
  if let Some(location) = location {
    builder = builder.header("location", location);
  }
  Ok(builder.body(string_body(&value.to_string()))?)
}

fn problem(status: StatusCode, type_: &str, detail: &str) -> HandlerOutput {
  let value = serde_json::json!({
    "type": format!("urn:ietf:params:acme:error:{type_}"),
    "detail": detail,
  });
  Ok(
    Response::builder()
      .status(status)
      .header("content-type", "application/problem+json")
      .header("replay-nonce", next_nonce())
      .body(string_body(&value.to_string()))?,
  )
}

/// The fields of a JOSE request body we care about. Signatures are not
/// verified.
struct JoseBody {
  protected: serde_json::Value,
  payload: serde_json::Value,
}

async fn read_jose_body(
  req: Request<hyper::body::Incoming>,
) -> Result<JoseBody, anyhow::Error> {
  let body = req.collect().await?.to_bytes();
  let jose: serde_json::Value = serde_json::from_slice(&body)?;
  let protected = serde_json::from_slice(
    &BASE64_URL.decode(jose["protected"].as_str().unwrap_or_default())?,
  )?;
  let payload_b64 = jose["payload"].as_str().unwrap_or_default();
  let payload = if payload_b64.is_empty() {
    serde_json::Value::Null
  } else {
    serde_json::from_slice(&BASE64_URL.decode(payload_b64)?)?
  };
  Ok(JoseBody { protected, payload })
}

/// RFC 7638 thumbprint of an EC P-256 JWK.
fn jwk_thumbprint(jwk: &serde_json::Value) -> String {
  let canonical = format!(
    r#"{{"crv":"{}","kty":"{}","x":"{}","y":"{}"}}"#,
    jwk["crv"].as_str().unwrap_or_default(),
    jwk["kty"].as_str().unwrap_or_default(),
    jwk["x"].as_str().unwrap_or_default(),
    jwk["y"].as_str().unwrap_or_default(),
  );
  BASE64_URL.encode(sha2::Sha256::digest(canonical.as_bytes()))
}

/// Extract the account id from a `kid` protected header, eg.
/// `http://localhost:4270/acme/acct/3` -> 3.
fn account_id_from_kid(protected: &serde_json::Value) -> Option<u64> {
  let kid = protected["kid"].as_str()?;
  kid.rsplit('/').next()?.parse().ok()
}

fn order_json(state: &AcmeState, id: u64, order: &Order) -> serde_json::Value {
  serde_json::json!({
    "status": match order.status {
      OrderStatus::Pending => "pending",
      OrderStatus::Ready => "ready",
      OrderStatus::Valid => "valid",
      OrderStatus::Invalid => "invalid",
    },
    "identifiers": order.identifiers.iter()
      .map(|d| serde_json::json!({ "type": "dns", "value": d }))
      .collect::<Vec<_>>(),
    "authorizations": order.authz_ids.iter()
      .map(|id| state.url(&format!("/acme/authz/{id}")))
      .collect::<Vec<_>>(),
    "finalize": state.url(&format!("/acme/order/{id}/finalize")),
    "certificate": order.cert_id
      .map(|id| state.url(&format!("/acme/cert/{id}"))),
  })
}

fn authz_json(state: &AcmeState, id: u64, authz: &Authz) -> serde_json::Value {
  serde_json::json!({
    "status": match authz.status {
      AuthzStatus::Pending => "pending",
      AuthzStatus::Valid => "valid",
      AuthzStatus::Invalid => "invalid",
    },
    "identifier": { "type": "dns", "value": authz.domain },
    "challenges": [{
      "type": "http-01",
      "url": state.url(&format!("/acme/chall/{id}")),
      "token": authz.token,
      "status": match authz.status {
        AuthzStatus::Pending => "pending",
        AuthzStatus::Valid => "valid",
        AuthzStatus::Invalid => "invalid",
      },
    }],
  })
}

/// Sign the CSR with the testdata root CA, returning a PEM chain of
/// `leaf + root`.
fn issue_certificate(
  csr_der: &[u8],
  domains: &[String],
) -> Result<String, anyhow::Error> {
  let tls_dir = testdata_path().join("tls");
  let ca_key_pem = std::fs::read_to_string(tls_dir.join("RootCA.key"))?;
  let ca_cert_pem = std::fs::read_to_string(tls_dir.join("RootCA.pem"))?;

  let ca_key = rcgen::KeyPair::from_pem(&ca_key_pem)?;
  let ca_params = rcgen::CertificateParams::from_ca_cert_pem(&ca_cert_pem)?;
  let ca_cert = ca_params.self_signed(&ca_key)?;

  let mut csr = rcgen::CertificateSigningRequestParams::from_der(
    &rustls::pki_types::CertificateSigningRequestDer::from(csr_der.to_vec()),
  )?;
  // Fixed validity so tests are deterministic; far enough out that renewal
  // logic doesn't kick in during a test run.
  csr.params.not_before = rcgen::date_time_ymd(2026, 1, 1);
  csr.params.not_after = rcgen::date_time_ymd(2028, 1, 1);
  // Like a real CA, issue for the order's identifiers rather than blindly
  // trusting the CSR's SAN extension.
  csr.params.subject_alt_names = domains
    .iter()
    .map(|d| {
      Ok::<_, anyhow::Error>(rcgen::SanType::DnsName(d.clone().try_into()?))
    })
    .collect::<Result<Vec<_>, _>>()?;

  let cert = csr.signed_by(&ca_cert, &ca_key)?;
  Ok(format!("{}{}", cert.pem(), ca_cert_pem))
}

/// Fetch the `http-01` challenge response from the client under test and
/// validate the key authorization.
async fn validate_http01_challenge(authz_id: u64) {
  let (token, thumbprint) = {
    let state = STATE.lock();
    let Some(authz) = state.authzs.get(&authz_id) else {
      return;
    };
    let Some(order) = state.orders.get(&authz.order_id) else {
      return;
    };
    let Some(account) = state.accounts.get(&order.account_id) else {
      return;
    };
    (authz.token.clone(), account.thumbprint.clone())
  };

  let url = format!(
    "http://localhost:{ACME_CHALLENGE_PORT}/.well-known/acme-challenge/{token}"
  );
  let expected = format!("{token}.{thumbprint}");
  let fetched = match reqwest::get(&url).await {
    Ok(res) if res.status().is_success() => res.text().await.ok(),
    _ => None,
  };
  let ok = fetched.as_deref() == Some(expected.as_str());

  let mut state = STATE.lock();
  let Some(authz) = state.authzs.get_mut(&authz_id) else {
    return;
  };
  let order_id = authz.order_id;
  authz.status = if ok {
    AuthzStatus::Valid
  } else {
    AuthzStatus::Invalid
  };
  let all_valid = {
    let order = state.orders.get(&order_id).unwrap();
    order
      .authz_ids
      .iter()
      .all(|id| state.authzs[id].status == AuthzStatus::Valid)
  };
  let order = state.orders.get_mut(&order_id).unwrap();
  if !ok {
    order.status = OrderStatus::Invalid;
  } else if all_valid && order.status == OrderStatus::Pending {
    order.status = OrderStatus::Ready;
  }
}

async fn acme_handler(req: Request<hyper::body::Incoming>) -> HandlerOutput {
  let path = req.uri().path().to_string();
  let segments = path
    .strip_prefix("/acme/")
    .map(|p| p.split('/').collect::<Vec<_>>())
    .unwrap_or_default();

  match segments.as_slice() {
    ["directory"] => {
      let state = STATE.lock();
      json_response(
        StatusCode::OK,
        None,
        serde_json::json!({
          "newNonce": state.url("/acme/new-nonce"),
          "newAccount": state.url("/acme/new-account"),
          "newOrder": state.url("/acme/new-order"),
        }),
      )
    }
    ["new-nonce"] => Ok(
      Response::builder()
        .status(StatusCode::OK)
        .header("replay-nonce", next_nonce())
        .body(string_body(""))?,
    ),
    ["new-account"] => {
      let jose = read_jose_body(req).await?;
      let thumbprint = jwk_thumbprint(&jose.protected["jwk"]);
      let mut state = STATE.lock();
      let id = state.id();
      state.accounts.insert(id, Account { thumbprint });
      let location = state.url(&format!("/acme/acct/{id}"));
      json_response(
        StatusCode::CREATED,
        Some(location),
        serde_json::json!({ "status": "valid" }),
      )
    }
    ["new-order"] => {
      let jose = read_jose_body(req).await?;
      let Some(account_id) = account_id_from_kid(&jose.protected) else {
        return problem(
          StatusCode::BAD_REQUEST,
          "accountDoesNotExist",
          "no kid",
        );
      };
      let identifiers = jose.payload["identifiers"]
        .as_array()
        .map(|ids| {
          ids
            .iter()
            .filter_map(|i| i["value"].as_str().map(str::to_string))
            .collect::<Vec<_>>()
        })
        .unwrap_or_default();
      if identifiers.is_empty() {
        return problem(StatusCode::BAD_REQUEST, "malformed", "no identifiers");
      }
      let mut state = STATE.lock();
      let order_id = state.id();
      let mut authz_ids = Vec::new();
      for domain in &identifiers {
        let authz_id = state.id();
        let token = format!("mock-token-{authz_id}");
        state.authzs.insert(
          authz_id,
          Authz {
            domain: domain.clone(),
            token,
            status: AuthzStatus::Pending,
            order_id,
          },
        );
        authz_ids.push(authz_id);
      }
      let order = Order {
        account_id,
        identifiers,
        authz_ids,
        status: OrderStatus::Pending,
        cert_id: None,
      };
      let body = order_json(&state, order_id, &order);
      let location = state.url(&format!("/acme/order/{order_id}"));
      state.orders.insert(order_id, order);
      json_response(StatusCode::CREATED, Some(location), body)
    }
    ["order", id] => {
      let id: u64 = id.parse()?;
      // consume the JOSE body (POST-as-GET)
      let _ = read_jose_body(req).await?;
      let state = STATE.lock();
      let Some(order) = state.orders.get(&id) else {
        return problem(StatusCode::NOT_FOUND, "malformed", "no such order");
      };
      json_response(StatusCode::OK, None, order_json(&state, id, order))
    }
    ["order", id, "finalize"] => {
      let id: u64 = id.parse()?;
      let jose = read_jose_body(req).await?;
      let Some(csr_b64) = jose.payload["csr"].as_str() else {
        return problem(StatusCode::BAD_REQUEST, "badCSR", "missing csr");
      };
      let csr_der = BASE64_URL.decode(csr_b64)?;
      let (domains, ready) = {
        let state = STATE.lock();
        let Some(order) = state.orders.get(&id) else {
          return problem(StatusCode::NOT_FOUND, "malformed", "no such order");
        };
        (
          order.identifiers.clone(),
          order.status == OrderStatus::Ready,
        )
      };
      if !ready {
        return problem(
          StatusCode::FORBIDDEN,
          "orderNotReady",
          "order is not ready for finalization",
        );
      }
      let pem_chain = issue_certificate(&csr_der, &domains)?;
      let mut state = STATE.lock();
      let cert_id = state.id();
      state.certs.insert(cert_id, pem_chain);
      let order = state.orders.get_mut(&id).unwrap();
      order.status = OrderStatus::Valid;
      order.cert_id = Some(cert_id);
      let order = state.orders.get(&id).unwrap();
      json_response(StatusCode::OK, None, order_json(&state, id, order))
    }
    ["authz", id] => {
      let id: u64 = id.parse()?;
      let _ = read_jose_body(req).await?;
      let state = STATE.lock();
      let Some(authz) = state.authzs.get(&id) else {
        return problem(StatusCode::NOT_FOUND, "malformed", "no such authz");
      };
      json_response(StatusCode::OK, None, authz_json(&state, id, authz))
    }
    ["chall", id] => {
      let id: u64 = id.parse()?;
      let _ = read_jose_body(req).await?;
      validate_http01_challenge(id).await;
      let state = STATE.lock();
      let Some(authz) = state.authzs.get(&id) else {
        return problem(
          StatusCode::NOT_FOUND,
          "malformed",
          "no such challenge",
        );
      };
      let challenges = authz_json(&state, id, authz);
      json_response(StatusCode::OK, None, challenges["challenges"][0].clone())
    }
    ["cert", id] => {
      let id: u64 = id.parse()?;
      let _ = read_jose_body(req).await?;
      let state = STATE.lock();
      let Some(pem) = state.certs.get(&id) else {
        return problem(StatusCode::NOT_FOUND, "malformed", "no such cert");
      };
      Ok(
        Response::builder()
          .status(StatusCode::OK)
          .header("content-type", "application/pem-certificate-chain")
          .header("replay-nonce", next_nonce())
          .body(string_body(pem))?,
      )
    }
    _ => problem(StatusCode::NOT_FOUND, "malformed", "unknown endpoint"),
  }
}
