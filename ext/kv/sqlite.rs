// Copyright 2018-2025 the Deno authors. MIT license.

use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::OnceLock;

use async_trait::async_trait;
use deno_core::OpState;
use deno_core::unsync::spawn_blocking;
use deno_error::JsErrorBox;
use deno_permissions::OpenAccessKind;
use deno_permissions::PermissionsContainer;
pub use denokv_sqlite::SqliteBackendError;
use denokv_sqlite::SqliteConfig;
use denokv_sqlite::SqliteNotifier;
use rand::SeedableRng;
use rusqlite::OpenFlags;

use crate::DatabaseHandler;

static SQLITE_NOTIFIERS_MAP: OnceLock<Mutex<HashMap<PathBuf, SqliteNotifier>>> =
  OnceLock::new();

pub struct SqliteDbHandler {
  pub default_storage_dir: Option<PathBuf>,
  versionstamp_rng_seed: Option<u64>,
}

impl SqliteDbHandler {
  pub fn new(
    default_storage_dir: Option<PathBuf>,
    versionstamp_rng_seed: Option<u64>,
  ) -> Self {
    Self {
      default_storage_dir,
      versionstamp_rng_seed,
    }
  }
}

deno_error::js_error_wrapper!(
  SqliteBackendError,
  JsSqliteBackendError,
  "TypeError"
);

#[derive(Debug)]
enum Mode {
  Disk,
  InMemory,
}

#[async_trait(?Send)]
impl DatabaseHandler for SqliteDbHandler {
  type DB = denokv_sqlite::Sqlite;

  async fn open(
    &self,
    state: Rc<RefCell<OpState>>,
    path: Option<String>,
  ) -> Result<Self::DB, JsErrorBox> {
    enum PathOrInMemory {
      InMemory,
      Path(PathBuf),
    }

    #[must_use = "the resolved return value to mitigate time-of-check to time-of-use issues"]
    fn validate_path(
      state: &RefCell<OpState>,
      path: Option<String>,
    ) -> Result<Option<PathOrInMemory>, JsErrorBox> {
      let Some(path) = path else {
        return Ok(None);
      };
      if path == ":memory:" {
        return Ok(Some(PathOrInMemory::InMemory));
      }
      if path.is_empty() {
        return Err(JsErrorBox::type_error("Filename cannot be empty"));
      }
      if path.starts_with(':') {
        return Err(JsErrorBox::type_error(
          "Filename cannot start with ':' unless prefixed with './'",
        ));
      }
      {
        let state = state.borrow();
        let permissions = state.borrow::<PermissionsContainer>();
        let path = permissions
          .check_open(
            Cow::Owned(PathBuf::from(path)),
            OpenAccessKind::ReadWriteNoFollow,
            Some("Deno.openKv"),
          )
          .map_err(JsErrorBox::from_err)?;
        Ok(Some(PathOrInMemory::Path(path.into_owned_path())))
      }
    }

    let path = validate_path(&state, path)?;
    let default_storage_dir = self.default_storage_dir.clone();
    type ConnGen =
      Arc<dyn Fn() -> rusqlite::Result<rusqlite::Connection> + Send + Sync>;
    let (conn_gen, notifier_key): (ConnGen, _) = spawn_blocking(move || {
      denokv_sqlite::sqlite_retry_loop(move || {
        let mode = match std::env::var("DENO_KV_DB_MODE")
          .unwrap_or_default()
          .as_str()
        {
          "disk" | "" => Mode::Disk,
          "memory" => Mode::InMemory,
          _ => {
            log::warn!("Unknown DENO_KV_DB_MODE value, defaulting to disk");
            Mode::Disk
          }
        };

        if matches!(mode, Mode::InMemory) {
          return Ok::<_, SqliteBackendError>((
            Arc::new(rusqlite::Connection::open_in_memory) as ConnGen,
            None,
          ));
        }

        let (conn, notifier_key) = match (path.as_ref(), &default_storage_dir) {
          (Some(PathOrInMemory::InMemory), _) | (None, None) => (
            Arc::new(rusqlite::Connection::open_in_memory) as ConnGen,
            None,
          ),
          (Some(PathOrInMemory::Path(path)), _) => {
            let flags =
              OpenFlags::default().difference(OpenFlags::SQLITE_OPEN_URI);
            let resolved_path =
              deno_path_util::fs::canonicalize_path_maybe_not_exists(
                // todo(dsherret): probably should use the FileSystem in the op state instead
                &sys_traits::impls::RealSys,
                path,
              )
              .map_err(JsErrorBox::from_err)?;
            let path = path.clone();
            (
              Arc::new(move || {
                rusqlite::Connection::open_with_flags(&path, flags)
              }) as ConnGen,
              Some(resolved_path),
            )
          }
          (None, Some(path)) => {
            std::fs::create_dir_all(path).map_err(JsErrorBox::from_err)?;
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
    .unwrap()
    .map_err(JsErrorBox::from_err)?;

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
        let conn =
          conn_gen().map_err(|e| JsErrorBox::generic(e.to_string()))?;
        conn
          .pragma_update(None, "journal_mode", "wal")
          .map_err(|e| JsErrorBox::generic(e.to_string()))?;
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
    .map_err(|e| JsErrorBox::generic(e.to_string()))
  }
}
