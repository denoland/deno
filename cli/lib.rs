use args::Flags;
pub use deno_runtime::colors;
use factory::CliFactory;
use util::display;

pub use deno_config;
pub use deno_runtime;
pub use deno_graph;

pub mod args;
pub mod auth_tokens;
pub mod cache;
pub mod deno_std;
pub mod emit;
pub mod errors;
pub mod factory;
pub mod file_fetcher;
pub mod graph_util;
pub mod http_util;
pub mod js;
pub mod lsp;
pub mod module_loader;
pub mod napi;
pub mod node;
pub mod npm;
pub mod ops;
pub mod resolver;
pub mod standalone;
pub mod tools;
pub mod tsc;
pub mod util;
pub mod version;
pub mod worker;

#[allow(dead_code)]
pub(crate) fn unstable_warn_cb(feature: &str) {
  eprintln!(
    "The `--unstable` flag is deprecated, use --unstable-{feature} instead."
  );
}

pub(crate) fn unstable_exit_cb(_feature: &str, api_name: &str) {
  // TODO(bartlomieju): change to "The `--unstable-{feature}` flag must be provided.".
  eprintln!("Unstable API '{api_name}'. The --unstable flag must be provided.");
  std::process::exit(70);
}
