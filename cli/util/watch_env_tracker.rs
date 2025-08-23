// Copyright 2018-2025 the Deno authors. MIT license.

use std::collections::HashMap;
use std::collections::HashSet;
use std::env;
use std::ffi::OsString;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::OnceLock;

use deno_terminal::colors;

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
      loaded_variables: HashSet::new(),
      unused_variables: HashSet::new(),
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

  // Consolidated error handling function
  fn handle_dotenvy_error(
    error: dotenvy::Error,
    file_path: &Path,
    log_level: Option<log::Level>,
  ) {
    #[allow(clippy::print_stderr)]
    if log_level.map(|l| l >= log::Level::Info).unwrap_or(true) {
      match error {
        dotenvy::Error::LineParse(line, index) => eprintln!(
          "{} Parsing failed within the specified environment file: {} at index: {} of the value: {}",
          colors::yellow("Warning"),
          file_path.display(),
          index,
          line
        ),
        dotenvy::Error::Io(_) => eprintln!(
          "{} The `--env-file` flag was used, but the environment file specified '{}' was not found.",
          colors::yellow("Warning"),
          file_path.display()
        ),
        dotenvy::Error::EnvVar(_) => eprintln!(
          "{} One or more of the environment variables isn't present or not unicode within the specified environment file: {}",
          colors::yellow("Warning"),
          file_path.display()
        ),
        _ => eprintln!(
          "{} Unknown failure occurred with the specified environment file: {}",
          colors::yellow("Warning"),
          file_path.display()
        ),
      }
    }
  }

  // Internal method that accepts an already-acquired lock to avoid deadlocks
  fn load_env_file_inner(
    &self,
    file_path: PathBuf,
    log_level: Option<log::Level>,
    inner: &mut WatchEnvTrackerInner,
  ) {
    // Check if file exists
    if !file_path.exists() {
      // Only show warning if logging is enabled
      #[allow(clippy::print_stderr)]
      if log_level.map(|l| l >= log::Level::Info).unwrap_or(true) {
        eprintln!(
          "{} The environment file specified '{}' was not found.",
          colors::yellow("Warning"),
          file_path.display()
        );
      }
      return;
    }

    match dotenvy::from_path_iter(&file_path) {
      Ok(iter) => {
        for item in iter {
          match item {
            Ok((key, value)) => {
              // Convert to OsString for consistency
              let key_os = OsString::from(key);
              let value_os = OsString::from(value);

              // Check if this variable is already loaded from a previous file
              if inner.loaded_variables.contains(&key_os) {
                // Variable already exists from a previous file, skip it
                #[allow(clippy::print_stderr)]
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
              Self::handle_dotenvy_error(e, &file_path, log_level);
            }
          }
        }
      }
      Err(e) =>
      {
        #[allow(clippy::print_stderr)]
        if log_level.map(|l| l >= log::Level::Info).unwrap_or(true) {
          eprintln!(
            "{} Failed to read {}: {}",
            colors::yellow("Warning"),
            file_path.display(),
            e
          );
        }
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

        #[allow(clippy::print_stderr)]
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

        #[allow(clippy::print_stderr)]
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
    file_paths: Option<&Vec<PathBuf>>,
    log_level: Option<log::Level>,
  ) {
    let Some(env_file_names) = file_paths else {
      return;
    };

    let mut inner = self.inner.lock().unwrap();

    inner.unused_variables = std::mem::take(&mut inner.loaded_variables);
    inner.loaded_variables = HashSet::new();

    for env_file_name in env_file_names.iter().rev() {
      self.load_env_file_inner(
        env_file_name.to_path_buf(),
        log_level,
        &mut inner,
      );
    }

    self._cleanup_removed_variables(&mut inner, log_level);
  }
}

pub fn load_env_variables_from_env_files(
  filename: Option<&Vec<PathBuf>>,
  flags_log_level: Option<log::Level>,
) {
  let Some(env_file_names) = filename else {
    return;
  };

  for env_file_name in env_file_names.iter().rev() {
    match dotenvy::from_filename(env_file_name) {
      Ok(_) => (),
      Err(error) => {
        WatchEnvTracker::handle_dotenvy_error(
          error,
          env_file_name,
          flags_log_level,
        );
      }
    }
  }
}
