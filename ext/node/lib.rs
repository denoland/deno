// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

#![deny(clippy::print_stderr)]
#![deny(clippy::print_stdout)]

use std::collections::HashSet;
use std::path::Path;
use std::path::PathBuf;

use deno_core::error::AnyError;
use deno_core::located_script_name;
use deno_core::op2;
use deno_core::url::Url;
#[allow(unused_imports)]
use deno_core::v8;
use deno_core::v8::ExternalReference;
use deno_core::JsRuntime;
use deno_core::ModuleSpecifier;
use deno_core::OpState;
use deno_fs::sync::MaybeSend;
use deno_fs::sync::MaybeSync;
use once_cell::sync::Lazy;

extern crate libz_sys as zlib;

pub mod analyze;
pub mod errors;
mod global;
mod ops;
mod package_json;
mod path;
mod polyfill;
mod resolution;

pub use deno_package_json::PackageJson;
pub use ops::ipc::ChildPipeFd;
pub use ops::ipc::IpcJsonStreamResource;
use ops::vm;
pub use ops::vm::create_v8_context;
pub use ops::vm::init_global_template;
pub use ops::vm::ContextInitMode;
pub use ops::vm::VM_CONTEXT_INDEX;
pub use package_json::load_pkg_json;
pub use package_json::PackageJsonThreadLocalCache;
pub use path::PathClean;
pub use polyfill::is_builtin_node_module;
pub use polyfill::SUPPORTED_BUILTIN_NODE_MODULES;
pub use polyfill::SUPPORTED_BUILTIN_NODE_MODULES_WITH_PREFIX;
pub use resolution::NodeModuleKind;
pub use resolution::NodeResolution;
pub use resolution::NodeResolutionMode;
pub use resolution::NodeResolver;
use resolution::NodeResolverRc;

use crate::global::global_object_middleware;
use crate::global::global_template_middleware;

pub trait NodePermissions {
  fn check_net_url(
    &mut self,
    url: &Url,
    api_name: &str,
  ) -> Result<(), AnyError>;
  #[inline(always)]
  fn check_read(&mut self, path: &Path) -> Result<(), AnyError> {
    self.check_read_with_api_name(path, None)
  }
  fn check_read_with_api_name(
    &mut self,
    path: &Path,
    api_name: Option<&str>,
  ) -> Result<(), AnyError>;
  fn check_sys(&mut self, kind: &str, api_name: &str) -> Result<(), AnyError>;
  fn check_write_with_api_name(
    &mut self,
    path: &Path,
    api_name: Option<&str>,
  ) -> Result<(), AnyError>;
}

pub struct AllowAllNodePermissions;

impl NodePermissions for AllowAllNodePermissions {
  fn check_net_url(
    &mut self,
    _url: &Url,
    _api_name: &str,
  ) -> Result<(), AnyError> {
    Ok(())
  }
  fn check_read_with_api_name(
    &mut self,
    _path: &Path,
    _api_name: Option<&str>,
  ) -> Result<(), AnyError> {
    Ok(())
  }
  fn check_write_with_api_name(
    &mut self,
    _path: &Path,
    _api_name: Option<&str>,
  ) -> Result<(), AnyError> {
    Ok(())
  }
  fn check_sys(
    &mut self,
    _kind: &str,
    _api_name: &str,
  ) -> Result<(), AnyError> {
    Ok(())
  }
}

impl NodePermissions for deno_permissions::PermissionsContainer {
  #[inline(always)]
  fn check_net_url(
    &mut self,
    url: &Url,
    api_name: &str,
  ) -> Result<(), AnyError> {
    deno_permissions::PermissionsContainer::check_net_url(self, url, api_name)
  }

  #[inline(always)]
  fn check_read_with_api_name(
    &mut self,
    path: &Path,
    api_name: Option<&str>,
  ) -> Result<(), AnyError> {
    deno_permissions::PermissionsContainer::check_read_with_api_name(
      self, path, api_name,
    )
  }

  #[inline(always)]
  fn check_write_with_api_name(
    &mut self,
    path: &Path,
    api_name: Option<&str>,
  ) -> Result<(), AnyError> {
    deno_permissions::PermissionsContainer::check_write_with_api_name(
      self, path, api_name,
    )
  }

  fn check_sys(&mut self, kind: &str, api_name: &str) -> Result<(), AnyError> {
    deno_permissions::PermissionsContainer::check_sys(self, kind, api_name)
  }
}

#[allow(clippy::disallowed_types)]
pub type NpmResolverRc = deno_fs::sync::MaybeArc<dyn NpmResolver>;

pub trait NpmResolver: std::fmt::Debug + MaybeSend + MaybeSync {
  /// Gets a string containing the serialized npm state of the process.
  ///
  /// This will be set on the `DENO_DONT_USE_INTERNAL_NODE_COMPAT_STATE` environment
  /// variable when doing a `child_process.fork`. The implementor can then check this environment
  /// variable on startup to repopulate the internal npm state.
  fn get_npm_process_state(&self) -> String {
    // This method is only used in the CLI.
    String::new()
  }

  /// Resolves an npm package folder path from an npm package referrer.
  fn resolve_package_folder_from_package(
    &self,
    specifier: &str,
    referrer: &ModuleSpecifier,
  ) -> Result<PathBuf, errors::PackageFolderResolveError>;

  fn in_npm_package(&self, specifier: &ModuleSpecifier) -> bool;

  fn in_npm_package_at_dir_path(&self, path: &Path) -> bool {
    let specifier =
      match ModuleSpecifier::from_directory_path(path.to_path_buf().clean()) {
        Ok(p) => p,
        Err(_) => return false,
      };
    self.in_npm_package(&specifier)
  }

  fn in_npm_package_at_file_path(&self, path: &Path) -> bool {
    let specifier =
      match ModuleSpecifier::from_file_path(path.to_path_buf().clean()) {
        Ok(p) => p,
        Err(_) => return false,
      };
    self.in_npm_package(&specifier)
  }

  fn ensure_read_permission(
    &self,
    permissions: &mut dyn NodePermissions,
    path: &Path,
  ) -> Result<(), AnyError>;
}

pub static NODE_ENV_VAR_ALLOWLIST: Lazy<HashSet<String>> = Lazy::new(|| {
  // The full list of environment variables supported by Node.js is available
  // at https://nodejs.org/api/cli.html#environment-variables
  let mut set = HashSet::new();
  set.insert("NODE_DEBUG".to_string());
  set.insert("NODE_OPTIONS".to_string());
  set
});

#[op2]
#[string]
fn op_node_build_os() -> String {
  env!("TARGET").split('-').nth(2).unwrap().to_string()
}

#[op2(fast)]
fn op_node_is_promise_rejected(value: v8::Local<v8::Value>) -> bool {
  let Ok(promise) = v8::Local::<v8::Promise>::try_from(value) else {
    return false;
  };

  promise.state() == v8::PromiseState::Rejected
}

#[op2]
#[string]
fn op_npm_process_state(state: &mut OpState) -> Result<String, AnyError> {
  let npm_resolver = state.borrow_mut::<NpmResolverRc>();
  Ok(npm_resolver.get_npm_process_state())
}

deno_core::extension!(deno_node,
  deps = [ deno_io, deno_fs ],
  parameters = [P: NodePermissions],
  ops = [
    ops::blocklist::op_socket_address_parse,
    ops::blocklist::op_socket_address_get_serialization,

    ops::blocklist::op_blocklist_new,
    ops::blocklist::op_blocklist_add_address,
    ops::blocklist::op_blocklist_add_range,
    ops::blocklist::op_blocklist_add_subnet,
    ops::blocklist::op_blocklist_check,

    ops::buffer::op_is_ascii,
    ops::buffer::op_is_utf8,
    ops::crypto::op_node_create_decipheriv,
    ops::crypto::op_node_cipheriv_encrypt,
    ops::crypto::op_node_cipheriv_final,
    ops::crypto::op_node_cipheriv_set_aad,
    ops::crypto::op_node_decipheriv_set_aad,
    ops::crypto::op_node_create_cipheriv,
    ops::crypto::op_node_create_hash,
    ops::crypto::op_node_get_hashes,
    ops::crypto::op_node_decipheriv_decrypt,
    ops::crypto::op_node_decipheriv_final,
    ops::crypto::op_node_hash_update,
    ops::crypto::op_node_hash_update_str,
    ops::crypto::op_node_hash_digest,
    ops::crypto::op_node_hash_digest_hex,
    ops::crypto::op_node_hash_clone,
    ops::crypto::op_node_private_encrypt,
    ops::crypto::op_node_private_decrypt,
    ops::crypto::op_node_public_encrypt,
    ops::crypto::op_node_check_prime,
    ops::crypto::op_node_check_prime_async,
    ops::crypto::op_node_check_prime_bytes,
    ops::crypto::op_node_check_prime_bytes_async,
    ops::crypto::op_node_gen_prime,
    ops::crypto::op_node_gen_prime_async,
    ops::crypto::op_node_pbkdf2,
    ops::crypto::op_node_pbkdf2_async,
    ops::crypto::op_node_hkdf,
    ops::crypto::op_node_hkdf_async,
    ops::crypto::op_node_generate_secret,
    ops::crypto::op_node_generate_secret_async,
    ops::crypto::op_node_sign,
    ops::crypto::op_node_generate_rsa,
    ops::crypto::op_node_generate_rsa_async,
    ops::crypto::op_node_dsa_generate,
    ops::crypto::op_node_dsa_generate_async,
    ops::crypto::op_node_ec_generate,
    ops::crypto::op_node_ec_generate_async,
    ops::crypto::op_node_ed25519_generate,
    ops::crypto::op_node_ed25519_generate_async,
    ops::crypto::op_node_x25519_generate,
    ops::crypto::op_node_x25519_generate_async,
    ops::crypto::op_node_dh_generate_group,
    ops::crypto::op_node_dh_generate_group_async,
    ops::crypto::op_node_dh_generate,
    ops::crypto::op_node_dh_generate2,
    ops::crypto::op_node_dh_compute_secret,
    ops::crypto::op_node_dh_generate_async,
    ops::crypto::op_node_verify,
    ops::crypto::op_node_random_int,
    ops::crypto::op_node_scrypt_sync,
    ops::crypto::op_node_scrypt_async,
    ops::crypto::op_node_ecdh_generate_keys,
    ops::crypto::op_node_ecdh_compute_secret,
    ops::crypto::op_node_ecdh_compute_public_key,
    ops::crypto::op_node_ecdh_encode_pubkey,
    ops::crypto::op_node_export_rsa_public_pem,
    ops::crypto::op_node_export_rsa_spki_der,
    ops::crypto::x509::op_node_x509_parse,
    ops::crypto::x509::op_node_x509_ca,
    ops::crypto::x509::op_node_x509_check_email,
    ops::crypto::x509::op_node_x509_fingerprint,
    ops::crypto::x509::op_node_x509_fingerprint256,
    ops::crypto::x509::op_node_x509_fingerprint512,
    ops::crypto::x509::op_node_x509_get_issuer,
    ops::crypto::x509::op_node_x509_get_subject,
    ops::crypto::x509::op_node_x509_get_valid_from,
    ops::crypto::x509::op_node_x509_get_valid_to,
    ops::crypto::x509::op_node_x509_get_serial_number,
    ops::crypto::x509::op_node_x509_key_usage,
    ops::fs::op_node_fs_exists_sync<P>,
    ops::fs::op_node_fs_exists<P>,
    ops::fs::op_node_cp_sync<P>,
    ops::fs::op_node_cp<P>,
    ops::fs::op_node_lchown_sync<P>,
    ops::fs::op_node_lchown<P>,
    ops::fs::op_node_lutimes_sync<P>,
    ops::fs::op_node_lutimes<P>,
    ops::fs::op_node_statfs<P>,
    ops::winerror::op_node_sys_to_uv_error,
    ops::v8::op_v8_cached_data_version_tag,
    ops::v8::op_v8_get_heap_statistics,
    ops::vm::op_vm_create_script,
    ops::vm::op_vm_create_context,
    ops::vm::op_vm_script_run_in_context,
    ops::vm::op_vm_script_run_in_this_context,
    ops::vm::op_vm_is_context,
    ops::idna::op_node_idna_domain_to_ascii,
    ops::idna::op_node_idna_domain_to_unicode,
    ops::idna::op_node_idna_punycode_to_ascii,
    ops::idna::op_node_idna_punycode_to_unicode,
    ops::idna::op_node_idna_punycode_decode,
    ops::idna::op_node_idna_punycode_encode,
    ops::zlib::op_zlib_new,
    ops::zlib::op_zlib_close,
    ops::zlib::op_zlib_close_if_pending,
    ops::zlib::op_zlib_write,
    ops::zlib::op_zlib_init,
    ops::zlib::op_zlib_reset,
    ops::zlib::brotli::op_brotli_compress,
    ops::zlib::brotli::op_brotli_compress_async,
    ops::zlib::brotli::op_create_brotli_compress,
    ops::zlib::brotli::op_brotli_compress_stream,
    ops::zlib::brotli::op_brotli_compress_stream_end,
    ops::zlib::brotli::op_brotli_decompress,
    ops::zlib::brotli::op_brotli_decompress_async,
    ops::zlib::brotli::op_create_brotli_decompress,
    ops::zlib::brotli::op_brotli_decompress_stream,
    ops::zlib::brotli::op_brotli_decompress_stream_end,
    ops::http::op_node_http_request<P>,
    ops::http2::op_http2_connect,
    ops::http2::op_http2_poll_client_connection,
    ops::http2::op_http2_client_request,
    ops::http2::op_http2_client_get_response,
    ops::http2::op_http2_client_get_response_body_chunk,
    ops::http2::op_http2_client_send_data,
    ops::http2::op_http2_client_reset_stream,
    ops::http2::op_http2_client_send_trailers,
    ops::http2::op_http2_client_get_response_trailers,
    ops::http2::op_http2_accept,
    ops::http2::op_http2_listen,
    ops::http2::op_http2_send_response,
    ops::os::op_node_os_get_priority<P>,
    ops::os::op_node_os_set_priority<P>,
    ops::os::op_node_os_user_info<P>,
    ops::os::op_geteuid<P>,
    ops::os::op_getegid<P>,
    ops::os::op_cpus<P>,
    ops::os::op_homedir<P>,
    op_node_build_os,
    op_node_is_promise_rejected,
    op_npm_process_state,
    ops::require::op_require_init_paths,
    ops::require::op_require_node_module_paths<P>,
    ops::require::op_require_proxy_path,
    ops::require::op_require_is_deno_dir_package,
    ops::require::op_require_resolve_deno_dir,
    ops::require::op_require_is_request_relative,
    ops::require::op_require_resolve_lookup_paths,
    ops::require::op_require_try_self_parent_path<P>,
    ops::require::op_require_try_self<P>,
    ops::require::op_require_real_path<P>,
    ops::require::op_require_path_is_absolute,
    ops::require::op_require_path_dirname,
    ops::require::op_require_stat<P>,
    ops::require::op_require_path_resolve,
    ops::require::op_require_path_basename,
    ops::require::op_require_read_file<P>,
    ops::require::op_require_as_file_path,
    ops::require::op_require_resolve_exports<P>,
    ops::require::op_require_read_closest_package_json<P>,
    ops::require::op_require_read_package_scope<P>,
    ops::require::op_require_package_imports_resolve<P>,
    ops::require::op_require_break_on_next_statement,
    ops::util::op_node_guess_handle_type,
    ops::worker_threads::op_worker_threads_filename<P>,
    ops::crypto::op_node_create_private_key,
    ops::crypto::op_node_create_public_key,
    ops::ipc::op_node_child_ipc_pipe,
    ops::ipc::op_node_ipc_write,
    ops::ipc::op_node_ipc_read,
    ops::process::op_node_process_kill,
    ops::process::op_process_abort,
  ],
  esm_entry_point = "ext:deno_node/02_init.js",
  esm = [
    dir "polyfills",
    "00_globals.js",
    "02_init.js",
    "_brotli.js",
    "_events.mjs",
    "_fs/_fs_access.ts",
    "_fs/_fs_appendFile.ts",
    "_fs/_fs_chmod.ts",
    "_fs/_fs_chown.ts",
    "_fs/_fs_close.ts",
    "_fs/_fs_common.ts",
    "_fs/_fs_constants.ts",
    "_fs/_fs_copy.ts",
    "_fs/_fs_cp.js",
    "_fs/_fs_dir.ts",
    "_fs/_fs_dirent.ts",
    "_fs/_fs_exists.ts",
    "_fs/_fs_fdatasync.ts",
    "_fs/_fs_fstat.ts",
    "_fs/_fs_fsync.ts",
    "_fs/_fs_ftruncate.ts",
    "_fs/_fs_futimes.ts",
    "_fs/_fs_lchown.ts",
    "_fs/_fs_link.ts",
    "_fs/_fs_lstat.ts",
    "_fs/_fs_lutimes.ts",
    "_fs/_fs_mkdir.ts",
    "_fs/_fs_mkdtemp.ts",
    "_fs/_fs_open.ts",
    "_fs/_fs_opendir.ts",
    "_fs/_fs_read.ts",
    "_fs/_fs_readdir.ts",
    "_fs/_fs_readFile.ts",
    "_fs/_fs_readlink.ts",
    "_fs/_fs_readv.ts",
    "_fs/_fs_realpath.ts",
    "_fs/_fs_rename.ts",
    "_fs/_fs_rm.ts",
    "_fs/_fs_rmdir.ts",
    "_fs/_fs_stat.ts",
    "_fs/_fs_statfs.js",
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
    "_util/_util_callbackify.js",
    "_util/asserts.ts",
    "_util/async.ts",
    "_util/os.ts",
    "_util/std_asserts.ts",
    "_util/std_fmt_colors.ts",
    "_util/std_testing_diff.ts",
    "_utils.ts",
    "_zlib_binding.mjs",
    "_zlib.mjs",
    "assertion_error.ts",
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
    "internal/blocklist.mjs",
    "internal/buffer.mjs",
    "internal/child_process.ts",
    "internal/cli_table.ts",
    "internal/console/constructor.mjs",
    "internal/constants.ts",
    "internal/crypto/_keys.ts",
    "internal/crypto/_randomBytes.ts",
    "internal/crypto/_randomFill.mjs",
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
    "internal/fs/handle.ts",
    "internal/hide_stack_frames.ts",
    "internal/http.ts",
    "internal/idna.ts",
    "internal/net.ts",
    "internal/normalize_encoding.mjs",
    "internal/options.ts",
    "internal/primordials.mjs",
    "internal/process/per_thread.mjs",
    "internal/process/report.ts",
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
    "internal/util/parse_args/parse_args.js",
    "internal/util/parse_args/utils.js",
    "internal/util/types.ts",
    "internal/validators.mjs",
    "path/_constants.ts",
    "path/_interface.ts",
    "path/_util.ts",
    "path/_posix.ts",
    "path/_win32.ts",
    "path/common.ts",
    "path/mod.ts",
    "path/separator.ts",
    "readline/promises.ts",
    "wasi.ts",
    "node:assert" = "assert.ts",
    "node:assert/strict" = "assert/strict.ts",
    "node:async_hooks" = "async_hooks.ts",
    "node:buffer" = "buffer.ts",
    "node:child_process" = "child_process.ts",
    "node:cluster" = "cluster.ts",
    "node:console" = "console.ts",
    "node:constants" = "constants.ts",
    "node:crypto" = "crypto.ts",
    "node:dgram" = "dgram.ts",
    "node:diagnostics_channel" = "diagnostics_channel.js",
    "node:dns" = "dns.ts",
    "node:dns/promises" = "dns/promises.ts",
    "node:domain" = "domain.ts",
    "node:events" = "events.ts",
    "node:fs" = "fs.ts",
    "node:fs/promises" = "fs/promises.ts",
    "node:http" = "http.ts",
    "node:http2" = "http2.ts",
    "node:https" = "https.ts",
    "node:module" = "01_require.js",
    "node:net" = "net.ts",
    "node:os" = "os.ts",
    "node:path" = "path.ts",
    "node:path/posix" = "path/posix.ts",
    "node:path/win32" = "path/win32.ts",
    "node:perf_hooks" = "perf_hooks.ts",
    "node:process" = "process.ts",
    "node:punycode" = "punycode.ts",
    "node:querystring" = "querystring.js",
    "node:readline" = "readline.ts",
    "node:repl" = "repl.ts",
    "node:stream" = "stream.ts",
    "node:stream/consumers" = "stream/consumers.mjs",
    "node:stream/promises" = "stream/promises.mjs",
    "node:stream/web" = "stream/web.ts",
    "node:string_decoder" = "string_decoder.ts",
    "node:sys" = "sys.ts",
    "node:test" = "testing.ts",
    "node:timers" = "timers.ts",
    "node:timers/promises" = "timers/promises.ts",
    "node:tls" = "tls.ts",
    "node:tty" = "tty.js",
    "node:url" = "url.ts",
    "node:util" = "util.ts",
    "node:util/types" = "util/types.ts",
    "node:v8" = "v8.ts",
    "node:vm" = "vm.ts",
    "node:worker_threads" = "worker_threads.ts",
    "node:zlib" = "zlib.ts",
  ],
  options = {
    maybe_node_resolver: Option<NodeResolverRc>,
    maybe_npm_resolver: Option<NpmResolverRc>,
    fs: deno_fs::FileSystemRc,
  },
  state = |state, options| {
    // you should provide both of these or neither
    debug_assert_eq!(options.maybe_node_resolver.is_some(), options.maybe_npm_resolver.is_some());

    state.put(options.fs.clone());

    if let Some(node_resolver) = &options.maybe_node_resolver {
      state.put(node_resolver.clone());
    }
    if let Some(npm_resolver) = &options.maybe_npm_resolver {
      state.put(npm_resolver.clone());
    }
  },
  global_template_middleware = global_template_middleware,
  global_object_middleware = global_object_middleware,
  customizer = |ext: &mut deno_core::Extension| {
    let mut external_references = Vec::with_capacity(14);

    vm::GETTER_MAP_FN.with(|getter| {
      external_references.push(ExternalReference {
        named_getter: *getter,
      });
    });
    vm::SETTER_MAP_FN.with(|setter| {
      external_references.push(ExternalReference {
        named_setter: *setter,
      });
    });
    vm::DELETER_MAP_FN.with(|deleter| {
      external_references.push(ExternalReference {
        named_getter: *deleter,
      },);
    });
    vm::ENUMERATOR_MAP_FN.with(|enumerator| {
      external_references.push(ExternalReference {
        enumerator: *enumerator,
      });
    });
    vm::DEFINER_MAP_FN.with(|definer| {
      external_references.push(ExternalReference {
        named_definer: *definer,
      });
    });
    vm::DESCRIPTOR_MAP_FN.with(|descriptor| {
      external_references.push(ExternalReference {
        named_getter: *descriptor,
      });
    });

    vm::INDEXED_GETTER_MAP_FN.with(|getter| {
      external_references.push(ExternalReference {
        indexed_getter: *getter,
      });
    });
    vm::INDEXED_SETTER_MAP_FN.with(|setter| {
      external_references.push(ExternalReference {
        indexed_setter: *setter,
      });
    });
    vm::INDEXED_DELETER_MAP_FN.with(|deleter| {
      external_references.push(ExternalReference {
        indexed_getter: *deleter,
      });
    });
    vm::INDEXED_DEFINER_MAP_FN.with(|definer| {
      external_references.push(ExternalReference {
        indexed_definer: *definer,
      });
    });
    vm::INDEXED_DESCRIPTOR_MAP_FN.with(|descriptor| {
      external_references.push(ExternalReference {
        indexed_getter: *descriptor,
      });
    });

    global::GETTER_MAP_FN.with(|getter| {
      external_references.push(ExternalReference {
        named_getter: *getter,
      });
    });
    global::SETTER_MAP_FN.with(|setter| {
      external_references.push(ExternalReference {
        named_setter: *setter,
      });
    });
    global::QUERY_MAP_FN.with(|query| {
      external_references.push(ExternalReference {
        named_getter: *query,
      });
    });
    global::DELETER_MAP_FN.with(|deleter| {
      external_references.push(ExternalReference {
        named_getter: *deleter,
      },);
    });
    global::ENUMERATOR_MAP_FN.with(|enumerator| {
      external_references.push(ExternalReference {
        enumerator: *enumerator,
      });
    });
    global::DEFINER_MAP_FN.with(|definer| {
      external_references.push(ExternalReference {
        named_definer: *definer,
      });
    });
    global::DESCRIPTOR_MAP_FN.with(|descriptor| {
      external_references.push(ExternalReference {
        named_getter: *descriptor,
      });
    });
    ext.external_references.to_mut().extend(external_references);
  },
);

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
  );

  js_runtime.execute_script(located_script_name!(), source_code)?;
  Ok(())
}
