// Copyright 2018-2026 the Deno authors. MIT license.

use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::Path;
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

fn resolve_sqlite_system_path_alias(path: &Path) -> PathBuf {
  // SQLITE_OPEN_NOFOLLOW rejects symlinks in every path component. macOS
  // exposes its temporary directories through root-owned aliases, so resolve
  // only those fixed system prefixes and leave every user-controlled path
  // component visible to SQLite.
  #[cfg(target_os = "macos")]
  for prefix in [Path::new("/var"), Path::new("/tmp")] {
    if let Ok(suffix) = path.strip_prefix(prefix)
      && let Ok(prefix) = deno_path_util::fs::canonicalize_path_maybe_not_exists(
        &sys_traits::impls::RealSys,
        prefix,
      )
    {
      return prefix.join(suffix);
    }
  }
  path.to_path_buf()
}

/// SQLite does not enforce `SQLITE_OPEN_NOFOLLOW` on Windows (its
/// `winFullPathname` never resolves reparse points), so reject symlinks and
/// junctions in every path component manually before opening.
#[cfg(windows)]
fn refuse_reparse_point_components(path: &Path) -> std::io::Result<()> {
  let mut current = PathBuf::new();
  for component in path.components() {
    current.push(component);
    #[allow(
      clippy::disallowed_methods,
      reason = "the database path is always on the real fs"
    )]
    match std::fs::symlink_metadata(&current) {
      Ok(metadata) if metadata.file_type().is_symlink() => {
        return Err(std::io::Error::new(
          std::io::ErrorKind::InvalidInput,
          format!(
            "unable to open database file: \"{}\" is a symlink",
            current.display()
          ),
        ));
      }
      Ok(_) => {}
      // Missing components are created (or rejected) by SQLite itself.
      Err(_) => break,
    }
  }
  Ok(())
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
        Ok(Some(PathOrInMemory::Path(
          resolve_sqlite_system_path_alias(&path),
        )))
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
            let flags = OpenFlags::default()
              .difference(OpenFlags::SQLITE_OPEN_URI)
              | OpenFlags::SQLITE_OPEN_NOFOLLOW;
            #[cfg(windows)]
            refuse_reparse_point_components(path)
              .map_err(JsErrorBox::from_err)?;
            // Open with the unresolved path so SQLITE_OPEN_NOFOLLOW keeps
            // rejecting symlinks, but key the notifier on the normalized
            // absolute path so lexical aliases of the same database (e.g.
            // `db.sqlite` and `./db.sqlite`) share one notifier. Resolving
            // symlinks here cannot redirect the open: paths containing
            // symlinks are refused above.
            let notifier_key =
              deno_path_util::fs::canonicalize_path_maybe_not_exists(
                &sys_traits::impls::RealSys,
                path,
              )
              .map_err(JsErrorBox::from_err)?;
            let path = path.clone();
            (
              Arc::new(move || {
                rusqlite::Connection::open_with_flags(&path, flags)
              }) as ConnGen,
              Some(notifier_key),
            )
          }
          (None, Some(path)) => {
            #[allow(
              clippy::disallowed_methods,
              reason = "the storage directory is always on the real fs"
            )]
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
