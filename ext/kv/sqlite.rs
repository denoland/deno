// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::cell::RefCell;
use std::collections::HashMap;
use std::env::current_dir;
use std::io::ErrorKind;
use std::marker::PhantomData;
use std::path::Path;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::OnceLock;

use async_trait::async_trait;
use deno_core::error::type_error;
use deno_core::error::AnyError;
use deno_core::unsync::spawn_blocking;
use deno_core::OpState;
use deno_node::PathClean;
pub use denokv_sqlite::SqliteBackendError;
use denokv_sqlite::SqliteConfig;
use denokv_sqlite::SqliteNotifier;
use rand::SeedableRng;
use rusqlite::OpenFlags;

use crate::DatabaseHandler;

static SQLITE_NOTIFIERS_MAP: OnceLock<Mutex<HashMap<PathBuf, SqliteNotifier>>> =
  OnceLock::new();

pub struct SqliteDbHandler<P: SqliteDbHandlerPermissions + 'static> {
  pub default_storage_dir: Option<PathBuf>,
  versionstamp_rng_seed: Option<u64>,
  _permissions: PhantomData<P>,
}

pub trait SqliteDbHandlerPermissions {
  fn check_read(&mut self, p: &Path, api_name: &str) -> Result<(), AnyError>;
  fn check_write(&mut self, p: &Path, api_name: &str) -> Result<(), AnyError>;
}

impl SqliteDbHandlerPermissions for deno_permissions::PermissionsContainer {
  #[inline(always)]
  fn check_read(&mut self, p: &Path, api_name: &str) -> Result<(), AnyError> {
    deno_permissions::PermissionsContainer::check_read(self, p, api_name)
  }

  #[inline(always)]
  fn check_write(&mut self, p: &Path, api_name: &str) -> Result<(), AnyError> {
    deno_permissions::PermissionsContainer::check_write(self, p, api_name)
  }
}

impl<P: SqliteDbHandlerPermissions> SqliteDbHandler<P> {
  pub fn new(
    default_storage_dir: Option<PathBuf>,
    versionstamp_rng_seed: Option<u64>,
  ) -> Self {
    Self {
      default_storage_dir,
      versionstamp_rng_seed,
      _permissions: PhantomData,
    }
  }
}

#[async_trait(?Send)]
impl<P: SqliteDbHandlerPermissions> DatabaseHandler for SqliteDbHandler<P> {
  type DB = denokv_sqlite::Sqlite;

  async fn open(
    &self,
    state: Rc<RefCell<OpState>>,
    path: Option<String>,
  ) -> Result<Self::DB, AnyError> {
    // Validate path
    if let Some(path) = &path {
      if path != ":memory:" {
        if path.is_empty() {
          return Err(type_error("Filename cannot be empty"));
        }
        if path.starts_with(':') {
          return Err(type_error(
            "Filename cannot start with ':' unless prefixed with './'",
          ));
        }
        let path = Path::new(path);
        {
          let mut state = state.borrow_mut();
          let permissions = state.borrow_mut::<P>();
          permissions.check_read(path, "Deno.openKv")?;
          permissions.check_write(path, "Deno.openKv")?;
        }
      }
    }

    let path = path.clone();
    let default_storage_dir = self.default_storage_dir.clone();
    type ConnGen =
      Arc<dyn Fn() -> rusqlite::Result<rusqlite::Connection> + Send + Sync>;
    let (conn_gen, notifier_key): (ConnGen, _) = spawn_blocking(move || {
      denokv_sqlite::sqlite_retry_loop(|| {
        let (conn, notifier_key) = match (path.as_deref(), &default_storage_dir)
        {
          (Some(":memory:"), _) | (None, None) => (
            Arc::new(rusqlite::Connection::open_in_memory) as ConnGen,
            None,
          ),
          (Some(path), _) => {
            let flags =
              OpenFlags::default().difference(OpenFlags::SQLITE_OPEN_URI);
            let resolved_path = canonicalize_path(&PathBuf::from(path))
              .map_err(anyhow::Error::from)?;
            let path = path.to_string();
            (
              Arc::new(move || {
                rusqlite::Connection::open_with_flags(&path, flags)
              }) as ConnGen,
              Some(resolved_path),
            )
          }
          (None, Some(path)) => {
            std::fs::create_dir_all(path).map_err(anyhow::Error::from)?;
            let path = path.join("kv.sqlite3");
            let path2 = path.clone();
            (
              Arc::new(move || rusqlite::Connection::open(&path2)) as ConnGen,
              Some(path),
            )
          }
        };

        Ok::<_, SqliteBackendError>((conn, notifier_key))
      })
    })
    .await
    .unwrap()?;

    let notifier = if let Some(notifier_key) = notifier_key {
      SQLITE_NOTIFIERS_MAP
        .get_or_init(Default::default)
        .lock()
        .unwrap()
        .entry(notifier_key)
        .or_default()
        .clone()
    } else {
      SqliteNotifier::default()
    };

    let versionstamp_rng_seed = self.versionstamp_rng_seed;

    let config = SqliteConfig {
      batch_timeout: None,
      num_workers: 1,
    };

    denokv_sqlite::Sqlite::new(
      move || {
        let conn = conn_gen()?;
        conn.pragma_update(None, "journal_mode", "wal")?;
        Ok((
          conn,
          match versionstamp_rng_seed {
            Some(seed) => Box::new(rand::rngs::StdRng::seed_from_u64(seed)),
            None => Box::new(rand::rngs::StdRng::from_entropy()),
          },
        ))
      },
      notifier,
      config,
    )
  }
}

/// Same as Path::canonicalize, but also handles non-existing paths.
fn canonicalize_path(path: &Path) -> Result<PathBuf, AnyError> {
  let path = path.to_path_buf().clean();
  let mut path = path;
  let mut names_stack = Vec::new();
  loop {
    match path.canonicalize() {
      Ok(mut canonicalized_path) => {
        for name in names_stack.into_iter().rev() {
          canonicalized_path = canonicalized_path.join(name);
        }
        return Ok(canonicalized_path);
      }
      Err(err) if err.kind() == ErrorKind::NotFound => {
        let file_name = path.file_name().map(|os_str| os_str.to_os_string());
        if let Some(file_name) = file_name {
          names_stack.push(file_name.to_str().unwrap().to_string());
          path = path.parent().unwrap().to_path_buf();
        } else {
          names_stack.push(path.to_str().unwrap().to_string());
          let current_dir = current_dir()?;
          path.clone_from(&current_dir);
        }
      }
      Err(err) => return Err(err.into()),
    }
  }
}
