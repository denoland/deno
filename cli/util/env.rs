// Copyright 2018-2026 the Deno authors. MIT license.

use std::borrow::Cow;
use std::collections::HashMap;
use std::collections::HashSet;
use std::env;
use std::ffi::OsStr;
use std::ffi::OsString;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::OnceLock;

use deno_terminal::colors;

use crate::sys::CliSys;

/// Environment variables that configure Deno's own runtime behavior and must
/// not be settable from an `.env` file loaded via `--env` / `--env-file`.
///
/// An `.env` file ships alongside the code it accompanies, so letting it set
/// Deno's own control variables would let that file change runtime behavior
/// the user did not opt into, for example silently enabling tunnel mode
/// (`DENO_CONNECTED`) or overriding the tunnel control endpoint
/// (`DENO_DEPLOY_TUNNEL_ENDPOINT`). Env files are meant to provide
/// configuration to the user's program, not to reconfigure Deno itself, so
/// these keys are ignored when they originate from an env file.
const ENV_FILE_DENYLIST: &[&str] =
  &["DENO_CONNECTED", "DENO_DEPLOY_TUNNEL_ENDPOINT"];

/// Whether `key` is a Deno-internal control variable that must not be set from
/// an env file. The comparison is ASCII case-insensitive because environment
/// variable lookups are case-insensitive on Windows.
fn is_denied_env_file_key(key: &OsStr) -> bool {
  match key.to_str() {
    Some(key) => ENV_FILE_DENYLIST
      .iter()
      .any(|denied| key.eq_ignore_ascii_case(denied)),
    None => false,
  }
}

pub fn handle_denied_env_file_key(
  key: &OsStr,
  file_path: &Path,
  log_level: Option<log::Level>,
) {
  #[allow(clippy::print_stderr, reason = "can't use log crate yet")]
  if log_level.map(|l| l >= log::Level::Info).unwrap_or(true) {
    eprintln!(
      "{} Ignoring '{}' from environment file '{}': this variable controls Deno's own runtime behavior and cannot be set from an env file.",
      colors::yellow("Warning"),
      key.to_string_lossy(),
      file_path.display(),
    )
  }
}

/// Resolves the mode used to select the `--env-file` cascade files. Prefers
/// the explicit `--mode` flag, falling back to the `DENO_ENV` environment
/// variable, and otherwise leaving the mode unset (only `.env` and
/// `.env.local` are loaded).
pub fn resolve_env_file_mode(flag_mode: Option<&str>) -> Option<String> {
  flag_mode
    .map(str::to_string)
    .or_else(|| env::var("DENO_ENV").ok().filter(|value| !value.is_empty()))
}

/// Expands each base `--env-file` specifier into its conventional cascade
/// (see [`deno_dotenv::cascade_paths`]), preserving the order of the passed
/// files. The resulting list is in increasing precedence order, which is what
/// [`load_env_variables_from_env_files`] expects.
pub fn expand_env_file_cascade(
  env_files: &[String],
  mode: Option<&str>,
) -> Vec<String> {
  env_files
    .iter()
    .flat_map(|base| deno_dotenv::cascade_paths(base, mode))
    .collect()
}

pub fn resolve_cwd(
  initial_cwd: Option<&Path>,
) -> Result<Cow<'_, Path>, std::io::Error> {
  match initial_cwd {
    Some(initial_cwd) => Ok(Cow::Borrowed(initial_cwd)),
    #[allow(
      clippy::disallowed_methods,
      reason = "ok because the lint recommends using this method"
    )]
    None => std::env::current_dir().map(Cow::Owned).map_err(|err| {
      std::io::Error::new(
        err.kind(),
        format!("could not read current working directory: {err}"),
      )
    }),
  }
}

/// Like `resolve_cwd`, but falls back to a sensible default (the system
/// root) when the current working directory can't be determined — for
/// example when it has been unlinked. This matches Node.js semantics where
/// the REPL still starts even if the parent process's cwd was deleted.
pub fn resolve_cwd_or_fallback(initial_cwd: Option<&Path>) -> PathBuf {
  match resolve_cwd(initial_cwd) {
    Ok(cwd) => cwd.into_owned(),
    Err(_) => fallback_cwd(),
  }
}

fn fallback_cwd() -> PathBuf {
  if cfg!(windows) {
    // System drive root, e.g. `C:\`.
    std::env::var_os("SystemDrive")
      .map(|d| {
        let mut p = PathBuf::from(d);
        p.push("\\");
        p
      })
      .unwrap_or_else(|| PathBuf::from("C:\\"))
  } else {
    PathBuf::from("/")
  }
}

#[derive(Debug, Clone)]
struct WatchEnvTrackerInner {
  // Track all loaded variables and their values
  loaded_variables: HashSet<OsString>,
  // Track variables that are no longer present in any loaded file
  unused_variables: HashSet<OsString>,
  // Track original env vars that existed before we started
  original_env: HashMap<OsString, OsString>,
}

impl WatchEnvTrackerInner {
  fn new() -> Self {
    // Capture the original environment state
    let original_env: HashMap<OsString, OsString> = env::vars_os().collect();

    Self {
      loaded_variables: Default::default(),
      unused_variables: Default::default(),
      original_env,
    }
  }
}

#[derive(Debug, Clone)]
pub struct WatchEnvTracker {
  inner: Arc<Mutex<WatchEnvTrackerInner>>,
}

// Global singleton instance
static WATCH_ENV_TRACKER: OnceLock<WatchEnvTracker> = OnceLock::new();

impl WatchEnvTracker {
  /// Get the global singleton instance
  pub fn snapshot() -> &'static WatchEnvTracker {
    WATCH_ENV_TRACKER.get_or_init(|| WatchEnvTracker {
      inner: Arc::new(Mutex::new(WatchEnvTrackerInner::new())),
    })
  }

  // Internal method that accepts an already-acquired lock to avoid deadlocks
  fn load_env_file_inner(
    &self,
    cwd: &Path,
    env_file: &str,
    log_level: Option<log::Level>,
    inner: &mut WatchEnvTrackerInner,
  ) {
    let (file_path, content) = match deno_dotenv::find_path_and_content(
      &CliSys::default(),
      cwd,
      env_file,
    ) {
      Ok(Some(result)) => result,
      Ok(None) => {
        handle_dotenv_not_found(env_file, log_level);
        return;
      }
      Err(err) => {
        handle_dotenv_io_error(&err, log_level);
        return;
      }
    };

    match deno_dotenv::from_content_sanitized_iter_with_substitution(
      &CliSys::default(),
      &content,
    ) {
      Ok(iter) => {
        for item in iter {
          match item {
            Ok((key, value)) => {
              // Convert to OsString for consistency
              let key_os = OsString::from(key);
              let value_os = OsString::from(value);

              // Process-level env vars should always take precedence over env files.
              if inner.original_env.contains_key(&key_os) {
                #[allow(
                  clippy::print_stderr,
                  reason = "can't use log crate yet"
                )]
                if log_level.map(|l| l >= log::Level::Debug).unwrap_or(false) {
                  eprintln!(
                    "{} Variable '{}' already exists in the process environment, skipping value from '{}'",
                    colors::yellow("Debug"),
                    key_os.to_string_lossy(),
                    file_path.display()
                  );
                }
                continue;
              }

              // Check if this variable is already loaded from a previous file
              if inner.loaded_variables.contains(&key_os) {
                // Variable already exists from a previous file, skip it
                #[allow(
                  clippy::print_stderr,
                  reason = "can't use log crate yet"
                )]
                if log_level.map(|l| l >= log::Level::Debug).unwrap_or(false) {
                  eprintln!(
                    "{} Variable '{}' already loaded from '{}', skipping value from '{}'",
                    colors::yellow("Debug"),
                    key_os.to_string_lossy(),
                    inner
                      .loaded_variables
                      .get(&key_os)
                      .map(|k| k.to_string_lossy().to_string())
                      .unwrap_or_else(|| "unknown".to_string()),
                    file_path.display()
                  );
                }
                continue;
              }

              if is_denied_env_file_key(&key_os) {
                handle_denied_env_file_key(&key_os, &file_path, log_level);
                continue;
              }

              // Set the environment variable
              // SAFETY: We're setting environment variables with valid UTF-8 strings
              // from the .env file. Both key and value are guaranteed to be valid strings.
              unsafe {
                env::set_var(&key_os, &value_os);
              }

              // Track this variable
              inner.loaded_variables.insert(key_os.clone());
              inner.unused_variables.remove(&key_os);
            }
            Err(e) => {
              handle_dotenv_error(&e, &file_path, log_level);
            }
          }
        }
      }
      Err(e) => {
        handle_dotenv_error(&e, &file_path, log_level);
      }
    }
  }

  /// Clean up variables that are no longer present in any loaded file
  fn _cleanup_removed_variables(
    &self,
    inner: &mut WatchEnvTrackerInner,
    log_level: Option<log::Level>,
  ) {
    for var_name in inner.unused_variables.iter() {
      if !inner.original_env.contains_key(var_name) {
        // SAFETY: We're removing an environment variable that we previously set
        unsafe {
          env::remove_var(var_name);
        }

        #[allow(clippy::print_stderr, reason = "can't use log crate yet")]
        if log_level.map(|l| l >= log::Level::Debug).unwrap_or(false) {
          eprintln!(
            "{} Variable '{}' removed from environment as it's no longer present in any loaded file",
            colors::yellow("Debug"),
            var_name.to_string_lossy()
          );
        }
      } else {
        let original_value = inner.original_env.get(var_name).unwrap();
        // SAFETY: We're setting an environment variable to a value we control
        unsafe {
          env::set_var(var_name, original_value);
        }

        #[allow(clippy::print_stderr, reason = "can't use log crate yet")]
        if log_level.map(|l| l >= log::Level::Debug).unwrap_or(false) {
          eprintln!(
            "{} Variable '{}' restored to original value as it's no longer present in any loaded file",
            colors::yellow("Debug"),
            var_name.to_string_lossy()
          );
        }
      }
    }
  }

  // Load multiple env files in reverse order (later files take precedence over earlier ones)
  pub fn load_env_variables_from_env_files(
    &self,
    cwd: &Path,
    env_files: &[String],
    log_level: Option<log::Level>,
  ) {
    let mut inner = self.inner.lock().unwrap();

    inner.unused_variables = std::mem::take(&mut inner.loaded_variables);
    inner.loaded_variables = HashSet::new();

    for env_file_path in env_files.iter().rev() {
      self.load_env_file_inner(cwd, env_file_path, log_level, &mut inner);
    }

    self._cleanup_removed_variables(&mut inner, log_level);
  }
}

pub fn load_env_variables_from_env_files(
  cwd: &Path,
  env_file_names: &[String],
  flags_log_level: Option<log::Level>,
) {
  let original_env_keys: HashSet<OsString> =
    env::vars_os().map(|(key, _)| key).collect();
  let mut loaded_keys = HashSet::new();

  for env_file_name in env_file_names.iter().rev() {
    let (env_file_path, content) = match deno_dotenv::find_path_and_content(
      &CliSys::default(),
      cwd,
      env_file_name,
    ) {
      Ok(Some(resolved)) => resolved,
      Ok(None) => {
        handle_dotenv_not_found(env_file_name, flags_log_level);
        continue;
      }
      Err(err) => {
        handle_dotenv_io_error(&err, flags_log_level);
        continue;
      }
    };
    let iter = match deno_dotenv::from_content_sanitized_iter_with_substitution(
      &sys_traits::impls::RealSys,
      &content,
    ) {
      Ok(iter) => iter,
      Err(err) => {
        handle_dotenv_error(&err, &env_file_path, flags_log_level);
        continue;
      }
    };

    for item in iter {
      let (key, value) = match item {
        Ok(pair) => pair,
        Err(error) => {
          handle_dotenv_error(&error, &env_file_path, flags_log_level);
          break;
        }
      };

      let key_os = OsString::from(key);
      if original_env_keys.contains(&key_os) || loaded_keys.contains(&key_os) {
        continue;
      }

      if is_denied_env_file_key(&key_os) {
        handle_denied_env_file_key(&key_os, &env_file_path, flags_log_level);
        continue;
      }

      // SAFETY: We're setting environment variables with sanitized key/value strings from a .env file.
      unsafe {
        env::set_var(&key_os, value);
      }
      loaded_keys.insert(key_os);
    }
  }
}

pub fn handle_dotenv_error(
  error: &deno_dotenv::ParseError,
  file_path: &Path,
  log_level: Option<log::Level>,
) {
  #[allow(clippy::print_stderr, reason = "can't use log crate yet")]
  if log_level.map(|l| l >= log::Level::Info).unwrap_or(true) {
    eprintln!(
      "{} Failed parsing value '{}' at index {} within the specified environment file.\n    at {}",
      colors::yellow("Warning"),
      error.line,
      error.index,
      file_path.display(),
    )
  }
}

pub fn handle_dotenv_io_error(
  error: &deno_dotenv::FindPathAndContentError,
  log_level: Option<log::Level>,
) {
  #[allow(clippy::print_stderr, reason = "can't use log crate yet")]
  if log_level.map(|l| l >= log::Level::Info).unwrap_or(true) {
    eprintln!(
      "{} Error reading from environment file: {}\n    at {}",
      colors::yellow("Warning"),
      error.source,
      error.path.display(),
    )
  }
}

pub fn handle_dotenv_not_found(specifier: &str, log_level: Option<log::Level>) {
  #[allow(clippy::print_stderr, reason = "can't use log crate yet")]
  if log_level.map(|l| l >= log::Level::Info).unwrap_or(true) {
    eprintln!(
      "{} The `--env-file` flag was used, but the environment file specified '{}' was not found.",
      colors::yellow("Warning"),
      specifier,
    )
  }
}
