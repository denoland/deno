use std::collections::HashMap;
use std::collections::HashSet;
use std::env;
use std::path::Path;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::OnceLock;

use deno_terminal::colors;

#[derive(Debug, Clone)]
struct EnvManagerInner {
  // Track which variables came from which files
  file_variables: HashMap<String, HashSet<String>>, // file_path -> set of variable names
  // Track all loaded variables and their sources
  loaded_variables: HashMap<String, String>, // variable_name -> file_path (source)
  // Track original env vars that existed before we started
  original_env: HashMap<String, String>,
}

impl EnvManagerInner {
  fn new() -> Self {
    // Capture the original environment state
    let original_env: HashMap<String, String> = env::vars().collect();

    Self {
      file_variables: HashMap::new(),
      loaded_variables: HashMap::new(),
      original_env,
    }
  }
}

#[derive(Debug, Clone)]
pub struct EnvManager {
  inner: Arc<Mutex<EnvManagerInner>>,
}

// Global singleton instance
static ENV_MANAGER: OnceLock<EnvManager> = OnceLock::new();

impl EnvManager {
  /// Get the global singleton instance
  pub fn instance() -> &'static EnvManager {
    ENV_MANAGER.get_or_init(|| EnvManager {
      inner: Arc::new(Mutex::new(EnvManagerInner::new())),
    })
  }
  ///
  /// # Arguments
  /// * `file_path` - Path to the .env file
  /// * `log_level` - Optional log level to control error message visibility
  ///
  /// # Returns
  /// * `Ok(usize)` - Number of variables successfully loaded
  /// * `Err(Box<dyn std::error::Error>)` - Critical errors that prevent loading
  pub fn load_env_file<P: AsRef<Path>>(
    &self,
    file_path: P,
    log_level: Option<log::Level>,
  ) -> Result<usize, Box<dyn std::error::Error>> {
    let mut inner = self.inner.lock().unwrap();
    let path_str = file_path.as_ref().to_string_lossy().to_string();
    self._unload_env_file_inner(&mut inner, &file_path)?;
    // Check if file exists
    if !file_path.as_ref().exists() {
      inner.file_variables.remove(&path_str);
      // Only show warning if logging is enabled
      if log_level.map(|l| l >= log::Level::Info).unwrap_or(true) {
        eprintln!(
          "{} The environment file specified '{}' was not found.",
          colors::yellow("Warning"),
          path_str
        );
      }
      return Ok(0);
    }

    let mut loaded_count = 0;
    let mut current_file_vars = HashSet::new();

    // Use dotenvy to parse the file
    match dotenvy::from_filename(file_path.as_ref()) {
      Ok(_) => {
        match dotenvy::from_path_iter(file_path.as_ref()) {
          Ok(iter) => {
            for item in iter {
              match item {
                Ok((key, value)) => {
                  // Set the environment variable
                  unsafe {
                    env::set_var(&key, &value);
                  }

                  // Track this variable
                  current_file_vars.insert(key.clone());
                  inner.loaded_variables.insert(key.clone(), path_str.clone());
                  loaded_count += 1;
                }
                Err(e) => {
                  // Handle parsing errors with detailed messages
                  #[allow(clippy::print_stderr)]
                  if log_level.map(|l| l >= log::Level::Info).unwrap_or(true) {
                    match e {
                      dotenvy::Error::LineParse(line, index) => eprintln!(
                        "{} Parsing failed within the specified environment file: {} at index: {} of the value: {}",
                        colors::yellow("Warning"),
                        path_str,
                        index,
                        line
                      ),
                      dotenvy::Error::Io(_) => eprintln!(
                        "{} The `--env-file` flag was used, but the environment file specified '{}' was not found.",
                        colors::yellow("Warning"),
                        path_str
                      ),
                      dotenvy::Error::EnvVar(_) => eprintln!(
                        "{} One or more of the environment variables isn't present or not unicode within the specified environment file: {}",
                        colors::yellow("Warning"),
                        path_str
                      ),
                      _ => eprintln!(
                        "{} Unknown failure occurred with the specified environment file: {}",
                        colors::yellow("Warning"),
                        path_str
                      ),
                    }
                  }
                }
              }
            }
          }
          Err(e) => {
            // This is a critical error - file exists but can't be read
            return Err(format!("Failed to read {}: {}", path_str, e).into());
          }
        }
      }
      Err(error) => {
        // Handle different types of errors with appropriate logging
        #[allow(clippy::print_stderr)]
        if log_level.map(|l| l >= log::Level::Info).unwrap_or(true) {
          match error {
            dotenvy::Error::LineParse(line, index) => eprintln!(
              "{} Parsing failed within the specified environment file: {} at index: {} of the value: {}",
              colors::yellow("Warning"),
              path_str,
              index,
              line
            ),
            dotenvy::Error::Io(_) => eprintln!(
              "{} The `--env-file` flag was used, but the environment file specified '{}' was not found.",
              colors::yellow("Warning"),
              path_str
            ),
            dotenvy::Error::EnvVar(_) => eprintln!(
              "{} One or more of the environment variables isn't present or not unicode within the specified environment file: {}",
              colors::yellow("Warning"),
              path_str
            ),
            _ => eprintln!(
              "{} Unknown failure occurred with the specified environment file: {}",
              colors::yellow("Warning"),
              path_str
            ),
          }
        }
      }
    }

    // Store the variables for this file
    inner.file_variables.insert(path_str, current_file_vars);

    Ok(loaded_count)
  }

  /// Internal helper for unloading (to avoid double-locking)
  fn _unload_env_file_inner<P: AsRef<Path>>(
    &self,
    inner: &mut EnvManagerInner,
    file_path: P,
  ) -> Result<(), Box<dyn std::error::Error>> {
    let path_str: String = file_path.as_ref().to_string_lossy().to_string();
    if let Some(variables) = inner.file_variables.remove(&path_str) {
      for var_name in variables {
        // Remove from loaded_variables tracking
        inner.loaded_variables.remove(&var_name);

        // Restore original value or remove entirely
        if let Some(original_value) = inner.original_env.get(&var_name) {
          unsafe {
            env::set_var(&var_name, original_value);
          }
        } else {
          // Only remove the variable if its current value is the same as when we set it (i.e., not changed by user/code)
          match env::var(&var_name) {
            Ok(current_value) => {
              // If the variable is not present in original_env, we set it, so only remove if unchanged
              if current_value.as_str()
                == inner
                  .loaded_variables
                  .get(&var_name)
                  .unwrap_or(&"".to_string())
              {
                unsafe {
                  env::remove_var(&var_name);
                }
              }
            }
            Err(_) => {
              // If the variable doesn't exist, nothing to do
            }
          }
        }
      }
    }

    Ok(())
  }

  /// Load multiple env files in order (later files override earlier ones)
  pub fn load_env_variables_from_env_files<P: AsRef<Path>>(
    &self,
    file_paths: Option<&Vec<P>>,
    log_level: Option<log::Level>,
  ) -> usize {
    let Some(env_file_names) = file_paths else {
      return 0;
    };

    let mut total_loaded = 0;

    // Process files in reverse order (matches original behavior)
    for env_file_name in env_file_names.iter().rev() {
      match self.load_env_file(env_file_name, log_level) {
        Ok(count) => {
          total_loaded += count;
        }
        Err(e) => {
          // Log critical errors but continue processing other files
          #[allow(clippy::print_stderr)]
          if log_level.map(|l| l >= log::Level::Info).unwrap_or(true) {
            eprintln!(
              "{} Critical error loading {}: {}",
              colors::yellow("Warning"),
              env_file_name.as_ref().to_string_lossy(),
              e
            );
          }
        }
      }
    }

    total_loaded
  }
}

pub fn load_env_variables_from_env_files<P: AsRef<Path>>(
  file_paths: &[P],
  flags_log_level: Option<log::Level>,
) -> Result<usize, Box<dyn std::error::Error>> {
  let file_paths_vec: Vec<&P> = file_paths.iter().collect();
  Ok(
    EnvManager::instance().load_env_variables_from_env_files(
      Some(&file_paths_vec),
      flags_log_level,
    ),
  )
}
