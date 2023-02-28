// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use deno_core::error::AnyError;
use deno_core::include_js_files;
use deno_core::located_script_name;
use deno_core::op;
use deno_core::Extension;
use deno_core::JsRuntime;
use once_cell::sync::Lazy;
use std::collections::HashSet;
use std::path::Path;
use std::path::PathBuf;
use std::rc::Rc;

mod crypto;
pub mod errors;
mod idna;
mod ops;
mod package_json;
mod path;
mod polyfill;
mod resolution;
mod v8;
mod winerror;

pub use package_json::PackageJson;
pub use path::PathClean;
pub use polyfill::find_builtin_node_module;
pub use polyfill::is_builtin_node_module;
pub use polyfill::NodeModulePolyfill;
pub use polyfill::SUPPORTED_BUILTIN_NODE_MODULES;
pub use resolution::get_closest_package_json;
pub use resolution::get_package_scope_config;
pub use resolution::legacy_main_resolve;
pub use resolution::package_exports_resolve;
pub use resolution::package_imports_resolve;
pub use resolution::package_resolve;
pub use resolution::path_to_declaration_path;
pub use resolution::NodeModuleKind;
pub use resolution::NodeResolutionMode;
pub use resolution::DEFAULT_CONDITIONS;

pub trait NodePermissions {
  fn check_read(&mut self, path: &Path) -> Result<(), AnyError>;
}

pub trait RequireNpmResolver {
  fn resolve_package_folder_from_package(
    &self,
    specifier: &str,
    referrer: &Path,
    mode: NodeResolutionMode,
  ) -> Result<PathBuf, AnyError>;

  fn resolve_package_folder_from_path(
    &self,
    path: &Path,
  ) -> Result<PathBuf, AnyError>;

  fn in_npm_package(&self, path: &Path) -> bool;

  fn ensure_read_permission(
    &self,
    permissions: &mut dyn NodePermissions,
    path: &Path,
  ) -> Result<(), AnyError>;
}

pub static NODE_GLOBAL_THIS_NAME: Lazy<String> = Lazy::new(|| {
  let now = std::time::SystemTime::now();
  let seconds = now
    .duration_since(std::time::SystemTime::UNIX_EPOCH)
    .unwrap()
    .as_secs();
  // use a changing variable name to make it hard to depend on this
  format!("__DENO_NODE_GLOBAL_THIS_{seconds}__")
});

pub static NODE_ENV_VAR_ALLOWLIST: Lazy<HashSet<String>> = Lazy::new(|| {
  // The full list of environment variables supported by Node.js is available
  // at https://nodejs.org/api/cli.html#environment-variables
  let mut set = HashSet::new();
  set.insert("NODE_DEBUG".to_string());
  set.insert("NODE_OPTIONS".to_string());
  set
});

#[op]
fn op_node_build_os() -> String {
  std::env::var("TARGET")
    .unwrap()
    .split('-')
    .nth(2)
    .unwrap()
    .to_string()
}

pub fn init_polyfill() -> Extension {
  let esm_files = include_js_files!(
    dir "polyfills",
    "_core.ts",
    "_events.mjs",
    "_fs/_fs_access.ts",
    "_fs/_fs_appendFile.ts",
    "_fs/_fs_chmod.ts",
    "_fs/_fs_chown.ts",
    "_fs/_fs_close.ts",
    "_fs/_fs_common.ts",
    "_fs/_fs_constants.ts",
    "_fs/_fs_copy.ts",
    "_fs/_fs_dir.ts",
    "_fs/_fs_dirent.ts",
    "_fs/_fs_exists.ts",
    "_fs/_fs_fdatasync.ts",
    "_fs/_fs_fstat.ts",
    "_fs/_fs_fsync.ts",
    "_fs/_fs_ftruncate.ts",
    "_fs/_fs_futimes.ts",
    "_fs/_fs_link.ts",
    "_fs/_fs_lstat.ts",
    "_fs/_fs_mkdir.ts",
    "_fs/_fs_mkdtemp.ts",
    "_fs/_fs_open.ts",
    "_fs/_fs_opendir.ts",
    "_fs/_fs_read.ts",
    "_fs/_fs_readdir.ts",
    "_fs/_fs_readFile.ts",
    "_fs/_fs_readlink.ts",
    "_fs/_fs_realpath.ts",
    "_fs/_fs_rename.ts",
    "_fs/_fs_rm.ts",
    "_fs/_fs_rmdir.ts",
    "_fs/_fs_stat.ts",
    "_fs/_fs_symlink.ts",
    "_fs/_fs_truncate.ts",
    "_fs/_fs_unlink.ts",
    "_fs/_fs_utimes.ts",
    "_fs/_fs_watch.ts",
    "_fs/_fs_write.mjs",
    "_fs/_fs_writeFile.ts",
    "_fs/_fs_writev.mjs",
    "_http_agent.mjs",
    "_http_common.ts",
    "_http_outgoing.ts",
    "_next_tick.ts",
    "_pako.mjs",
    "_process/exiting.ts",
    "_process/process.ts",
    "_process/streams.mjs",
    "_readline.mjs",
    "_stream.mjs",
    "_tls_common.ts",
    "_tls_wrap.ts",
    "_util/_util_callbackify.ts",
    "_util/asserts.ts",
    "_util/async.ts",
    "_util/os.ts",
    "_util/std_asserts.ts",
    "_util/std_fmt_colors.ts",
    "_util/std_testing_diff.ts",
    "_utils.ts",
    "_zlib_binding.mjs",
    "_zlib.mjs",
    "assert.ts",
    "assert/strict.ts",
    "assertion_error.ts",
    "async_hooks.ts",
    "buffer.ts",
    "child_process.ts",
    "cluster.ts",
    "console.ts",
    "constants.ts",
    "crypto.ts",
    "dgram.ts",
    "diagnostics_channel.ts",
    "dns.ts",
    "dns/promises.ts",
    "domain.ts",
    "events.ts",
    "fs.ts",
    "fs/promises.ts",
    "global.ts",
    "http.ts",
    "http2.ts",
    "https.ts",
    "inspector.ts",
    "internal_binding/_libuv_winerror.ts",
    "internal_binding/_listen.ts",
    "internal_binding/_node.ts",
    "internal_binding/_timingSafeEqual.ts",
    "internal_binding/_utils.ts",
    "internal_binding/ares.ts",
    "internal_binding/async_wrap.ts",
    "internal_binding/buffer.ts",
    "internal_binding/cares_wrap.ts",
    "internal_binding/config.ts",
    "internal_binding/connection_wrap.ts",
    "internal_binding/constants.ts",
    "internal_binding/contextify.ts",
    "internal_binding/credentials.ts",
    "internal_binding/crypto.ts",
    "internal_binding/errors.ts",
    "internal_binding/fs_dir.ts",
    "internal_binding/fs_event_wrap.ts",
    "internal_binding/fs.ts",
    "internal_binding/handle_wrap.ts",
    "internal_binding/heap_utils.ts",
    "internal_binding/http_parser.ts",
    "internal_binding/icu.ts",
    "internal_binding/inspector.ts",
    "internal_binding/js_stream.ts",
    "internal_binding/messaging.ts",
    "internal_binding/mod.ts",
    "internal_binding/module_wrap.ts",
    "internal_binding/native_module.ts",
    "internal_binding/natives.ts",
    "internal_binding/node_file.ts",
    "internal_binding/node_options.ts",
    "internal_binding/options.ts",
    "internal_binding/os.ts",
    "internal_binding/performance.ts",
    "internal_binding/pipe_wrap.ts",
    "internal_binding/process_methods.ts",
    "internal_binding/report.ts",
    "internal_binding/serdes.ts",
    "internal_binding/signal_wrap.ts",
    "internal_binding/spawn_sync.ts",
    "internal_binding/stream_wrap.ts",
    "internal_binding/string_decoder.ts",
    "internal_binding/symbols.ts",
    "internal_binding/task_queue.ts",
    "internal_binding/tcp_wrap.ts",
    "internal_binding/timers.ts",
    "internal_binding/tls_wrap.ts",
    "internal_binding/trace_events.ts",
    "internal_binding/tty_wrap.ts",
    "internal_binding/types.ts",
    "internal_binding/udp_wrap.ts",
    "internal_binding/url.ts",
    "internal_binding/util.ts",
    "internal_binding/uv.ts",
    "internal_binding/v8.ts",
    "internal_binding/worker.ts",
    "internal_binding/zlib.ts",
    "internal/assert.mjs",
    "internal/async_hooks.ts",
    "internal/blob.mjs",
    "internal/buffer.mjs",
    "internal/child_process.ts",
    "internal/cli_table.ts",
    "internal/console/constructor.mjs",
    "internal/constants.ts",
    "internal/crypto/_hex.ts",
    "internal/crypto/_keys.ts",
    "internal/crypto/_randomBytes.ts",
    "internal/crypto/_randomFill.ts",
    "internal/crypto/_randomInt.ts",
    "internal/crypto/certificate.ts",
    "internal/crypto/cipher.ts",
    "internal/crypto/constants.ts",
    "internal/crypto/diffiehellman.ts",
    "internal/crypto/hash.ts",
    "internal/crypto/hkdf.ts",
    "internal/crypto/keygen.ts",
    "internal/crypto/keys.ts",
    "internal/crypto/pbkdf2.ts",
    "internal/crypto/random.ts",
    "internal/crypto/scrypt.ts",
    "internal/crypto/sig.ts",
    "internal/crypto/types.ts",
    "internal/crypto/util.ts",
    "internal/crypto/x509.ts",
    "internal/dgram.ts",
    "internal/dns/promises.ts",
    "internal/dns/utils.ts",
    "internal/dtrace.ts",
    "internal/error_codes.ts",
    "internal/errors.ts",
    "internal/event_target.mjs",
    "internal/fixed_queue.ts",
    "internal/freelist.ts",
    "internal/fs/streams.mjs",
    "internal/fs/utils.mjs",
    "internal/hide_stack_frames.ts",
    "internal/http.ts",
    "internal/idna.ts",
    "internal/net.ts",
    "internal/normalize_encoding.mjs",
    "internal/options.ts",
    "internal/primordials.mjs",
    "internal/process/per_thread.mjs",
    "internal/querystring.ts",
    "internal/readline/callbacks.mjs",
    "internal/readline/emitKeypressEvents.mjs",
    "internal/readline/interface.mjs",
    "internal/readline/promises.mjs",
    "internal/readline/symbols.mjs",
    "internal/readline/utils.mjs",
    "internal/stream_base_commons.ts",
    "internal/streams/add-abort-signal.mjs",
    "internal/streams/buffer_list.mjs",
    "internal/streams/destroy.mjs",
    "internal/streams/duplex.mjs",
    "internal/streams/end-of-stream.mjs",
    "internal/streams/lazy_transform.mjs",
    "internal/streams/legacy.mjs",
    "internal/streams/passthrough.mjs",
    "internal/streams/readable.mjs",
    "internal/streams/state.mjs",
    "internal/streams/transform.mjs",
    "internal/streams/utils.mjs",
    "internal/streams/writable.mjs",
    "internal/test/binding.ts",
    "internal/timers.mjs",
    "internal/url.ts",
    "internal/util.mjs",
    "internal/util/comparisons.ts",
    "internal/util/debuglog.ts",
    "internal/util/inspect.mjs",
    "internal/util/types.ts",
    "internal/validators.mjs",
    "module_all.ts",
    "module_esm.ts",
    "module.js",
    "net.ts",
    "os.ts",
    "path.ts",
    "path/_constants.ts",
    "path/_interface.ts",
    "path/_util.ts",
    "path/common.ts",
    "path/glob.ts",
    "path/mod.ts",
    "path/posix.ts",
    "path/separator.ts",
    "path/win32.ts",
    "perf_hooks.ts",
    "process.ts",
    "punycode.ts",
    "querystring.ts",
    "readline.ts",
    "readline/promises.ts",
    "repl.ts",
    "stream.ts",
    "stream/consumers.mjs",
    "stream/promises.mjs",
    "stream/web.ts",
    "string_decoder.ts",
    "sys.ts",
    "timers.ts",
    "timers/promises.ts",
    "tls.ts",
    "tty.ts",
    "upstream_modules.ts",
    "url.ts",
    "util.ts",
    "util/types.ts",
    "v8.ts",
    "vm.ts",
    "wasi.ts",
    "worker_threads.ts",
    "zlib.ts",
  );

  Extension::builder(env!("CARGO_PKG_NAME"))
    .esm(esm_files)
    .esm_entry_point("internal:deno_node/polyfills/module_all.ts")
    .ops(vec![
      crypto::op_node_create_hash::decl(),
      crypto::op_node_hash_update::decl(),
      crypto::op_node_hash_digest::decl(),
      crypto::op_node_hash_clone::decl(),
      crypto::op_node_private_encrypt::decl(),
      crypto::op_node_private_decrypt::decl(),
      crypto::op_node_public_encrypt::decl(),
      winerror::op_node_sys_to_uv_error::decl(),
      v8::op_v8_cached_data_version_tag::decl(),
      v8::op_v8_get_heap_statistics::decl(),
      idna::op_node_idna_domain_to_ascii::decl(),
      idna::op_node_idna_domain_to_unicode::decl(),
      idna::op_node_idna_punycode_decode::decl(),
      idna::op_node_idna_punycode_encode::decl(),
      op_node_build_os::decl(),
    ])
    .build()
}

pub fn init<P: NodePermissions + 'static>(
  maybe_npm_resolver: Option<Rc<dyn RequireNpmResolver>>,
) -> Extension {
  Extension::builder("deno_node_loading")
    .esm(include_js_files!(
      "01_node.js",
      "02_require.js",
      "module_es_shim.js",
    ))
    .ops(vec![
      ops::op_require_init_paths::decl(),
      ops::op_require_node_module_paths::decl::<P>(),
      ops::op_require_proxy_path::decl(),
      ops::op_require_is_deno_dir_package::decl(),
      ops::op_require_resolve_deno_dir::decl(),
      ops::op_require_is_request_relative::decl(),
      ops::op_require_resolve_lookup_paths::decl(),
      ops::op_require_try_self_parent_path::decl::<P>(),
      ops::op_require_try_self::decl::<P>(),
      ops::op_require_real_path::decl::<P>(),
      ops::op_require_path_is_absolute::decl(),
      ops::op_require_path_dirname::decl(),
      ops::op_require_stat::decl::<P>(),
      ops::op_require_path_resolve::decl(),
      ops::op_require_path_basename::decl(),
      ops::op_require_read_file::decl::<P>(),
      ops::op_require_as_file_path::decl(),
      ops::op_require_resolve_exports::decl::<P>(),
      ops::op_require_read_closest_package_json::decl::<P>(),
      ops::op_require_read_package_scope::decl::<P>(),
      ops::op_require_package_imports_resolve::decl::<P>(),
      ops::op_require_break_on_next_statement::decl(),
    ])
    .state(move |state| {
      if let Some(npm_resolver) = maybe_npm_resolver.clone() {
        state.put(npm_resolver);
      }
      Ok(())
    })
    .build()
}

pub async fn initialize_runtime(
  js_runtime: &mut JsRuntime,
  uses_local_node_modules_dir: bool,
) -> Result<(), AnyError> {
  let source_code = &format!(
    r#"(async function loadBuiltinNodeModules(nodeGlobalThisName, usesLocalNodeModulesDir) {{
      Deno[Deno.internal].node.initialize(Deno[Deno.internal].nodeModuleAll, nodeGlobalThisName);
      if (usesLocalNodeModulesDir) {{
        Deno[Deno.internal].require.setUsesLocalNodeModulesDir();
      }}
    }})('{}', {});"#,
    NODE_GLOBAL_THIS_NAME.as_str(),
    uses_local_node_modules_dir,
  );

  let value =
    js_runtime.execute_script(&located_script_name!(), source_code)?;
  js_runtime.resolve_value(value).await?;
  Ok(())
}

pub fn load_cjs_module(
  js_runtime: &mut JsRuntime,
  module: &str,
  main: bool,
  inspect_brk: bool,
) -> Result<(), AnyError> {
  fn escape_for_single_quote_string(text: &str) -> String {
    text.replace('\\', r"\\").replace('\'', r"\'")
  }

  let source_code = &format!(
    r#"(function loadCjsModule(module, inspectBrk) {{
      if (inspectBrk) {{
        Deno[Deno.internal].require.setInspectBrk();
      }}
      Deno[Deno.internal].require.Module._load(module, null, {main});
    }})('{module}', {inspect_brk});"#,
    main = main,
    module = escape_for_single_quote_string(module),
    inspect_brk = inspect_brk,
  );

  js_runtime.execute_script(&located_script_name!(), source_code)?;
  Ok(())
}

pub async fn initialize_binary_command(
  js_runtime: &mut JsRuntime,
  binary_name: &str,
) -> Result<(), AnyError> {
  // overwrite what's done in deno_std in order to set the binary arg name
  let source_code = &format!(
    r#"(async function initializeBinaryCommand(binaryName) {{
      const process = Deno[Deno.internal].node.globalThis.process;
      Object.defineProperty(process.argv, "0", {{
        get: () => binaryName,
      }});
    }})('{binary_name}');"#,
  );

  let value =
    js_runtime.execute_script(&located_script_name!(), source_code)?;
  js_runtime.resolve_value(value).await?;
  Ok(())
}
