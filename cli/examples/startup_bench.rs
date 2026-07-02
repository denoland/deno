// Copyright 2018-2026 the Deno authors. MIT license.

//! Minimal startup-time microbenchmark.
//!
//! Boots a `MainWorker` with the real CLI snapshot, executes a trivial
//! script, then tears it down. Intended for fast iteration on startup-perf
//! work — rebuilds only this example binary, not the full `deno` CLI.
//!
//! Usage:
//!   cargo run --example startup_bench --release -- [iters]
//!
//! Env vars:
//!   DENO_STARTUP_PHASES=1   Print per-phase breakdown of one iteration
//!                           (V8 init, isolate+snapshot, ops, bootstrap JS).
//!   DENO_STARTUP_PROFILE=1  Skip the warmup so the first iter shows in profile.
//!
//! Tips for fast iteration:
//!   - Add a release-fast profile (lto=off, codegen-units=256, incremental=true)
//!     and pass `--profile release-fast`.
//!   - Use mold/lld to cut link time.
//!   - Combine with `hyperfine -w 3 ./target/release/examples/startup_bench`
//!     when measuring without rebuilding.

use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;

use deno_core::FsModuleLoader;
use deno_core::ModuleCodeString;
use deno_core::resolve_path;
use deno_resolver::npm::DenoInNpmPackageChecker;
use deno_resolver::npm::NpmResolver;
use deno_runtime::deno_fs::RealFs;
use deno_runtime::deno_permissions::Permissions;
use deno_runtime::deno_permissions::PermissionsContainer;
use deno_runtime::permissions::RuntimePermissionDescriptorParser;
use deno_runtime::worker::MainWorker;
use deno_runtime::worker::WorkerOptions;
use deno_runtime::worker::WorkerServiceOptions;

#[allow(clippy::disallowed_types, reason = "example binary")]
type Sys = sys_traits::impls::RealSys;

fn bootstrap_once() -> MainWorker {
  // Microbenchmark only — no resolver context, this is the simplest way to
  // get a `Url` to pass `bootstrap_from_options`.
  #[allow(
    clippy::disallowed_methods,
    reason = "example binary, no `initial_cwd` to thread"
  )]
  let cwd = std::env::current_dir().unwrap();
  let main_module = resolve_path("./startup_bench.js", &cwd).unwrap();
  let fs = Arc::new(RealFs);
  let permission_desc_parser =
    Arc::new(RuntimePermissionDescriptorParser::new(Sys::default()));

  let options = WorkerOptions {
    startup_snapshot: deno_snapshots::CLI_SNAPSHOT,
    residual_lazy_js_sources: deno_snapshots::RESIDUAL_LAZY_JS,
    residual_lazy_esm_sources: deno_snapshots::RESIDUAL_LAZY_ESM,
    ..Default::default()
  };

  MainWorker::bootstrap_from_options::<
    DenoInNpmPackageChecker,
    NpmResolver<Sys>,
    Sys,
  >(
    &main_module,
    WorkerServiceOptions {
      deno_rt_native_addon_loader: None,
      module_loader: Rc::new(FsModuleLoader),
      permissions: PermissionsContainer::new(
        permission_desc_parser,
        Permissions::none_without_prompt(),
      ),
      blob_store: std::sync::Arc::new(
        deno_runtime::deno_web::BlobStore::default(),
      )
        as std::sync::Arc<dyn deno_runtime::deno_web::BlobStoreTrait>,
      broadcast_channel: Default::default(),
      feature_checker: Default::default(),
      node_services: Default::default(),
      npm_process_state_provider: Default::default(),
      root_cert_store_provider: Default::default(),
      fetch_dns_resolver: Default::default(),
      shared_array_buffer_store: Default::default(),
      compiled_wasm_module_store: Default::default(),
      v8_code_cache: Default::default(),
      fs,
      bundle_provider: None,
    },
    options,
  )
}

fn one_iter() -> Duration {
  let start = Instant::now();
  let mut worker = bootstrap_once();
  // Trivial script. The bulk of the cost is bootstrap + first JS run; the
  // arithmetic itself is negligible and just keeps V8 honest.
  worker
    .execute_script("[startup_bench]", ModuleCodeString::from_static("1 + 1"))
    .expect("execute_script failed");
  let elapsed = start.elapsed();
  // Explicit drop so teardown is *not* counted in the next iteration.
  drop(worker);
  elapsed
}

fn summarize(label: &str, samples: &mut [Duration]) {
  samples.sort();
  let n = samples.len();
  let min = samples[0];
  let p50 = samples[n / 2];
  let p95 = samples[(n * 95) / 100];
  let max = samples[n - 1];
  let sum: Duration = samples.iter().sum();
  let mean = sum / n as u32;
  #[allow(
    clippy::print_stdout,
    clippy::disallowed_macros,
    reason = "example output"
  )]
  {
    println!(
      "{label:>10}  n={n:<3}  min={:>7.2?}  p50={:>7.2?}  mean={:>7.2?}  p95={:>7.2?}  max={:>7.2?}",
      min, p50, mean, p95, max,
    );
  }
}

fn main() {
  let iters: usize = std::env::args()
    .nth(1)
    .and_then(|s| s.parse().ok())
    .unwrap_or(20);

  // tokio current-thread runtime is required: MainWorker holds `!Send` state.
  let rt = tokio::runtime::Builder::new_current_thread()
    .enable_all()
    .build()
    .unwrap();

  let _guard = rt.enter();

  // First iteration pays V8 platform init + lazy globals. Run it but report
  // it separately so it doesn't skew the steady-state numbers.
  let warmup = one_iter();
  #[allow(
    clippy::print_stdout,
    clippy::disallowed_macros,
    reason = "example output"
  )]
  {
    println!(
      "    warmup  n=1    {warmup:?}  (V8 platform init + first bootstrap)"
    );
  }

  let mut samples = Vec::with_capacity(iters);
  for _ in 0..iters {
    samples.push(one_iter());
  }
  summarize("steady", &mut samples);
}
