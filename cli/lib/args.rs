// Copyright 2018-2025 the Deno authors. MIT license.

use std::io::BufReader;
use std::io::Cursor;
use std::path::PathBuf;

use base64::prelude::BASE64_STANDARD;
use base64::prelude::Engine;
use deno_npm::resolution::PackageIdNotFoundError;
use deno_npm::resolution::ValidSerializedNpmResolutionSnapshot;
use deno_npm_installer::process_state::NpmProcessState;
use deno_npm_installer::process_state::NpmProcessStateFromEnvVarSys;
use deno_npm_installer::process_state::NpmProcessStateKind;
use deno_runtime::UNSTABLE_ENV_VAR_NAMES;
use deno_runtime::colors;
use deno_runtime::deno_tls::deno_native_certs::load_native_certs;
use deno_runtime::deno_tls::rustls;
use deno_runtime::deno_tls::rustls::RootCertStore;
use deno_runtime::deno_tls::rustls_pemfile;
use deno_runtime::deno_tls::webpki_roots;
use deno_semver::npm::NpmPackageReqReference;
use serde::Deserialize;
use serde::Serialize;
use thiserror::Error;

pub fn npm_pkg_req_ref_to_binary_command(
  req_ref: &NpmPackageReqReference,
) -> &str {
  req_ref.sub_path().unwrap_or_else(|| &req_ref.req().name)
}

pub fn has_trace_permissions_enabled() -> bool {
  has_flag_env_var("DENO_TRACE_PERMISSIONS")
}

pub fn has_flag_env_var(name: &str) -> bool {
  match std::env::var_os(name) {
    Some(value) => value == "1",
    None => false,
  }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CaData {
  /// The string is a file path
  File(String),
  /// The string holds the actual certificate
  Bytes(Vec<u8>),
}

impl CaData {
  pub fn parse(input: String) -> Option<Self> {
    if let Some(x) = input.strip_prefix("base64:") {
      Some(CaData::Bytes(BASE64_STANDARD.decode(x).ok()?))
    } else {
      Some(CaData::File(input))
    }
  }
}

#[derive(Error, Debug, Clone, deno_error::JsError)]
#[class(generic)]
pub enum RootCertStoreLoadError {
  #[error(
    "Unknown certificate store \"{0}\" specified (allowed: \"system,mozilla\")"
  )]
  UnknownStore(String),
  #[error("Unable to add pem file to certificate store: {0}")]
  FailedAddPemFile(String),
  #[error("Failed opening CA file: {0}")]
  CaFileOpenError(String),
  #[error("Failed to load platform certificates: {0}")]
  FailedNativeCerts(String),
}

/// Create and populate a root cert store based on the passed options and
/// environment.
pub fn get_root_cert_store(
  maybe_root_path: Option<PathBuf>,
  maybe_ca_stores: Option<Vec<String>>,
  maybe_ca_data: Option<CaData>,
) -> Result<RootCertStore, RootCertStoreLoadError> {
  let mut root_cert_store = RootCertStore::empty();
  let ca_stores: Vec<String> = maybe_ca_stores
    .or_else(|| {
      let env_ca_store = std::env::var("DENO_TLS_CA_STORE").ok()?;
      Some(
        env_ca_store
          .split(',')
          .map(|s| s.trim().to_string())
          .filter(|s| !s.is_empty())
          .collect(),
      )
    })
    .unwrap_or_else(|| vec!["mozilla".to_string()]);

  for store in ca_stores.iter() {
    match store.as_str() {
      "mozilla" => {
        root_cert_store.extend(webpki_roots::TLS_SERVER_ROOTS.to_vec());
      }
      "system" => {
        let roots = load_native_certs().map_err(|err| {
          RootCertStoreLoadError::FailedNativeCerts(err.to_string())
        })?;
        for root in roots {
          if let Err(err) = root_cert_store
            .add(rustls::pki_types::CertificateDer::from(root.0.clone()))
          {
            log::error!(
              "{}",
              colors::yellow(&format!(
                "Unable to add system certificate to certificate store: {:?}",
                err
              ))
            );
            let hex_encoded_root = faster_hex::hex_string(&root.0);
            log::error!("{}", colors::gray(&hex_encoded_root));
          }
        }
      }
      _ => {
        return Err(RootCertStoreLoadError::UnknownStore(store.clone()));
      }
    }
  }

  let ca_data = maybe_ca_data
    .or_else(|| std::env::var("DENO_CERT").ok().and_then(CaData::parse));
  if let Some(ca_data) = ca_data {
    let result = match ca_data {
      CaData::File(ca_file) => {
        let ca_file = if let Some(root) = &maybe_root_path {
          root.join(&ca_file)
        } else {
          PathBuf::from(ca_file)
        };
        let certfile = std::fs::File::open(ca_file).map_err(|err| {
          RootCertStoreLoadError::CaFileOpenError(err.to_string())
        })?;
        let mut reader = BufReader::new(certfile);
        rustls_pemfile::certs(&mut reader).collect::<Result<Vec<_>, _>>()
      }
      CaData::Bytes(data) => {
        let mut reader = BufReader::new(Cursor::new(data));
        rustls_pemfile::certs(&mut reader).collect::<Result<Vec<_>, _>>()
      }
    };

    match result {
      Ok(certs) => {
        root_cert_store.add_parsable_certificates(certs);
      }
      Err(e) => {
        return Err(RootCertStoreLoadError::FailedAddPemFile(e.to_string()));
      }
    }
  }

  Ok(root_cert_store)
}

pub fn npm_process_state(
  sys: &impl NpmProcessStateFromEnvVarSys,
) -> Option<&'static NpmProcessState> {
  static NPM_PROCESS_STATE: std::sync::OnceLock<Option<NpmProcessState>> =
    std::sync::OnceLock::new();

  NPM_PROCESS_STATE
    .get_or_init(|| {
      use deno_runtime::deno_process::NPM_RESOLUTION_STATE_FD_ENV_VAR_NAME;
      let fd_or_path = std::env::var_os(NPM_RESOLUTION_STATE_FD_ENV_VAR_NAME)?;

      #[allow(clippy::undocumented_unsafe_blocks)]
      unsafe {
        std::env::remove_var(NPM_RESOLUTION_STATE_FD_ENV_VAR_NAME)
      };
      if fd_or_path.is_empty() {
        return None;
      }
      NpmProcessState::from_env_var(sys, fd_or_path)
        .inspect_err(|e| {
          log::error!("failed to resolve npm process state: {}", e);
        })
        .ok()
    })
    .as_ref()
}

pub fn resolve_npm_resolution_snapshot(
  sys: &impl NpmProcessStateFromEnvVarSys,
) -> Result<Option<ValidSerializedNpmResolutionSnapshot>, PackageIdNotFoundError>
{
  if let Some(NpmProcessStateKind::Snapshot(snapshot)) =
    npm_process_state(sys).map(|s| &s.kind)
  {
    // TODO(bartlomieju): remove this clone
    Ok(Some(snapshot.clone().into_valid()?))
  } else {
    Ok(None)
  }
}

#[derive(Clone, Default, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct UnstableConfig {
  // TODO(bartlomieju): remove in Deno 2.5
  pub legacy_flag_enabled: bool, // --unstable
  pub bare_node_builtins: bool,
  pub detect_cjs: bool,
  pub lazy_dynamic_imports: bool,
  pub raw_imports: bool,
  pub sloppy_imports: bool,
  pub npm_lazy_caching: bool,
  pub tsgo: bool,
  pub features: Vec<String>, // --unstabe-kv --unstable-cron
}

impl UnstableConfig {
  pub fn fill_with_env(&mut self) {
    fn maybe_set(value: &mut bool, var_name: &str) {
      if !*value && has_flag_env_var(var_name) {
        *value = true;
      }
    }

    maybe_set(
      &mut self.bare_node_builtins,
      UNSTABLE_ENV_VAR_NAMES.bare_node_builtins,
    );
    maybe_set(
      &mut self.lazy_dynamic_imports,
      UNSTABLE_ENV_VAR_NAMES.lazy_dynamic_imports,
    );
    maybe_set(
      &mut self.npm_lazy_caching,
      UNSTABLE_ENV_VAR_NAMES.npm_lazy_caching,
    );
    maybe_set(&mut self.tsgo, UNSTABLE_ENV_VAR_NAMES.tsgo);
    maybe_set(&mut self.raw_imports, UNSTABLE_ENV_VAR_NAMES.raw_imports);
    maybe_set(
      &mut self.sloppy_imports,
      UNSTABLE_ENV_VAR_NAMES.sloppy_imports,
    );
  }

  pub fn enable_node_compat(&mut self) {
    self.bare_node_builtins = true;
    self.sloppy_imports = true;
    self.detect_cjs = true;
    if !self.features.iter().any(|f| f == "node-globals") {
      self.features.push("node-globals".to_string());
    }
  }
}
