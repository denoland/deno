// Copyright 2018-2025 the Deno authors. MIT license.

use std::ffi::OsStr;
use std::io::BufReader;
use std::io::Cursor;
use std::io::Read;
use std::io::Seek;
use std::path::PathBuf;
use std::sync::LazyLock;

use deno_npm::resolution::PackageIdNotFoundError;
use deno_npm::resolution::ValidSerializedNpmResolutionSnapshot;
use deno_npm_installer::process_state::NpmProcessState;
use deno_npm_installer::process_state::NpmProcessStateKind;
use deno_runtime::colors;
use deno_runtime::deno_tls::deno_native_certs::load_native_certs;
use deno_runtime::deno_tls::rustls;
use deno_runtime::deno_tls::rustls::RootCertStore;
use deno_runtime::deno_tls::rustls_pemfile;
use deno_runtime::deno_tls::webpki_roots;
use deno_runtime::UNSTABLE_ENV_VAR_NAMES;
use deno_semver::npm::NpmPackageReqReference;
use serde::Deserialize;
use serde::Serialize;
use thiserror::Error;

pub fn npm_pkg_req_ref_to_binary_command(
  req_ref: &NpmPackageReqReference,
) -> String {
  req_ref
    .sub_path()
    .map(|s| s.to_string())
    .unwrap_or_else(|| req_ref.req().name.to_string())
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
  /// This variant is not exposed as an option in the CLI, it is used internally
  /// for standalone binaries.
  Bytes(Vec<u8>),
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

  let ca_data =
    maybe_ca_data.or_else(|| std::env::var("DENO_CERT").ok().map(CaData::File));
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

pub static NPM_PROCESS_STATE: LazyLock<Option<NpmProcessState>> =
  LazyLock::new(|| {
    /// Allows for passing either a file descriptor or file path.
    enum FdOrPath {
      Fd(usize),
      Path(PathBuf),
    }

    impl FdOrPath {
      pub fn parse(value: &OsStr) -> Option<Self> {
        if value.is_empty() {
          return None;
        }

        match value.to_string_lossy().parse::<usize>() {
          Ok(value) => Some(FdOrPath::Fd(value)),
          Err(_) => Some(FdOrPath::Path(PathBuf::from(value))),
        }
      }

      pub fn open(&self) -> Option<std::fs::File> {
        use deno_runtime::deno_io::FromRawIoHandle;
        match self {
          // SAFETY: Assume valid file descriptor
          FdOrPath::Fd(fd) => unsafe {
            Some(std::fs::File::from_raw_io_handle(*fd as _))
          },
          FdOrPath::Path(path) => {
            // todo(dsherret): use sys_traits here
            #[allow(clippy::disallowed_methods)]
            std::fs::OpenOptions::new().read(true).open(path).ok()
          }
        }
      }
    }

    use deno_runtime::deno_process::NPM_RESOLUTION_STATE_FD_ENV_VAR_NAME;
    let fd_or_path = std::env::var_os(NPM_RESOLUTION_STATE_FD_ENV_VAR_NAME)?;
    std::env::remove_var(NPM_RESOLUTION_STATE_FD_ENV_VAR_NAME);
    let fd_or_path = FdOrPath::parse(&fd_or_path)?;
    let mut file = fd_or_path.open()?;
    let mut buf = Vec::new();
    // seek to beginning. after the file is written the position will be inherited by this subprocess,
    // and also this file might have been read before
    file.seek(std::io::SeekFrom::Start(0)).unwrap();
    file
      .read_to_end(&mut buf)
      .inspect_err(|e| {
        log::error!(
          "failed to read npm process state from {}: {}",
          match fd_or_path {
            FdOrPath::Fd(fd) => format!("fd {}", fd),
            FdOrPath::Path(path) => path.display().to_string(),
          },
          e
        );
      })
      .ok()?;
    let state: NpmProcessState = serde_json::from_slice(&buf)
      .inspect_err(|e| {
        log::error!(
          "failed to deserialize npm process state: {e} {}",
          String::from_utf8_lossy(&buf)
        )
      })
      .ok()?;
    Some(state)
  });

pub fn resolve_npm_resolution_snapshot(
) -> Result<Option<ValidSerializedNpmResolutionSnapshot>, PackageIdNotFoundError>
{
  if let Some(NpmProcessStateKind::Snapshot(snapshot)) =
    NPM_PROCESS_STATE.as_ref().map(|s| &s.kind)
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
  pub sloppy_imports: bool,
  pub npm_lazy_caching: bool,
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
    maybe_set(
      &mut self.sloppy_imports,
      UNSTABLE_ENV_VAR_NAMES.sloppy_imports,
    );
  }
}
