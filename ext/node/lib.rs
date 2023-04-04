// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use deno_core::error::AnyError;
use deno_core::located_script_name;
use deno_core::op;
use deno_core::JsRuntime;
use once_cell::sync::Lazy;
use std::collections::HashSet;
use std::io;
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
mod zlib;

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

pub trait NodeEnv {
  type P: NodePermissions;
  type Fs: NodeFs;
}

pub trait NodePermissions {
  fn check_read(&mut self, path: &Path) -> Result<(), AnyError>;
}

pub trait NodeFs {
  fn current_dir() -> io::Result<PathBuf>;
  fn metadata<P: AsRef<Path>>(path: P) -> io::Result<std::fs::Metadata>;
  fn read_to_string<P: AsRef<Path>>(path: P) -> io::Result<String>;
}

pub struct RealFs;
impl NodeFs for RealFs {
  fn current_dir() -> io::Result<PathBuf> {
    #[allow(clippy::disallowed_methods)]
    std::env::current_dir()
  }

  fn metadata<P: AsRef<Path>>(path: P) -> io::Result<std::fs::Metadata> {
    #[allow(clippy::disallowed_methods)]
    std::fs::metadata(path)
  }

  fn read_to_string<P: AsRef<Path>>(path: P) -> io::Result<String> {
    #[allow(clippy::disallowed_methods)]
    std::fs::read_to_string(path)
  }
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

deno_core::extension!(deno_node,
  deps = [ deno_io, deno_fs ],
  parameters = [Env: NodeEnv],
  ops = [
    crypto::op_node_create_decipheriv,
    crypto::op_node_cipheriv_encrypt,
    crypto::op_node_cipheriv_final,
    crypto::op_node_create_cipheriv,
    crypto::op_node_create_hash,
    crypto::op_node_decipheriv_decrypt,
    crypto::op_node_decipheriv_final,
    crypto::op_node_hash_update,
    crypto::op_node_hash_update_str,
    crypto::op_node_hash_digest,
    crypto::op_node_hash_digest_hex,
    crypto::op_node_hash_clone,
    crypto::op_node_private_encrypt,
    crypto::op_node_private_decrypt,
    crypto::op_node_public_encrypt,
    crypto::op_node_check_prime,
    crypto::op_node_check_prime_async,
    crypto::op_node_check_prime_bytes,
    crypto::op_node_check_prime_bytes_async,
    crypto::op_node_pbkdf2,
    crypto::op_node_pbkdf2_async,
    crypto::op_node_sign,
    winerror::op_node_sys_to_uv_error,
    v8::op_v8_cached_data_version_tag,
    v8::op_v8_get_heap_statistics,
    idna::op_node_idna_domain_to_ascii,
    idna::op_node_idna_domain_to_unicode,
    idna::op_node_idna_punycode_decode,
    idna::op_node_idna_punycode_encode,
    zlib::op_zlib_new,
    zlib::op_zlib_close,
    zlib::op_zlib_close_if_pending,
    zlib::op_zlib_write,
    zlib::op_zlib_write_async,
    zlib::op_zlib_init,
    zlib::op_zlib_reset,
    op_node_build_os,

    ops::op_require_init_paths,
    ops::op_require_node_module_paths<Env>,
    ops::op_require_proxy_path,
    ops::op_require_is_deno_dir_package,
    ops::op_require_resolve_deno_dir,
    ops::op_require_is_request_relative,
    ops::op_require_resolve_lookup_paths,
    ops::op_require_try_self_parent_path<Env>,
    ops::op_require_try_self<Env>,
    ops::op_require_real_path<Env>,
    ops::op_require_path_is_absolute,
    ops::op_require_path_dirname,
    ops::op_require_stat<Env>,
    ops::op_require_path_resolve,
    ops::op_require_path_basename,
    ops::op_require_read_file<Env>,
    ops::op_require_as_file_path,
    ops::op_require_resolve_exports<Env>,
    ops::op_require_read_closest_package_json<Env>,
    ops::op_require_read_package_scope<Env>,
    ops::op_require_package_imports_resolve<Env>,
    ops::op_require_break_on_next_statement,
  ],
  esm_entry_point = "ext:deno_node/02_init.js",
  esm = [
    dir "polyfills",
    "00_globals.js",
    "01_require.js",
    "02_init.js",
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
    "internal_binding/connection_wrap.ts",
    "internal_binding/constants.ts",
    "internal_binding/crypto.ts",
    "internal_binding/handle_wrap.ts",
    "internal_binding/mod.ts",
    "internal_binding/node_file.ts",
    "internal_binding/node_options.ts",
    "internal_binding/pipe_wrap.ts",
    "internal_binding/stream_wrap.ts",
    "internal_binding/string_decoder.ts",
    "internal_binding/symbols.ts",
    "internal_binding/tcp_wrap.ts",
    "internal_binding/types.ts",
    "internal_binding/udp_wrap.ts",
    "internal_binding/util.ts",
    "internal_binding/uv.ts",
    "internal/assert.mjs",
    "internal/async_hooks.ts",
    "internal/buffer.mjs",
    "internal/child_process.ts",
    "internal/cli_table.ts",
    "internal/console/constructor.mjs",
    "internal/constants.ts",
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
    "url.ts",
    "util.ts",
    "util/types.ts",
    "v8.ts",
    "vm.ts",
    "wasi.ts",
    "worker_threads.ts",
    "zlib.ts",
  ],
  options = {
    maybe_npm_resolver: Option<Rc<dyn RequireNpmResolver>>,
  },
  state = |state, options| {
    if let Some(npm_resolver) = options.maybe_npm_resolver {
      state.put(npm_resolver);
    }
  },
);

pub fn initialize_runtime(
  js_runtime: &mut JsRuntime,
  uses_local_node_modules_dir: bool,
  maybe_binary_command_name: Option<String>,
) -> Result<(), AnyError> {
  let argv0 = if let Some(binary_command_name) = maybe_binary_command_name {
    format!("\"{}\"", binary_command_name)
  } else {
    "undefined".to_string()
  };
  let source_code = format!(
    r#"(function loadBuiltinNodeModules(nodeGlobalThisName, usesLocalNodeModulesDir, argv0) {{
      Deno[Deno.internal].node.initialize(
        nodeGlobalThisName,
        usesLocalNodeModulesDir,
        argv0
      );
    }})('{}', {}, {});"#,
    NODE_GLOBAL_THIS_NAME.as_str(),
    uses_local_node_modules_dir,
    argv0
  );

  js_runtime.execute_script(located_script_name!(), source_code.into())?;
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

  let source_code = format!(
    r#"(function loadCjsModule(moduleName, isMain, inspectBrk) {{
      Deno[Deno.internal].node.loadCjsModule(moduleName, isMain, inspectBrk);
    }})('{module}', {main}, {inspect_brk});"#,
    main = main,
    module = escape_for_single_quote_string(module),
    inspect_brk = inspect_brk,
  )
  .into();

  js_runtime.execute_script(located_script_name!(), source_code)?;
  Ok(())
}
