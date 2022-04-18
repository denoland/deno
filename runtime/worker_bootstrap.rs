use crate::ops::runtime::ppid;
use deno_core::serde_json;
use deno_core::serde_json::json;
use deno_core::ModuleSpecifier;

/// Common bootstrap options for MainWorker & WebWorker
#[derive(Clone)]
pub struct BootstrapOptions {
  /// Sets `Deno.args` in JS runtime.
  pub args: Vec<String>,
  pub cpu_count: usize,
  pub debug_flag: bool,
  pub enable_testing_features: bool,
  pub location: Option<ModuleSpecifier>,
  /// Sets `Deno.noColor` in JS runtime.
  pub no_color: bool,
  pub is_tty: bool,
  /// Sets `Deno.version.deno` in JS runtime.
  pub runtime_version: String,
  /// Sets `Deno.version.typescript` in JS runtime.
  pub ts_version: String,
  pub unstable: bool,
}

impl BootstrapOptions {
  pub fn as_json(&self) -> String {
    let payload = json!({
      // Shared bootstrap args
      "args": self.args,
      "cpuCount": self.cpu_count,
      "debugFlag": self.debug_flag,
      "denoVersion": self.runtime_version,
      "location": self.location,
      "noColor": self.no_color,
      "isTty": self.is_tty,
      "tsVersion": self.ts_version,
      "unstableFlag": self.unstable,
      // Web worker only
      "enableTestingFeaturesFlag": self.enable_testing_features,
      // Env values
      "pid": std::process::id(),
      "ppid": ppid(),
      "target": env!("TARGET"),
      "v8Version": deno_core::v8_version(),
    });
    serde_json::to_string_pretty(&payload).unwrap()
  }
}
