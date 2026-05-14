// Copyright 2018-2026 the Deno authors. MIT license.

#![deny(clippy::print_stderr)]
#![deny(clippy::print_stdout)]
#![allow(
  clippy::too_many_arguments,
  reason = "op macro expansion causes issues"
)]

use std::borrow::Cow;
use std::env;
use std::path::Path;

use deno_core::FastString;
use deno_core::OpState;
use deno_core::op2;
use deno_core::url::Url;
use deno_core::v8;
use deno_core::v8::ExternalReference;
use deno_error::JsErrorBox;
use deno_permissions::OpenAccessKind;
use deno_permissions::PermissionsContainer;
use node_resolver::DenoIsBuiltInNodeModuleChecker;
use node_resolver::InNpmPackageChecker;
use node_resolver::IsBuiltInNodeModuleChecker;
use node_resolver::NpmPackageFolderResolver;
use node_resolver::PackageJsonResolverRc;
use node_resolver::errors::PackageJsonLoadError;

extern crate libz_sys as zlib;

pub mod ops;

use deno_dotenv::parse_env_content_hook;
pub use deno_package_json::PackageJson;
use deno_permissions::PermissionCheckError;
pub use node_resolver::DENO_SUPPORTED_BUILTIN_NODE_MODULES as SUPPORTED_BUILTIN_NODE_MODULES;
pub use node_resolver::PathClean;
pub use ops::ipc::ChildPipeFd;
use ops::vm;
pub use ops::vm::ContextInitMode;
pub use ops::vm::VM_CONTEXT_INDEX;
pub use ops::vm::create_v8_context;
pub use ops::vm::init_global_template;

pub fn is_builtin_node_module(module_name: &str) -> bool {
  DenoIsBuiltInNodeModuleChecker.is_builtin_node_module(module_name)
}

#[allow(clippy::disallowed_types, reason = "definition")]
pub type NodeRequireLoaderRc = std::rc::Rc<dyn NodeRequireLoader>;

pub trait NodeRequireLoader {
  #[must_use = "the resolved return value to mitigate time-of-check to time-of-use issues"]
  fn ensure_read_permission<'a>(
    &self,
    permissions: &mut PermissionsContainer,
    path: Cow<'a, Path>,
  ) -> Result<Cow<'a, Path>, JsErrorBox>;

  fn load_text_file_lossy(&self, path: &Path)
  -> Result<FastString, JsErrorBox>;

  /// Get if the module kind is maybe CJS and loading should determine
  /// if its CJS or ESM.
  fn is_maybe_cjs(&self, specifier: &Url)
  -> Result<bool, PackageJsonLoadError>;

  fn resolve_require_node_module_paths(&self, from: &Path) -> Vec<String> {
    default_resolve_require_node_module_paths(from)
  }
}

pub fn default_resolve_require_node_module_paths(from: &Path) -> Vec<String> {
  let mut paths = Vec::with_capacity(from.components().count());
  let mut current_path = from;
  let mut maybe_parent = Some(current_path);
  while let Some(parent) = maybe_parent {
    if !parent.ends_with("node_modules") {
      paths.push(parent.join("node_modules").to_string_lossy().into_owned());
    }
    current_path = parent;
    maybe_parent = current_path.parent();
  }

  paths
}

#[op2]
#[string]
fn op_node_build_os() -> String {
  env!("TARGET").split('-').nth(2).unwrap().to_string()
}

#[derive(Debug, thiserror::Error, deno_error::JsError)]
enum DotEnvLoadErr {
  #[class(inherit)]
  #[error("{0}")]
  Fs(#[from] deno_io::fs::FsError),
  #[class(inherit)]
  #[error(transparent)]
  Permission(
    #[from]
    #[inherit]
    PermissionCheckError,
  ),
}

#[op2(fast)]
#[undefined]
fn op_node_load_env_file(
  state: &mut OpState,
  #[string] path: &str,
) -> Result<(), DotEnvLoadErr> {
  let fs = state.borrow::<deno_fs::FileSystemRc>().clone();
  let path = state
    .borrow::<PermissionsContainer>()
    .check_open(
      Cow::Borrowed(Path::new(path)),
      OpenAccessKind::ReadNoFollow,
      Some("process.loadEnvFile"),
    )
    .map_err(DotEnvLoadErr::Permission)?;

  let contents = fs.read_text_file_lossy_sync(&path)?;
  parse_env_content_hook(&contents, &mut |key, value| {
    // Follows Node.js behavior where null bytes are stripped from env keys and values
    let key = if let Some(null_pos) = key.find('\0') {
      &key[..null_pos]
    } else {
      key
    };

    if key.is_empty() {
      return;
    }

    let value = if let Some(null_pos) = value.find('\0') {
      &value[..null_pos]
    } else {
      value
    };

    // SAFETY: called during single-threaded initialization
    unsafe {
      env::set_var(key, value);
    }
  });

  Ok(())
}

#[derive(Clone)]
pub struct NodeExtInitServices<
  TInNpmPackageChecker: InNpmPackageChecker,
  TNpmPackageFolderResolver: NpmPackageFolderResolver,
  TSys: ExtNodeSys,
> {
  pub node_require_loader: NodeRequireLoaderRc,
  pub node_resolver:
    NodeResolverRc<TInNpmPackageChecker, TNpmPackageFolderResolver, TSys>,
  pub pkg_json_resolver: PackageJsonResolverRc<TSys>,
  pub sys: TSys,
}

deno_core::extension!(deno_node,
  deps = [ deno_io, deno_fs ],
  parameters = [TInNpmPackageChecker: InNpmPackageChecker, TNpmPackageFolderResolver: NpmPackageFolderResolver, TSys: ExtNodeSys],
  ops = [
    ops::assert::op_node_get_first_expression,

    ops::module_hooks::op_module_hooks_register,
    ops::module_hooks::op_module_hooks_poll_resolve,
    ops::module_hooks::op_module_hooks_respond_resolve,
    ops::module_hooks::op_module_hooks_poll_load,
    ops::module_hooks::op_module_hooks_respond_load,

    ops::blocklist::op_socket_address_parse,
    ops::blocklist::op_socket_address_get_serialization,

    ops::blocklist::op_blocklist_new,
    ops::blocklist::op_blocklist_add_address,
    ops::blocklist::op_blocklist_add_range,
    ops::blocklist::op_blocklist_add_subnet,
    ops::blocklist::op_blocklist_check,

    ops::buffer::op_mark_as_untransferable,
    ops::buffer::op_is_ascii,
    ops::buffer::op_is_utf8,
    ops::buffer::op_transcode,
    ops::buffer::op_node_buffer_compare,
    ops::buffer::op_node_buffer_compare_offset,
    ops::constant::op_node_fs_constants,
    ops::buffer::op_node_decode_utf8,
    ops::dns::op_node_getaddrinfo,
    ops::dns::op_node_getnameinfo,
    ops::fs::op_node_fs_exists_sync,
    ops::fs::op_node_fs_exists,
    ops::fs::op_node_lchmod_sync,
    ops::fs::op_node_lchmod,
    ops::fs::op_node_lchown_sync,
    ops::fs::op_node_lchown,
    ops::fs::op_node_lutimes_sync,
    ops::fs::op_node_lutimes,
    ops::fs::op_node_mkdtemp_sync,
    ops::fs::op_node_mkdtemp,
    ops::fs::op_node_open_sync,
    ops::fs::op_node_open,
    ops::fs::op_node_rmdir_sync,
    ops::fs::op_node_rmdir,
    ops::fs::op_node_statfs_sync,
    ops::fs::op_node_statfs,
    ops::fs::op_node_create_pipe,
    ops::fs::op_node_fd_set_blocking,
    ops::fs::op_node_fs_close,
    ops::fs::op_node_fs_read_sync,
    ops::fs::op_node_fs_read_deferred,
    ops::fs::op_node_fs_write_sync,
    ops::fs::op_node_fs_write_deferred,
    ops::fs::op_node_fs_seek_sync,
    ops::fs::op_node_fs_seek,
    ops::fs::op_node_fs_fstat_sync,
    ops::fs::op_node_fs_fstat,
    ops::fs::op_node_fs_ftruncate_sync,
    ops::fs::op_node_fs_ftruncate,
    ops::fs::op_node_fs_fsync_sync,
    ops::fs::op_node_fs_fsync,
    ops::fs::op_node_fs_fdatasync_sync,
    ops::fs::op_node_fs_fdatasync,
    ops::fs::op_node_fs_futimes_sync,
    ops::fs::op_node_fs_futimes,
    ops::fs::op_node_fs_fchmod_sync,
    ops::fs::op_node_fs_fchmod,
    ops::fs::op_node_fs_fchown_sync,
    ops::fs::op_node_fs_fchown,
    ops::fs::op_node_fs_read_file_sync,
    ops::fs::op_node_fs_read_file,
    ops::fs::op_node_cp_check_paths_recursive,
    ops::fs::op_node_cp_on_file,
    ops::fs::op_node_cp_on_link,
    ops::fs::op_node_cp_sync,
    ops::fs::op_node_cp_validate_and_prepare,
    ops::winerror::op_node_sys_to_uv_error,
    ops::v8::op_v8_cached_data_version_tag,
    ops::v8::op_v8_get_heap_statistics,
    ops::v8::op_v8_number_of_heap_spaces,
    ops::v8::op_v8_update_heap_space_statistics,
    ops::v8::op_v8_get_heap_code_statistics,
    ops::v8::op_v8_take_heap_snapshot,
    ops::v8::op_v8_get_wire_format_version,
    ops::v8::op_v8_new_deserializer,
    ops::v8::op_v8_new_serializer,
    ops::v8::op_v8_read_double,
    ops::v8::op_v8_read_header,
    ops::v8::op_v8_read_raw_bytes,
    ops::v8::op_v8_read_uint32,
    ops::v8::op_v8_read_uint64,
    ops::v8::op_v8_read_value,
    ops::v8::op_v8_release_buffer,
    ops::v8::op_v8_set_treat_array_buffer_views_as_host_objects,
    ops::v8::op_v8_transfer_array_buffer,
    ops::v8::op_v8_transfer_array_buffer_de,
    ops::v8::op_v8_write_double,
    ops::v8::op_v8_write_header,
    ops::v8::op_v8_write_raw_bytes,
    ops::v8::op_v8_write_uint32,
    ops::v8::op_v8_write_uint64,
    ops::v8::op_v8_write_value,
    ops::vm::op_vm_create_script,
    ops::vm::op_vm_create_context,
    ops::vm::op_vm_create_context_without_contextify,
    ops::vm::op_vm_script_run_in_context,
    ops::vm::op_vm_is_context,
    ops::vm::op_vm_compile_function,
    ops::vm::op_vm_script_get_source_map_url,
    ops::vm::op_vm_script_create_cached_data,
    ops::vm::op_vm_module_create_source_text_module,
    ops::vm::op_vm_module_link,
    ops::vm::op_vm_module_instantiate,
    ops::vm::op_vm_module_evaluate,
    ops::vm::op_vm_module_get_status,
    ops::vm::op_vm_module_get_namespace,
    ops::vm::op_vm_module_get_exception,
    ops::vm::op_vm_module_get_module_requests,
    ops::vm::op_vm_module_get_identifier,
    ops::idna::op_node_idna_domain_to_ascii,
    ops::idna::op_node_idna_domain_to_unicode,
    ops::idna::op_node_idna_punycode_to_ascii,
    ops::idna::op_node_idna_punycode_to_unicode,
    ops::idna::op_node_idna_punycode_decode,
    ops::idna::op_node_idna_punycode_encode,
    ops::zlib::op_zlib_crc32,
    ops::zlib::op_zlib_crc32_string,
    ops::handle_wrap::op_node_new_async_id,
    ops::http2::op_http2_callbacks,
    // Keep the HTTP/2 error-string op wired so `internal/test/binding`
    // can mirror Node's `internalBinding('http2').nghttp2ErrorString()`
    // in node_compat tests; the JS side also exposes `respond` /
    // `pushPromise` shims on `Http2Stream` so tests can monkey-patch the
    // prototype to inject NGHTTP2 error codes.
    ops::http2::op_http2_error_string,
    ops::http2::op_http2_http_state,
    ops::os::op_node_os_get_priority,
    ops::os::op_node_os_set_priority,
    ops::os::op_node_os_user_info,
    ops::os::op_geteuid,
    ops::os::op_getegid,
    ops::os::op_getgroups,
    ops::os::op_cpus,
    ops::os::op_homedir,
    op_node_build_os,
    op_node_load_env_file,
    ops::require::op_require_can_parse_as_esm,
    ops::require::op_require_init_paths,
    ops::require::op_require_node_module_paths<TSys>,
    ops::require::op_require_proxy_path,
    ops::require::op_require_is_deno_dir_package<TInNpmPackageChecker, TNpmPackageFolderResolver, TSys>,
    ops::require::op_require_resolve_deno_dir<TInNpmPackageChecker, TNpmPackageFolderResolver, TSys>,
    ops::require::op_require_is_maybe_cjs,
    ops::require::op_require_is_request_relative,
    ops::require::op_require_resolve_lookup_paths,
    ops::require::op_require_try_self<TInNpmPackageChecker, TNpmPackageFolderResolver, TSys>,
    ops::require::op_require_real_path<TSys>,
    ops::require::op_require_path_is_absolute,
    ops::require::op_require_path_dirname,
    ops::require::op_require_stat<TSys>,
    ops::require::op_require_path_resolve,
    ops::require::op_require_path_basename,
    ops::require::op_require_read_file,
    ops::require::op_require_as_file_path,
    ops::require::op_require_resolve_exports<TInNpmPackageChecker, TNpmPackageFolderResolver, TSys>,
    ops::require::op_require_read_package_scope<TSys>,
    ops::require::op_require_package_imports_resolve<TInNpmPackageChecker, TNpmPackageFolderResolver, TSys>,
    ops::require::op_require_break_on_next_statement,
    ops::util::op_node_guess_handle_type,
    ops::util::op_node_view_has_buffer,
    ops::util::op_node_get_own_non_index_properties,
    ops::util::op_node_call_is_from_dependency<TInNpmPackageChecker, TNpmPackageFolderResolver, TSys>,
    ops::util::op_node_in_npm_package<TInNpmPackageChecker, TNpmPackageFolderResolver, TSys>,
    ops::util::op_node_parse_env,
    ops::worker_threads::op_worker_threads_filename<TSys>,
    ops::worker_threads::op_worker_get_resource_limits,
    ops::ipc::op_node_child_ipc_pipe,
    ops::ipc::op_node_ipc_write_json,
    ops::ipc::op_node_ipc_read_json,
    ops::ipc::op_node_ipc_read_advanced,
    ops::ipc::op_node_ipc_write_advanced,
    ops::ipc::op_node_ipc_buffer_constructor,
    ops::ipc::op_node_ipc_ref,
    ops::ipc::op_node_ipc_unref,
    ops::process::op_node_process_set_title,
    ops::process::op_node_process_kill,
    ops::process::op_node_process_setegid,
    ops::process::op_node_process_seteuid,
    ops::process::op_node_process_setgid,
    ops::process::op_node_process_setuid,
    ops::process::op_process_abort,
    ops::process::op_node_process_constrained_memory<TSys>,
    ops::node_cli_parser::op_node_translate_cli_args,
    ops::shell::op_node_parse_shell_args,
    ops::tls::op_get_root_certificates,
    ops::tls::op_node_get_ca_certificates<TSys>,
    ops::tls::op_set_default_ca_certificates,
    ops::tls::op_tls_peer_certificate,
    ops::tls::op_tls_canonicalize_ipv4_address,
    ops::tls::op_node_tls_start,
    ops::tls::op_node_tls_handshake,
    ops::inspector::op_inspector_open,
    ops::inspector::op_inspector_close,
    ops::inspector::op_inspector_url,
    ops::inspector::op_inspector_wait,
    ops::inspector::op_inspector_connect,
    ops::inspector::op_inspector_dispatch,
    ops::inspector::op_inspector_disconnect,
    ops::inspector::op_inspector_emit_protocol_event,
    ops::inspector::op_inspector_enabled,
    ops::udp::op_node_udp_bind,
    ops::udp::op_node_udp_join_multi_v4,
    ops::udp::op_node_udp_leave_multi_v4,
    ops::udp::op_node_udp_join_multi_v6,
    ops::udp::op_node_udp_leave_multi_v6,
    ops::udp::op_node_udp_set_broadcast,
    ops::udp::op_node_udp_set_multicast_loopback,
    ops::udp::op_node_udp_set_multicast_ttl,
    ops::udp::op_node_udp_set_ttl,
    ops::udp::op_node_udp_set_multicast_interface,
    ops::udp::op_node_udp_join_source_specific,
    ops::udp::op_node_udp_leave_source_specific,
    ops::udp::op_node_udp_send,
    ops::udp::op_node_udp_recv,
    ops::udp::op_node_udp_fd_for_ipc,
    ops::udp::op_node_udp_open,
    ops::stream_wrap::op_stream_base_register_state,
    ops::tty_wrap::op_tty_check_fd_permission,
  ],
  objects = [
    ops::perf_hooks::EldHistogram,
    ops::handle_wrap::AsyncWrap,
    ops::handle_wrap::HandleWrap,
    ops::stream_wrap::LibUvStreamWrap,
    ops::tty_wrap::TTY,
    ops::zlib::BrotliDecoder,
    ops::zlib::BrotliEncoder,
    ops::zlib::Zlib,
    ops::zlib::ZstdCompress,
    ops::zlib::ZstdDecompress,
    ops::tcp_wrap::TCPWrap,
    ops::pipe_wrap::PipeWrap,
    ops::tls_wrap::TLSWrap,
    ops::llhttp::binding::HTTPParser,
    ops::http2::Http2Session,
    ops::http2::Http2Stream,
  ],
  esm_entry_point = "node:module",
  esm = [
    dir "polyfills",
    "internal/streams/compose.js",
    "internal/streams/duplexpair.js",
    "internal/streams/lazy_transform.js",
    "internal/streams/pipeline.js",
    "internal_binding/mod.ts",
    "internal/streams/operators.js",
    "node:module" = "01_require.js",
    "node:process" = "process.ts",
    "node:repl" = "repl.ts",
    "node:stream" = "stream.ts",
    "node:stream/promises" = "stream/promises.js",
  ],
  lazy_loaded_esm = [
    dir "polyfills",
    "_fs/_fs_copy.ts",
    "_fs/_fs_dir.ts",
    "_fs/_fs_exists.ts",
    "_fs/_fs_glob.ts",
    "_fs/_fs_lutimes.ts",
    "_fs/_fs_read.ts",
    "_fs/_fs_readdir.ts",
    "_process/streams.mjs",
    "internal/fs/promises.ts",
    "internal/fs/stat_utils.ts",
    "internal/event_target.mjs",
    "internal/fs/streams.mjs",
    "internal/fs/utils.mjs",
    "internal/fs/handle.ts",
    "internal/repl.ts",
    "_readline.mjs",
    "internal/streams/duplexify.js",
    "internal/streams/fast-utf8-stream.js",
    "internal/streams/from.js",
    "internal/tty.js",
    "internal/webstreams/adapters.js",
    "readline/promises.ts",
    "node:readline/promises" = "readline/promises.ts",
    "deps/minimatch.js",
    "node:_http_agent" = "_http_agent.js",
    "node:_http_client" = "_http_client.js",
    "node:_http_common" = "_http_common.js",
    "node:_http_incoming" = "_http_incoming.js",
    "node:_http_outgoing" = "_http_outgoing.ts",
    "node:_http_server" = "_http_server.js",
    "node:path" = "path.ts",
    "node:path/posix" = "path/posix.ts",
    "node:path/win32" = "path/win32.ts",
    "node:assert" = "assert_esm.ts",
    "node:buffer" = "buffer.ts",
    "node:assert/strict" = "assert/strict.ts",
    "node:async_hooks" = "async_hooks_esm.ts",
    "node:diagnostics_channel" = "diagnostics_channel_esm.js",
    "node:events" = "events_esm.ts",
    "node:domain" = "domain_esm.ts",
    "node:perf_hooks" = "perf_hooks_esm.js",
    "node:punycode" = "punycode_esm.ts",
    "node:querystring" = "querystring_esm.js",
    "node:sys" = "sys_esm.js",
    "node:trace_events" = "trace_events_esm.ts",
    "node:util" = "util_esm.ts",
    "node:util/types" = "util/types.ts",
    "node:vm" = "vm_esm.js",
    "node:wasi" = "wasi_esm.ts",
    "node:sqlite" = "sqlite_esm.ts",
    "node:os" = "os_esm.ts",
    "node:stream/consumers" = "stream/consumers_esm.js",
    "node:stream/web" = "stream/web_esm.js",
    "node:string_decoder" = "string_decoder_esm.ts",
    "node:test" = "testing_esm.ts",
    "node:test/reporters" = "test/reporters_esm.ts",
    "node:cluster" = "cluster_esm.ts",
    "node:console" = "console_esm.ts",
    "node:constants" = "constants_esm.ts",
    "node:crypto" = "crypto_esm.ts",
    "node:dgram" = "dgram_esm.ts",
    "node:dns" = "dns_esm.ts",
    "node:dns/promises" = "dns/promises_esm.ts",
    "node:timers" = "timers_esm.ts",
    "node:timers/promises" = "timers/promises_esm.ts",
    "node:tls" = "tls_esm.ts",
    "node:tty" = "tty_esm.ts",
    "node:url" = "url_esm.ts",
    "node:v8" = "v8_esm.ts",
    "node:worker_threads" = "worker_threads_esm.ts",
    "node:zlib" = "zlib_esm.ts",
    "node:child_process" = "child_process_esm.ts",
    "node:fs" = "fs_esm.ts",
    "node:fs/promises" = "fs/promises_esm.ts",
    "node:http" = "http_esm.ts",
    "node:http2" = "http2_esm.ts",
    "node:https" = "https_esm.ts",
    "node:inspector" = "inspector_esm.js",
    "node:inspector/promises" = "inspector/promises_esm.js",
    "node:_stream_duplex" = "internal/streams/duplex_esm.js",
    "node:_stream_passthrough" = "internal/streams/passthrough_esm.js",
    "node:_stream_readable" = "internal/streams/readable_esm.js",
    "node:_stream_transform" = "internal/streams/transform_esm.js",
    "node:_stream_writable" = "internal/streams/writable_esm.js",
    "node:net" = "net_esm.ts",
    "node:_tls_common" = "_tls_common_esm.ts",
    "node:_tls_wrap" = "_tls_wrap_esm.js",
    "node:readline" = "readline.ts",
  ],
  lazy_loaded_js = [
    dir "polyfills",
    "cluster.ts",
    "console.ts",
    "constants.ts",
    "crypto.ts",
    "dgram.ts",
    "dns.ts",
    "dns/promises.ts",
    "timers.ts",
    "timers/promises.ts",
    "tls.ts",
    "tty.js",
    "url.ts",
    "v8.ts",
    "worker_threads.ts",
    "zlib.js",
    "child_process.ts",
    "fs.ts",
    "fs/promises.ts",
    "net.ts",
    "_tls_common.ts",
    "_tls_wrap.js",
    "http.ts",
    "http2.ts",
    "https.ts",
    "inspector.js",
    "inspector/promises.js",
    "internal/streams/duplex.js",
    "internal/streams/passthrough.js",
    "internal/streams/readable.js",
    "internal/streams/transform.js",
    "internal/streams/writable.js",
    "internal/validators.mjs",
    "internal/normalize_encoding.ts",
    "internal/error_codes.ts",
    "internal/hide_stack_frames.ts",
    "internal/util/types.ts",
    "internal/crypto/_keys.ts",
    "internal/crypto/constants.ts",
    "internal_binding/types.ts",
    "_util/os.ts",
    "_utils.ts",
    "internal/primordials.mjs",
    "internal_binding/constants.ts",
    "internal_binding/_libuv_winerror.ts",
    "internal_binding/uv.ts",
    "internal/util/inspect.mjs",
    "internal/errors.ts",
    "internal/errors/error_source.ts",
    "internal/util.mjs",
    "_fs/_fs_constants.ts",
    "_next_tick.ts",
    "_process/exiting.ts",
    "_process/process.ts",
    "_util/_util_callbackify.js",
    "_zlib_binding.mjs",
    "internal_binding/_listen.ts",
    "internal_binding/_node.ts",
    "internal_binding/_utils.ts",
    "internal_binding/ares.ts",
    "internal_binding/async_wrap.ts",
    "internal_binding/block_list.ts",
    "internal_binding/_timingSafeEqual.ts",
    "internal_binding/cares_wrap.ts",
    "internal_binding/crypto.ts",
    "internal_binding/buffer.ts",
    "internal_binding/http_parser.ts",
    "internal_binding/handle_wrap.ts",
    "internal_binding/http2.ts",
    "internal_binding/node_file.ts",
    "internal_binding/node_options.ts",
    "internal_binding/pipe_wrap.ts",
    "internal_binding/stream_wrap.ts",
    "internal_binding/string_decoder.ts",
    "internal_binding/symbols.ts",
    "internal_binding/tcp_wrap.ts",
    "internal_binding/tls_wrap.ts",
    "internal_binding/tty_wrap.ts",
    "internal_binding/udp_wrap.ts",
    "internal_binding/util.ts",
    "internal/assert/assertion_error.js",
    "internal/assert/calltracker.js",
    "internal/assert/myers_diff.js",
    "internal/assert/utils.ts",
    "internal/assert.mjs",
    "internal/async_hooks.ts",
    "internal/blocklist.mjs",
    "internal/buffer.mjs",
    "internal/cli_table.ts",
    "internal/constants.ts",
    "internal/crypto/_randomBytes.ts",
    "internal/crypto/_randomFill.mjs",
    "internal/crypto/_randomInt.ts",
    "internal/crypto/certificate.ts",
    "internal/crypto/cipher.ts",
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
    "internal/child_process.ts",
    "internal/cluster/child.ts",
    "internal/cluster/linkedlist.ts",
    "internal/cluster/primary.ts",
    "internal/cluster/round_robin_handle.ts",
    "internal/cluster/shared_handle.ts",
    "internal/cluster/utils.ts",
    "internal/cluster/worker.ts",
    "internal/console/constructor.mjs",
    "internal/deps/undici/undici.js",
    "internal/dgram.ts",
    "internal/dns/promises.ts",
    "internal/dns/utils.ts",
    "internal/dtrace.ts",
    "internal/events/abort_listener.mjs",
    "internal/fs/sync_write_stream.js",
    "internal/http.ts",
    "internal/http2/compat.js",
    "internal/http2/constants.ts",
    "internal/http2/core.ts",
    "internal/http2/util.ts",
    "internal/js_stream_socket.js",
    "internal/idna.ts",
    "internal/mime.ts",
    "internal/options.ts",
    "internal/priority_queue.ts",
    "internal/process/per_thread.mjs",
    "internal/process/report.ts",
    "internal/process/warning.ts",
    "internal/querystring.ts",
    "internal/readline/callbacks.mjs",
    "internal/readline/emitKeypressEvents.mjs",
    "internal/readline/interface.mjs",
    "internal/readline/promises.mjs",
    "internal/readline/symbols.mjs",
    "internal/readline/utils.mjs",
    "internal/socketaddress.js",
    "internal/stream_base_commons.ts",
    "internal/streams/add-abort-signal.js",
    "internal/streams/utils.js",
    "internal/test/binding.ts",
    "internal/timers.mjs",
    "internal/url.ts",
    "internal/util/colors.ts",
    "internal/util/debuglog.ts",
    "internal/util/parse_args/parse_args.js",
    "internal/util/parse_args/utils.js",
    "internal/net.ts",
    "internal/tls_common.js",
    "internal/util/comparisons.ts",
    "path/_constants.ts",
    "path/_interface.ts",
    "path/_util.ts",
    "path/common.ts",
    "path/separator.ts",
    "assert.ts",
    "util.ts",
    "_events.mjs",
    "internal/streams/state.js",
    "internal/streams/legacy.js",
    "internal/streams/destroy.js",
    "internal/streams/end-of-stream.js",
    "async_hooks.ts",
    "diagnostics_channel.js",
    "domain.ts",
    "perf_hooks.js",
    "punycode.ts",
    "querystring.js",
    "trace_events.ts",
    "vm.js",
    "wasi.ts",
    "sqlite.ts",
    "os.ts",
    "stream/consumers.js",
    "stream/web.js",
    "string_decoder.ts",
    "testing.ts",
    "test/reporters.ts",
    "_fs/_fs_common.ts",
    "_fs/_fs_cp.ts",
    "_fs/_fs_fstat.ts",
    "_fs/_fs_lstat.ts",
    "_fs/cp/cp.ts",
    "_fs/cp/cp_sync.ts",
    "path/_posix.ts",
    "path/_win32.ts",
    "path/mod.ts",
  ],
  options = {
    maybe_init: Option<NodeExtInitServices<TInNpmPackageChecker, TNpmPackageFolderResolver, TSys>>,
    fs: deno_fs::FileSystemRc,
  },
  state = |state, options| {
    state.put(options.fs.clone());
    state.put(ops::module_hooks::LoaderHookRegistry::default());

    if let Some(init) = &options.maybe_init {
      state.put(init.sys.clone());
      state.put(init.node_require_loader.clone());
      state.put(init.node_resolver.clone());
      state.put(init.pkg_json_resolver.clone());
    }

    // Always seed `NodeTlsState` so the shared client session cache is
    // available for TLS resumption from the very first `tls.connect()`.
    // Without this, every connection built its ClientConfig with an empty
    // per-config session cache and `isSessionReused()` always returned false.
    state.put(crate::ops::tls::NodeTlsState {
      custom_ca_certs: None,
      client_session_store: std::sync::Arc::new(
        deno_tls::rustls::client::ClientSessionMemoryCache::new(256),
      ),
      server_ticketer: None,
      cached_default_verifier: None,
      cached_no_client_auth: None,
    });
  },
  customizer = |ext: &mut deno_core::Extension| {
    let external_references = [
      vm::QUERY_MAP_FN.with(|query| {
        ExternalReference {
          named_query: *query,
        }
      }),
      vm::GETTER_MAP_FN.with(|getter| {
        ExternalReference {
          named_getter: *getter,
        }
      }),
      vm::SETTER_MAP_FN.with(|setter| {
        ExternalReference {
          named_setter: *setter,
        }
      }),
      vm::DESCRIPTOR_MAP_FN.with(|descriptor| {
        ExternalReference {
          named_getter: *descriptor,
        }
      }),
      vm::DELETER_MAP_FN.with(|deleter| {
        ExternalReference {
          named_deleter: *deleter,
        }
      }),
      vm::ENUMERATOR_MAP_FN.with(|enumerator| {
        ExternalReference {
          enumerator: *enumerator,
        }
      }),
      vm::DEFINER_MAP_FN.with(|definer| {
        ExternalReference {
          named_definer: *definer,
        }
      }),

      vm::INDEXED_QUERY_MAP_FN.with(|query| {
        ExternalReference {
          indexed_query: *query,
        }
      }),
      vm::INDEXED_GETTER_MAP_FN.with(|getter| {
        ExternalReference {
          indexed_getter: *getter,
        }
      }),
      vm::INDEXED_SETTER_MAP_FN.with(|setter| {
        ExternalReference {
          indexed_setter: *setter,
        }
      }),
      vm::INDEXED_DESCRIPTOR_MAP_FN.with(|descriptor| {
        ExternalReference {
          indexed_getter: *descriptor,
        }
      }),
      vm::INDEXED_DELETER_MAP_FN.with(|deleter| {
        ExternalReference {
          indexed_deleter: *deleter,
        }
      }),
      vm::INDEXED_DEFINER_MAP_FN.with(|definer| {
        ExternalReference {
          indexed_definer: *definer,
        }
      }),
      vm::INDEXED_ENUMERATOR_MAP_FN.with(|enumerator| {
        ExternalReference {
          enumerator: *enumerator,
        }
      }),

    ];

    ext.external_references.to_mut().extend(external_references);
  },
);

#[sys_traits::auto_impl]
pub trait ExtNodeSys:
  node_resolver::NodeResolverSys
  + sys_traits::EnvCurrentDir
  + sys_traits::EnvVar
  + Clone
{
}

pub type NodeResolver<TInNpmPackageChecker, TNpmPackageFolderResolver, TSys> =
  node_resolver::NodeResolver<
    TInNpmPackageChecker,
    DenoIsBuiltInNodeModuleChecker,
    TNpmPackageFolderResolver,
    TSys,
  >;
#[allow(clippy::disallowed_types, reason = "definition")]
pub type NodeResolverRc<TInNpmPackageChecker, TNpmPackageFolderResolver, TSys> =
  deno_fs::sync::MaybeArc<
    NodeResolver<TInNpmPackageChecker, TNpmPackageFolderResolver, TSys>,
  >;

pub fn create_host_defined_options<'s>(
  scope: &mut v8::PinScope<'s, '_>,
) -> v8::Local<'s, v8::Data> {
  let host_defined_options = v8::PrimitiveArray::new(scope, 1);
  let value = v8::Boolean::new(scope, true);
  host_defined_options.set(scope, 0, value.into());
  host_defined_options.into()
}
