// Copyright the Deno authors. MIT license.

/*!
 * Node.js CLI Argument Parser - Rust Implementation
 *
 * This is a Rust implementation that matches the exact behavior of
 * Node.js CLI argument parsing found in src/node_options.cc and related files.
 *
 * Based on Node.js source code analysis of:
 * - src/node_options.cc (option definitions and parsing logic)
 * - src/node_options.h (option structures and types)
 * - src/node_options-inl.h (template implementation)
 * - src/node.cc (main execution flow)
 */

use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub enum OptionType {
    NoOp,
    V8Option,
    Boolean,
    Integer,
    UInteger,
    String,
    HostPort,
    StringList,
}

#[derive(Debug, Clone, PartialEq)]
pub enum OptionEnvvarSettings {
    DisallowedInEnvvar,
    AllowedInEnvvar,
}

#[derive(Debug, Clone)]
pub struct HostPort {
    pub host: String,
    pub port: u16,
}

impl Default for HostPort {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 9229,
        }
    }
}

impl HostPort {
    pub fn new(host: String, port: u16) -> Self {
        Self { host, port }
    }

    pub fn update(&mut self, other: &HostPort) {
        if !other.host.is_empty() {
            self.host = other.host.clone();
        }
        self.port = other.port;
    }
}

#[derive(Debug, Clone, Default)]
pub struct InspectPublishUid {
    pub console: bool,
    pub http: bool,
}

#[derive(Debug, Clone)]
pub struct DebugOptions {
    pub allow_attaching_debugger: bool,
    pub inspector_enabled: bool,
    pub inspect_wait: bool,
    pub deprecated_debug: bool,
    pub break_first_line: bool,
    pub break_node_first_line: bool,
    pub inspect_publish_uid_string: String,
    pub inspect_publish_uid: InspectPublishUid,
    pub host_port: HostPort,
}

impl Default for DebugOptions {
    fn default() -> Self {
        Self {
            allow_attaching_debugger: true,
            inspector_enabled: false,
            inspect_wait: false,
            deprecated_debug: false,
            break_first_line: false,
            break_node_first_line: false,
            inspect_publish_uid_string: "stderr,http".to_string(),
            inspect_publish_uid: InspectPublishUid::default(),
            host_port: HostPort::default(),
        }
    }
}

impl DebugOptions {
    pub fn enable_break_first_line(&mut self) {
        self.inspector_enabled = true;
        self.break_first_line = true;
    }

    pub fn disable_wait_or_break_first_line(&mut self) {
        self.inspect_wait = false;
        self.break_first_line = false;
    }

    pub fn wait_for_connect(&self) -> bool {
        self.break_first_line || self.break_node_first_line || self.inspect_wait
    }

    pub fn should_break_first_line(&self) -> bool {
        self.break_first_line || self.break_node_first_line
    }

    pub fn check_options(&mut self, errors: &mut Vec<String>) {
        let entries: Vec<&str> = self.inspect_publish_uid_string.split(',').collect();
        self.inspect_publish_uid.console = false;
        self.inspect_publish_uid.http = false;

        for entry in entries {
            let destination = entry.trim();
            match destination {
                "stderr" => self.inspect_publish_uid.console = true,
                "http" => self.inspect_publish_uid.http = true,
                _ => errors
                    .push("--inspect-publish-uid destination can be stderr or http".to_string()),
            }
        }

        if self.deprecated_debug {
            errors.push("[DEP0062]: `node --debug` and `node --debug-brk` are invalid. Please use `node --inspect` and `node --inspect-brk` instead.".to_string());
        }
    }
}

#[derive(Debug, Clone)]
pub struct EnvironmentOptions {
    pub abort_on_uncaught_exception: bool,
    pub conditions: Vec<String>,
    pub detect_module: bool,
    pub disable_sigusr1: bool,
    pub print_required_tla: bool,
    pub require_module: bool,
    pub dns_result_order: String,
    pub enable_source_maps: bool,
    pub experimental_addon_modules: bool,
    pub experimental_eventsource: bool,
    pub experimental_fetch: bool,
    pub experimental_websocket: bool,
    pub experimental_sqlite: bool,
    pub experimental_webstorage: bool,
    pub experimental_quic: bool,
    pub localstorage_file: String,
    pub experimental_global_navigator: bool,
    pub experimental_global_web_crypto: bool,
    pub experimental_wasm_modules: bool,
    pub experimental_import_meta_resolve: bool,
    pub input_type: String,
    pub entry_is_url: bool,
    pub permission: bool,
    pub allow_fs_read: Vec<String>,
    pub allow_fs_write: Vec<String>,
    pub allow_addons: bool,
    pub allow_child_process: bool,
    pub allow_net: Vec<String>,
    pub allow_wasi: bool,
    pub allow_worker_threads: bool,
    pub experimental_repl_await: bool,
    pub experimental_vm_modules: bool,
    pub async_context_frame: bool,
    pub expose_internals: bool,
    pub force_node_api_uncaught_exceptions_policy: bool,
    pub frozen_intrinsics: bool,
    pub heap_snapshot_near_heap_limit: i64,
    pub heapsnapshot_signal: String,
    pub network_family_autoselection: bool,
    pub network_family_autoselection_attempt_timeout: u64,
    pub max_http_header_size: u64,
    pub deprecation: bool,
    pub force_async_hooks_checks: bool,
    pub allow_native_addons: bool,
    pub global_search_paths: bool,
    pub warnings: bool,
    pub disable_warnings: Vec<String>,
    pub force_context_aware: bool,
    pub pending_deprecation: bool,
    pub preserve_symlinks: bool,
    pub preserve_symlinks_main: bool,
    pub prof_process: bool,
    pub cpu_prof_dir: String,
    pub cpu_prof_interval: u64,
    pub cpu_prof_name: String,
    pub cpu_prof: bool,
    pub experimental_network_inspection: bool,
    pub experimental_worker_inspection: bool,
    pub heap_prof_dir: String,
    pub heap_prof_name: String,
    pub heap_prof_interval: u64,
    pub heap_prof: bool,
    pub redirect_warnings: String,
    pub diagnostic_dir: String,
    pub env_file: String,
    pub optional_env_file: String,
    pub has_env_file_string: bool,
    pub test_runner: bool,
    pub test_runner_concurrency: u64,
    pub test_runner_timeout: u64,
    pub test_runner_coverage: bool,
    pub test_runner_force_exit: bool,
    pub test_coverage_branches: u64,
    pub test_coverage_functions: u64,
    pub test_coverage_lines: u64,
    pub test_runner_module_mocks: bool,
    pub test_runner_update_snapshots: bool,
    pub test_name_pattern: Vec<String>,
    pub test_reporter: Vec<String>,
    pub test_reporter_destination: Vec<String>,
    pub test_global_setup_path: String,
    pub test_only: bool,
    pub test_udp_no_try_send: bool,
    pub test_isolation: String,
    pub test_shard: String,
    pub test_skip_pattern: Vec<String>,
    pub coverage_include_pattern: Vec<String>,
    pub coverage_exclude_pattern: Vec<String>,
    pub throw_deprecation: bool,
    pub trace_deprecation: bool,
    pub trace_exit: bool,
    pub trace_sync_io: bool,
    pub trace_tls: bool,
    pub trace_uncaught: bool,
    pub trace_warnings: bool,
    pub trace_promises: bool,
    pub trace_env: bool,
    pub trace_env_js_stack: bool,
    pub trace_env_native_stack: bool,
    pub trace_require_module: String,
    pub extra_info_on_fatal_exception: bool,
    pub unhandled_rejections: String,
    pub userland_loaders: Vec<String>,
    pub verify_base_objects: bool,
    pub watch_mode: bool,
    pub watch_mode_report_to_parent: bool,
    pub watch_mode_preserve_output: bool,
    pub watch_mode_kill_signal: String,
    pub watch_mode_paths: Vec<String>,
    pub syntax_check_only: bool,
    pub has_eval_string: bool,
    pub eval_string: String,
    pub print_eval: bool,
    pub force_repl: bool,
    pub insecure_http_parser: bool,
    pub tls_min_v1_0: bool,
    pub tls_min_v1_1: bool,
    pub tls_min_v1_2: bool,
    pub tls_min_v1_3: bool,
    pub tls_max_v1_2: bool,
    pub tls_max_v1_3: bool,
    pub tls_keylog: String,
    pub preload_cjs_modules: Vec<String>,
    pub preload_esm_modules: Vec<String>,
    pub experimental_strip_types: bool,
    pub experimental_transform_types: bool,
    pub user_argv: Vec<String>,
    pub report_exclude_env: bool,
    pub report_exclude_network: bool,
    pub experimental_config_file_path: String,
    pub experimental_default_config_file: bool,
    pub debug_options: DebugOptions,
}

impl Default for EnvironmentOptions {
    fn default() -> Self {
        Self {
            abort_on_uncaught_exception: false,
            conditions: Vec::new(),
            detect_module: true,
            disable_sigusr1: false,
            print_required_tla: false,
            require_module: true,
            dns_result_order: String::new(),
            enable_source_maps: false,
            experimental_addon_modules: false,
            experimental_eventsource: false,
            experimental_fetch: true,
            experimental_websocket: true,
            experimental_sqlite: true,
            experimental_webstorage: false,
            experimental_quic: false,
            localstorage_file: String::new(),
            experimental_global_navigator: true,
            experimental_global_web_crypto: true,
            experimental_wasm_modules: false,
            experimental_import_meta_resolve: false,
            input_type: String::new(),
            entry_is_url: false,
            permission: false,
            allow_fs_read: Vec::new(),
            allow_fs_write: Vec::new(),
            allow_addons: false,
            allow_child_process: false,
            allow_net: Vec::new(),
            allow_wasi: false,
            allow_worker_threads: false,
            experimental_repl_await: true,
            experimental_vm_modules: false,
            async_context_frame: true,
            expose_internals: false,
            force_node_api_uncaught_exceptions_policy: false,
            frozen_intrinsics: false,
            heap_snapshot_near_heap_limit: 0,
            heapsnapshot_signal: String::new(),
            network_family_autoselection: true,
            network_family_autoselection_attempt_timeout: 250,
            max_http_header_size: 16 * 1024,
            deprecation: true,
            force_async_hooks_checks: true,
            allow_native_addons: true,
            global_search_paths: true,
            warnings: true,
            disable_warnings: Vec::new(),
            force_context_aware: false,
            pending_deprecation: false,
            preserve_symlinks: false,
            preserve_symlinks_main: false,
            prof_process: false,
            cpu_prof_dir: String::new(),
            cpu_prof_interval: 1000,
            cpu_prof_name: String::new(),
            cpu_prof: false,
            experimental_network_inspection: false,
            experimental_worker_inspection: false,
            heap_prof_dir: String::new(),
            heap_prof_name: String::new(),
            heap_prof_interval: 512 * 1024,
            heap_prof: false,
            redirect_warnings: String::new(),
            diagnostic_dir: String::new(),
            env_file: String::new(),
            optional_env_file: String::new(),
            has_env_file_string: false,
            test_runner: false,
            test_runner_concurrency: 0,
            test_runner_timeout: 0,
            test_runner_coverage: false,
            test_runner_force_exit: false,
            test_coverage_branches: 0,
            test_coverage_functions: 0,
            test_coverage_lines: 0,
            test_runner_module_mocks: false,
            test_runner_update_snapshots: false,
            test_name_pattern: Vec::new(),
            test_reporter: Vec::new(),
            test_reporter_destination: Vec::new(),
            test_global_setup_path: String::new(),
            test_only: false,
            test_udp_no_try_send: false,
            test_isolation: "process".to_string(),
            test_shard: String::new(),
            test_skip_pattern: Vec::new(),
            coverage_include_pattern: Vec::new(),
            coverage_exclude_pattern: Vec::new(),
            throw_deprecation: false,
            trace_deprecation: false,
            trace_exit: false,
            trace_sync_io: false,
            trace_tls: false,
            trace_uncaught: false,
            trace_warnings: false,
            trace_promises: false,
            trace_env: false,
            trace_env_js_stack: false,
            trace_env_native_stack: false,
            trace_require_module: String::new(),
            extra_info_on_fatal_exception: true,
            unhandled_rejections: String::new(),
            userland_loaders: Vec::new(),
            verify_base_objects: false,
            watch_mode: false,
            watch_mode_report_to_parent: false,
            watch_mode_preserve_output: false,
            watch_mode_kill_signal: "SIGTERM".to_string(),
            watch_mode_paths: Vec::new(),
            syntax_check_only: false,
            has_eval_string: false,
            eval_string: String::new(),
            print_eval: false,
            force_repl: false,
            insecure_http_parser: false,
            tls_min_v1_0: false,
            tls_min_v1_1: false,
            tls_min_v1_2: false,
            tls_min_v1_3: false,
            tls_max_v1_2: false,
            tls_max_v1_3: false,
            tls_keylog: String::new(),
            preload_cjs_modules: Vec::new(),
            preload_esm_modules: Vec::new(),
            experimental_strip_types: true,
            experimental_transform_types: false,
            user_argv: Vec::new(),
            report_exclude_env: false,
            report_exclude_network: false,
            experimental_config_file_path: String::new(),
            experimental_default_config_file: false,
            debug_options: DebugOptions::default(),
        }
    }
}

impl EnvironmentOptions {
    pub fn check_options(&mut self, errors: &mut Vec<String>) {
        if !self.input_type.is_empty()
            && !matches!(
                self.input_type.as_str(),
                "commonjs" | "module" | "commonjs-typescript" | "module-typescript"
            )
        {
            errors.push("--input-type must be \"module\", \"commonjs\", \"module-typescript\" or \"commonjs-typescript\"".to_string());
        }

        if self.syntax_check_only && self.has_eval_string {
            errors.push("either --check or --eval can be used, not both".to_string());
        }

        if !self.unhandled_rejections.is_empty()
            && !matches!(
                self.unhandled_rejections.as_str(),
                "warn-with-error-code" | "throw" | "strict" | "warn" | "none"
            )
        {
            errors.push("invalid value for --unhandled-rejections".to_string());
        }

        if self.tls_min_v1_3 && self.tls_max_v1_2 {
            errors
                .push("either --tls-min-v1.3 or --tls-max-v1.2 can be used, not both".to_string());
        }

        if self.heap_snapshot_near_heap_limit < 0 {
            errors.push("--heapsnapshot-near-heap-limit must not be negative".to_string());
        }

        if !self.trace_require_module.is_empty()
            && !matches!(
                self.trace_require_module.as_str(),
                "all" | "no-node-modules"
            )
        {
            errors.push("invalid value for --trace-require-module".to_string());
        }

        if self.test_runner {
            if self.test_isolation == "none" {
                self.debug_options.allow_attaching_debugger = true;
            } else if self.test_isolation != "process" {
                errors.push("invalid value for --test-isolation".to_string());
            }

            if self.syntax_check_only {
                errors.push("either --test or --check can be used, not both".to_string());
            }

            if self.has_eval_string {
                errors.push("either --test or --eval can be used, not both".to_string());
            }

            if self.force_repl {
                errors.push("either --test or --interactive can be used, not both".to_string());
            }

            if !self.watch_mode_paths.is_empty() {
                errors.push("--watch-path cannot be used in combination with --test".to_string());
            }
        }

        if self.watch_mode {
            if self.syntax_check_only {
                errors.push("either --watch or --check can be used, not both".to_string());
            } else if self.has_eval_string {
                errors.push("either --watch or --eval can be used, not both".to_string());
            } else if self.force_repl {
                errors.push("either --watch or --interactive can be used, not both".to_string());
            } else if self.test_runner_force_exit {
                errors
                    .push("either --watch or --test-force-exit can be used, not both".to_string());
            }

            self.debug_options.allow_attaching_debugger = false;
        }

        // CPU profiling validation
        if !self.cpu_prof {
            if !self.cpu_prof_name.is_empty() {
                errors.push("--cpu-prof-name must be used with --cpu-prof".to_string());
            }
            if !self.cpu_prof_dir.is_empty() {
                errors.push("--cpu-prof-dir must be used with --cpu-prof".to_string());
            }
            if self.cpu_prof_interval != 1000 {
                errors.push("--cpu-prof-interval must be used with --cpu-prof".to_string());
            }
        }

        if self.cpu_prof && self.cpu_prof_dir.is_empty() && !self.diagnostic_dir.is_empty() {
            self.cpu_prof_dir = self.diagnostic_dir.clone();
        }

        // Heap profiling validation
        if !self.heap_prof {
            if !self.heap_prof_name.is_empty() {
                errors.push("--heap-prof-name must be used with --heap-prof".to_string());
            }
            if !self.heap_prof_dir.is_empty() {
                errors.push("--heap-prof-dir must be used with --heap-prof".to_string());
            }
            if self.heap_prof_interval != 512 * 1024 {
                errors.push("--heap-prof-interval must be used with --heap-prof".to_string());
            }
        }

        if self.heap_prof && self.heap_prof_dir.is_empty() && !self.diagnostic_dir.is_empty() {
            self.heap_prof_dir = self.diagnostic_dir.clone();
        }

        self.debug_options.check_options(errors);
    }
}

#[derive(Debug, Clone)]
pub struct PerIsolateOptions {
    pub per_env: EnvironmentOptions,
    pub track_heap_objects: bool,
    pub report_uncaught_exception: bool,
    pub report_on_signal: bool,
    pub experimental_shadow_realm: bool,
    pub stack_trace_limit: i64,
    pub report_signal: String,
    pub build_snapshot: bool,
    pub build_snapshot_config: String,
}

impl Default for PerIsolateOptions {
    fn default() -> Self {
        Self {
            per_env: EnvironmentOptions::default(),
            track_heap_objects: false,
            report_uncaught_exception: false,
            report_on_signal: false,
            experimental_shadow_realm: false,
            stack_trace_limit: 10,
            report_signal: "SIGUSR2".to_string(),
            build_snapshot: false,
            build_snapshot_config: String::new(),
        }
    }
}

impl PerIsolateOptions {
    pub fn check_options(&mut self, errors: &mut Vec<String>) {
        self.per_env.check_options(errors);
    }
}

#[derive(Debug, Clone)]
pub struct PerProcessOptions {
    pub per_isolate: PerIsolateOptions,
    pub title: String,
    pub trace_event_categories: String,
    pub trace_event_file_pattern: String,
    pub v8_thread_pool_size: i64,
    pub zero_fill_all_buffers: bool,
    pub debug_arraybuffer_allocations: bool,
    pub disable_proto: String,
    pub node_snapshot: bool,
    pub snapshot_blob: String,
    pub security_reverts: Vec<String>,
    pub print_bash_completion: bool,
    pub print_help: bool,
    pub print_v8_help: bool,
    pub print_version: bool,
    pub experimental_sea_config: String,
    pub run: String,
    pub icu_data_dir: String,
    pub openssl_config: String,
    pub tls_cipher_list: String,
    pub secure_heap: i64,
    pub secure_heap_min: i64,
    pub ssl_openssl_cert_store: bool,
    pub use_openssl_ca: bool,
    pub use_system_ca: bool,
    pub use_bundled_ca: bool,
    pub enable_fips_crypto: bool,
    pub force_fips_crypto: bool,
    pub openssl_legacy_provider: bool,
    pub openssl_shared_config: bool,
    pub disable_wasm_trap_handler: bool,
    pub report_on_fatalerror: bool,
    pub report_compact: bool,
    pub report_directory: String,
    pub report_filename: String,
    pub use_largepages: String,
    pub trace_sigint: bool,
}

impl Default for PerProcessOptions {
    fn default() -> Self {
        Self {
            per_isolate: PerIsolateOptions::default(),
            title: String::new(),
            trace_event_categories: String::new(),
            trace_event_file_pattern: "node_trace.${rotation}.log".to_string(),
            v8_thread_pool_size: 4,
            zero_fill_all_buffers: false,
            debug_arraybuffer_allocations: false,
            disable_proto: String::new(),
            node_snapshot: true,
            snapshot_blob: String::new(),
            security_reverts: Vec::new(),
            print_bash_completion: false,
            print_help: false,
            print_v8_help: false,
            print_version: false,
            experimental_sea_config: String::new(),
            run: String::new(),
            icu_data_dir: String::new(),
            openssl_config: String::new(),
            tls_cipher_list: "ECDHE+AESGCM:ECDHE+CHACHA20:DHE+AESGCM:DHE+CHACHA20:!aNULL:!MD5:!DSS"
                .to_string(),
            secure_heap: 0,
            secure_heap_min: 2,
            ssl_openssl_cert_store: false,
            use_openssl_ca: false,
            use_system_ca: false,
            use_bundled_ca: false,
            enable_fips_crypto: false,
            force_fips_crypto: false,
            openssl_legacy_provider: false,
            openssl_shared_config: false,
            disable_wasm_trap_handler: false,
            report_on_fatalerror: false,
            report_compact: false,
            report_directory: String::new(),
            report_filename: String::new(),
            use_largepages: "off".to_string(),
            trace_sigint: false,
        }
    }
}

impl PerProcessOptions {
    pub fn check_options(&mut self, errors: &mut Vec<String>) {
        if self.use_openssl_ca && self.use_bundled_ca {
            errors.push(
                "either --use-openssl-ca or --use-bundled-ca can be used, not both".to_string(),
            );
        }

        if self.secure_heap >= 2 {
            if (self.secure_heap & (self.secure_heap - 1)) != 0 {
                errors.push("--secure-heap must be a power of 2".to_string());
            }
            self.secure_heap_min = self
                .secure_heap
                .min(self.secure_heap_min)
                .min(i32::MAX as i64);
            self.secure_heap_min = self.secure_heap_min.max(2);
            if (self.secure_heap_min & (self.secure_heap_min - 1)) != 0 {
                errors.push("--secure-heap-min must be a power of 2".to_string());
            }
        }

        if !matches!(self.use_largepages.as_str(), "off" | "on" | "silent") {
            errors.push("invalid value for --use-largepages".to_string());
        }

        self.per_isolate.check_options(errors);
    }
}

fn remove_brackets(host: &str) -> String {
    if !host.is_empty() && host.starts_with('[') && host.ends_with(']') {
        host[1..host.len() - 1].to_string()
    } else {
        host.to_string()
    }
}

fn parse_and_validate_port(port_str: &str, errors: &mut Vec<String>) -> u16 {
    match port_str.parse::<u16>() {
        Ok(port) => {
            if port != 0 && port < 1024 {
                errors.push("must be 0 or in range 1024 to 65535.".to_string());
                0
            } else {
                port
            }
        }
        Err(_) => {
            errors.push("must be 0 or in range 1024 to 65535.".to_string());
            0
        }
    }
}

fn split_host_port(arg: &str, errors: &mut Vec<String>) -> HostPort {
    let host = remove_brackets(arg);
    if host.len() < arg.len() {
        return HostPort::new(host, 9229);
    }

    if let Some(colon_index) = arg.rfind(':') {
        let host_part = remove_brackets(&arg[..colon_index]);
        let port_part = &arg[colon_index + 1..];
        HostPort::new(host_part, parse_and_validate_port(port_part, errors))
    } else {
        // Either a port number or a host name
        if arg.chars().all(|c| c.is_ascii_digit()) {
            HostPort::new(String::new(), parse_and_validate_port(arg, errors))
        } else {
            HostPort::new(arg.to_string(), 9229)
        }
    }
}

#[derive(Debug, Clone)]
struct OptionInfo {
    option_type: OptionType,
    env_setting: OptionEnvvarSettings,
    help_text: String,
    default_is_true: bool,
}

pub struct OptionsParser {
    options: HashMap<String, OptionInfo>,
    aliases: HashMap<String, Vec<String>>,
    implications: HashMap<String, Vec<String>>,
}

impl OptionsParser {
    pub fn new() -> Self {
        let mut parser = Self {
            options: HashMap::new(),
            aliases: HashMap::new(),
            implications: HashMap::new(),
        };
        parser.setup_options();
        parser
    }

    fn add_option(
        &mut self,
        name: &str,
        help_text: &str,
        option_type: OptionType,
        env_setting: OptionEnvvarSettings,
        default_is_true: bool,
    ) {
        self.options.insert(
            name.to_string(),
            OptionInfo {
                option_type,
                env_setting,
                help_text: help_text.to_string(),
                default_is_true,
            },
        );
    }

    fn add_alias(&mut self, from: &str, to: Vec<&str>) {
        self.aliases
            .insert(from.to_string(), to.iter().map(|s| s.to_string()).collect());
    }

    fn add_implication(&mut self, from: &str, to: Vec<&str>) {
        self.implications
            .insert(from.to_string(), to.iter().map(|s| s.to_string()).collect());
    }

    fn setup_options(&mut self) {
        // Debug options
        self.add_option(
            "--inspect-port",
            "set host:port for inspector",
            OptionType::HostPort,
            OptionEnvvarSettings::AllowedInEnvvar,
            false,
        );
        self.add_option(
            "--inspect",
            "activate inspector on host:port (default: 127.0.0.1:9229)",
            OptionType::Boolean,
            OptionEnvvarSettings::AllowedInEnvvar,
            false,
        );
        self.add_option(
            "--debug",
            "",
            OptionType::Boolean,
            OptionEnvvarSettings::DisallowedInEnvvar,
            false,
        );
        self.add_option(
            "--debug-brk",
            "",
            OptionType::Boolean,
            OptionEnvvarSettings::DisallowedInEnvvar,
            false,
        );
        self.add_option(
            "--inspect-brk",
            "activate inspector on host:port and break at start of user script",
            OptionType::Boolean,
            OptionEnvvarSettings::AllowedInEnvvar,
            false,
        );
        self.add_option(
            "--inspect-brk-node",
            "",
            OptionType::Boolean,
            OptionEnvvarSettings::DisallowedInEnvvar,
            false,
        );
        self.add_option(
            "--inspect-wait",
            "activate inspector on host:port and wait for debugger to be attached",
            OptionType::Boolean,
            OptionEnvvarSettings::AllowedInEnvvar,
            false,
        );
        self.add_option(
            "--inspect-publish-uid",
            "comma separated list of destinations for inspector uid (default: stderr,http)",
            OptionType::String,
            OptionEnvvarSettings::AllowedInEnvvar,
            false,
        );

        // Environment options
        self.add_option(
            "--conditions",
            "additional user conditions for conditional exports and imports",
            OptionType::StringList,
            OptionEnvvarSettings::AllowedInEnvvar,
            false,
        );
        self.add_option("--experimental-detect-module", "when ambiguous modules fail to evaluate because they contain ES module syntax, try again to evaluate them as ES modules", OptionType::Boolean, OptionEnvvarSettings::AllowedInEnvvar, true);
        self.add_option("--experimental-print-required-tla", "Print pending top-level await. If --experimental-require-module is true, evaluate asynchronous graphs loaded by `require()` but do not run the microtasks, in order to to find and print top-level await in the graph", OptionType::Boolean, OptionEnvvarSettings::AllowedInEnvvar, false);
        self.add_option(
            "--experimental-require-module",
            "Allow loading synchronous ES Modules in require().",
            OptionType::Boolean,
            OptionEnvvarSettings::AllowedInEnvvar,
            true,
        );
        self.add_option(
            "--diagnostic-dir",
            "set dir for all output files (default: current working directory)",
            OptionType::String,
            OptionEnvvarSettings::AllowedInEnvvar,
            false,
        );
        self.add_option(
            "--disable-sigusr1",
            "Disable inspector thread to be listening for SIGUSR1 signal",
            OptionType::Boolean,
            OptionEnvvarSettings::AllowedInEnvvar,
            false,
        );
        self.add_option("--dns-result-order", "set default value of verbatim in dns.lookup. Options are 'ipv4first' (IPv4 addresses are placed before IPv6 addresses) 'ipv6first' (IPv6 addresses are placed before IPv4 addresses) 'verbatim' (addresses are in the order the DNS resolver returned)", OptionType::String, OptionEnvvarSettings::AllowedInEnvvar, false);
        self.add_option(
            "--network-family-autoselection",
            "Disable network address family autodetection algorithm",
            OptionType::Boolean,
            OptionEnvvarSettings::AllowedInEnvvar,
            true,
        );
        self.add_option(
            "--network-family-autoselection-attempt-timeout",
            "Sets the default value for the network family autoselection attempt timeout.",
            OptionType::UInteger,
            OptionEnvvarSettings::AllowedInEnvvar,
            false,
        );
        self.add_option(
            "--enable-source-maps",
            "Source Map V3 support for stack traces",
            OptionType::Boolean,
            OptionEnvvarSettings::AllowedInEnvvar,
            false,
        );
        self.add_option(
            "--entry-url",
            "Treat the entrypoint as a URL",
            OptionType::Boolean,
            OptionEnvvarSettings::AllowedInEnvvar,
            false,
        );
        self.add_option(
            "--experimental-addon-modules",
            "experimental import support for addons",
            OptionType::Boolean,
            OptionEnvvarSettings::AllowedInEnvvar,
            false,
        );
        self.add_option(
            "--experimental-abortcontroller",
            "",
            OptionType::NoOp,
            OptionEnvvarSettings::AllowedInEnvvar,
            false,
        );
        self.add_option(
            "--experimental-eventsource",
            "experimental EventSource API",
            OptionType::Boolean,
            OptionEnvvarSettings::AllowedInEnvvar,
            false,
        );
        self.add_option(
            "--experimental-fetch",
            "",
            OptionType::NoOp,
            OptionEnvvarSettings::AllowedInEnvvar,
            false,
        );
        self.add_option(
            "--experimental-websocket",
            "experimental WebSocket API",
            OptionType::Boolean,
            OptionEnvvarSettings::AllowedInEnvvar,
            true,
        );
        self.add_option(
            "--experimental-global-customevent",
            "",
            OptionType::NoOp,
            OptionEnvvarSettings::AllowedInEnvvar,
            false,
        );
        self.add_option(
            "--experimental-sqlite",
            "experimental node:sqlite module",
            OptionType::Boolean,
            OptionEnvvarSettings::AllowedInEnvvar,
            true,
        );
        self.add_option(
            "--experimental-quic",
            "",
            OptionType::Boolean,
            OptionEnvvarSettings::AllowedInEnvvar,
            false,
        );
        self.add_option(
            "--experimental-webstorage",
            "experimental Web Storage API",
            OptionType::Boolean,
            OptionEnvvarSettings::AllowedInEnvvar,
            false,
        );
        self.add_option(
            "--localstorage-file",
            "file used to persist localStorage data",
            OptionType::String,
            OptionEnvvarSettings::AllowedInEnvvar,
            false,
        );
        self.add_option(
            "--experimental-global-navigator",
            "expose experimental Navigator API on the global scope",
            OptionType::Boolean,
            OptionEnvvarSettings::AllowedInEnvvar,
            true,
        );
        self.add_option(
            "--experimental-global-webcrypto",
            "",
            OptionType::NoOp,
            OptionEnvvarSettings::AllowedInEnvvar,
            false,
        );
        self.add_option(
            "--experimental-json-modules",
            "",
            OptionType::NoOp,
            OptionEnvvarSettings::AllowedInEnvvar,
            false,
        );
        self.add_option(
            "--experimental-loader",
            "use the specified module as a custom loader",
            OptionType::StringList,
            OptionEnvvarSettings::AllowedInEnvvar,
            false,
        );
        self.add_option(
            "--experimental-modules",
            "",
            OptionType::NoOp,
            OptionEnvvarSettings::AllowedInEnvvar,
            false,
        );
        self.add_option(
            "--experimental-wasm-modules",
            "experimental ES Module support for webassembly modules",
            OptionType::Boolean,
            OptionEnvvarSettings::AllowedInEnvvar,
            false,
        );
        self.add_option(
            "--experimental-import-meta-resolve",
            "experimental ES Module import.meta.resolve() parentURL support",
            OptionType::Boolean,
            OptionEnvvarSettings::AllowedInEnvvar,
            false,
        );
        self.add_option(
            "--permission",
            "enable the permission system",
            OptionType::Boolean,
            OptionEnvvarSettings::AllowedInEnvvar,
            false,
        );
        self.add_option(
            "--allow-fs-read",
            "allow permissions to read the filesystem",
            OptionType::StringList,
            OptionEnvvarSettings::AllowedInEnvvar,
            false,
        );
        self.add_option(
            "--allow-fs-write",
            "allow permissions to write in the filesystem",
            OptionType::StringList,
            OptionEnvvarSettings::AllowedInEnvvar,
            false,
        );
        self.add_option(
            "--allow-addons",
            "allow use of addons when any permissions are set",
            OptionType::Boolean,
            OptionEnvvarSettings::AllowedInEnvvar,
            false,
        );
        self.add_option(
            "--allow-child-process",
            "allow use of child process when any permissions are set",
            OptionType::Boolean,
            OptionEnvvarSettings::AllowedInEnvvar,
            false,
        );
        self.add_option(
            "--allow-net",
            "allow use of network when any permissions are set",
            OptionType::StringList,
            OptionEnvvarSettings::AllowedInEnvvar,
            false,
        );
        self.add_option(
            "--allow-wasi",
            "allow wasi when any permissions are set",
            OptionType::Boolean,
            OptionEnvvarSettings::AllowedInEnvvar,
            false,
        );
        self.add_option(
            "--allow-worker",
            "allow worker threads when any permissions are set",
            OptionType::Boolean,
            OptionEnvvarSettings::AllowedInEnvvar,
            false,
        );
        self.add_option(
            "--experimental-repl-await",
            "experimental await keyword support in REPL",
            OptionType::Boolean,
            OptionEnvvarSettings::AllowedInEnvvar,
            true,
        );
        self.add_option(
            "--experimental-vm-modules",
            "experimental ES Module support in vm module",
            OptionType::Boolean,
            OptionEnvvarSettings::AllowedInEnvvar,
            false,
        );
        self.add_option(
            "--experimental-worker",
            "",
            OptionType::NoOp,
            OptionEnvvarSettings::AllowedInEnvvar,
            false,
        );
        self.add_option(
            "--experimental-report",
            "",
            OptionType::NoOp,
            OptionEnvvarSettings::AllowedInEnvvar,
            false,
        );
        self.add_option(
            "--experimental-wasi-unstable-preview1",
            "",
            OptionType::NoOp,
            OptionEnvvarSettings::AllowedInEnvvar,
            false,
        );
        self.add_option(
            "--expose-gc",
            "expose gc extension",
            OptionType::V8Option,
            OptionEnvvarSettings::AllowedInEnvvar,
            false,
        );
        self.add_option(
            "--async-context-frame",
            "Improve AsyncLocalStorage performance with AsyncContextFrame",
            OptionType::Boolean,
            OptionEnvvarSettings::AllowedInEnvvar,
            true,
        );
        self.add_option(
            "--expose-internals",
            "",
            OptionType::Boolean,
            OptionEnvvarSettings::DisallowedInEnvvar,
            false,
        );
        self.add_option(
            "--frozen-intrinsics",
            "experimental frozen intrinsics support",
            OptionType::Boolean,
            OptionEnvvarSettings::AllowedInEnvvar,
            false,
        );
        self.add_option(
            "--heapsnapshot-signal",
            "Generate heap snapshot on specified signal",
            OptionType::String,
            OptionEnvvarSettings::AllowedInEnvvar,
            false,
        );
        self.add_option("--heapsnapshot-near-heap-limit", "Generate heap snapshots whenever V8 is approaching the heap limit. No more than the specified number of heap snapshots will be generated.", OptionType::Integer, OptionEnvvarSettings::AllowedInEnvvar, false);
        self.add_option(
            "--http-parser",
            "",
            OptionType::NoOp,
            OptionEnvvarSettings::AllowedInEnvvar,
            false,
        );
        self.add_option(
            "--insecure-http-parser",
            "use an insecure HTTP parser that accepts invalid HTTP headers",
            OptionType::Boolean,
            OptionEnvvarSettings::AllowedInEnvvar,
            false,
        );
        self.add_option(
            "--input-type",
            "set module type for string input",
            OptionType::String,
            OptionEnvvarSettings::AllowedInEnvvar,
            false,
        );
        self.add_option(
            "--experimental-specifier-resolution",
            "",
            OptionType::NoOp,
            OptionEnvvarSettings::AllowedInEnvvar,
            false,
        );
        self.add_option(
            "--deprecation",
            "silence deprecation warnings",
            OptionType::Boolean,
            OptionEnvvarSettings::AllowedInEnvvar,
            true,
        );
        self.add_option(
            "--force-async-hooks-checks",
            "disable checks for async_hooks",
            OptionType::Boolean,
            OptionEnvvarSettings::AllowedInEnvvar,
            true,
        );
        self.add_option(
            "--force-node-api-uncaught-exceptions-policy",
            "enforces 'uncaughtException' event on Node API asynchronous callbacks",
            OptionType::Boolean,
            OptionEnvvarSettings::AllowedInEnvvar,
            false,
        );
        self.add_option(
            "--addons",
            "disable loading native addons",
            OptionType::Boolean,
            OptionEnvvarSettings::AllowedInEnvvar,
            true,
        );
        self.add_option(
            "--global-search-paths",
            "disable global module search paths",
            OptionType::Boolean,
            OptionEnvvarSettings::AllowedInEnvvar,
            true,
        );
        self.add_option(
            "--warnings",
            "silence all process warnings",
            OptionType::Boolean,
            OptionEnvvarSettings::AllowedInEnvvar,
            true,
        );
        self.add_option(
            "--disable-warning",
            "silence specific process warnings",
            OptionType::StringList,
            OptionEnvvarSettings::AllowedInEnvvar,
            false,
        );
        self.add_option(
            "--force-context-aware",
            "disable loading non-context-aware addons",
            OptionType::Boolean,
            OptionEnvvarSettings::AllowedInEnvvar,
            false,
        );
        self.add_option(
            "--pending-deprecation",
            "emit pending deprecation warnings",
            OptionType::Boolean,
            OptionEnvvarSettings::AllowedInEnvvar,
            false,
        );
        self.add_option(
            "--preserve-symlinks",
            "preserve symbolic links when resolving",
            OptionType::Boolean,
            OptionEnvvarSettings::AllowedInEnvvar,
            false,
        );
        self.add_option(
            "--preserve-symlinks-main",
            "preserve symbolic links when resolving the main module",
            OptionType::Boolean,
            OptionEnvvarSettings::AllowedInEnvvar,
            false,
        );
        self.add_option(
            "--prof",
            "Generate V8 profiler output.",
            OptionType::V8Option,
            OptionEnvvarSettings::DisallowedInEnvvar,
            false,
        );
        self.add_option(
            "--prof-process",
            "process V8 profiler output generated using --prof",
            OptionType::Boolean,
            OptionEnvvarSettings::DisallowedInEnvvar,
            false,
        );
        self.add_option("--cpu-prof", "Start the V8 CPU profiler on start up, and write the CPU profile to disk before exit. If --cpu-prof-dir is not specified, write the profile to the current working directory.", OptionType::Boolean, OptionEnvvarSettings::AllowedInEnvvar, false);
        self.add_option(
            "--cpu-prof-name",
            "specified file name of the V8 CPU profile generated with --cpu-prof",
            OptionType::String,
            OptionEnvvarSettings::AllowedInEnvvar,
            false,
        );
        self.add_option("--cpu-prof-interval", "specified sampling interval in microseconds for the V8 CPU profile generated with --cpu-prof. (default: 1000)", OptionType::UInteger, OptionEnvvarSettings::AllowedInEnvvar, false);
        self.add_option("--cpu-prof-dir", "Directory where the V8 profiles generated by --cpu-prof will be placed. Does not affect --prof.", OptionType::String, OptionEnvvarSettings::AllowedInEnvvar, false);
        self.add_option(
            "--experimental-network-inspection",
            "experimental network inspection support",
            OptionType::Boolean,
            OptionEnvvarSettings::DisallowedInEnvvar,
            false,
        );
        self.add_option(
            "--experimental-worker-inspection",
            "experimental worker inspection support",
            OptionType::Boolean,
            OptionEnvvarSettings::DisallowedInEnvvar,
            false,
        );
        self.add_option("--heap-prof", "Start the V8 heap profiler on start up, and write the heap profile to disk before exit. If --heap-prof-dir is not specified, write the profile to the current working directory.", OptionType::Boolean, OptionEnvvarSettings::AllowedInEnvvar, false);
        self.add_option(
            "--heap-prof-name",
            "specified file name of the V8 heap profile generated with --heap-prof",
            OptionType::String,
            OptionEnvvarSettings::AllowedInEnvvar,
            false,
        );
        self.add_option(
            "--heap-prof-dir",
            "Directory where the V8 heap profiles generated by --heap-prof will be placed.",
            OptionType::String,
            OptionEnvvarSettings::AllowedInEnvvar,
            false,
        );
        self.add_option("--heap-prof-interval", "specified sampling interval in bytes for the V8 heap profile generated with --heap-prof. (default: 512 * 1024)", OptionType::UInteger, OptionEnvvarSettings::AllowedInEnvvar, false);
        self.add_option(
            "--max-http-header-size",
            "set the maximum size of HTTP headers (default: 16384 (16KB))",
            OptionType::UInteger,
            OptionEnvvarSettings::AllowedInEnvvar,
            false,
        );
        self.add_option(
            "--redirect-warnings",
            "write warnings to file instead of stderr",
            OptionType::String,
            OptionEnvvarSettings::AllowedInEnvvar,
            false,
        );
        self.add_option(
            "[has_env_file_string]",
            "",
            OptionType::Boolean,
            OptionEnvvarSettings::DisallowedInEnvvar,
            false,
        );
        self.add_option(
            "--env-file",
            "set environment variables from supplied file",
            OptionType::String,
            OptionEnvvarSettings::DisallowedInEnvvar,
            false,
        );
        self.add_option(
            "--env-file-if-exists",
            "set environment variables from supplied file",
            OptionType::String,
            OptionEnvvarSettings::DisallowedInEnvvar,
            false,
        );
        self.add_option(
            "--experimental-config-file",
            "set config file from supplied file",
            OptionType::String,
            OptionEnvvarSettings::DisallowedInEnvvar,
            false,
        );
        self.add_option(
            "--experimental-default-config-file",
            "set config file from default config file",
            OptionType::Boolean,
            OptionEnvvarSettings::DisallowedInEnvvar,
            false,
        );
        self.add_option(
            "--test",
            "launch test runner on startup",
            OptionType::Boolean,
            OptionEnvvarSettings::DisallowedInEnvvar,
            false,
        );
        self.add_option(
            "--test-concurrency",
            "specify test runner concurrency",
            OptionType::UInteger,
            OptionEnvvarSettings::DisallowedInEnvvar,
            false,
        );
        self.add_option(
            "--test-force-exit",
            "force test runner to exit upon completion",
            OptionType::Boolean,
            OptionEnvvarSettings::DisallowedInEnvvar,
            false,
        );
        self.add_option(
            "--test-timeout",
            "specify test runner timeout",
            OptionType::UInteger,
            OptionEnvvarSettings::DisallowedInEnvvar,
            false,
        );
        self.add_option(
            "--test-update-snapshots",
            "regenerate test snapshots",
            OptionType::Boolean,
            OptionEnvvarSettings::DisallowedInEnvvar,
            false,
        );
        self.add_option(
            "--experimental-test-coverage",
            "enable code coverage in the test runner",
            OptionType::Boolean,
            OptionEnvvarSettings::DisallowedInEnvvar,
            false,
        );
        self.add_option(
            "--test-coverage-branches",
            "the branch coverage minimum threshold",
            OptionType::UInteger,
            OptionEnvvarSettings::AllowedInEnvvar,
            false,
        );
        self.add_option(
            "--test-coverage-functions",
            "the function coverage minimum threshold",
            OptionType::UInteger,
            OptionEnvvarSettings::AllowedInEnvvar,
            false,
        );
        self.add_option(
            "--test-coverage-lines",
            "the line coverage minimum threshold",
            OptionType::UInteger,
            OptionEnvvarSettings::AllowedInEnvvar,
            false,
        );
        self.add_option(
            "--test-isolation",
            "configures the type of test isolation used in the test runner",
            OptionType::String,
            OptionEnvvarSettings::AllowedInEnvvar,
            false,
        );
        self.add_option(
            "--experimental-test-module-mocks",
            "enable module mocking in the test runner",
            OptionType::Boolean,
            OptionEnvvarSettings::DisallowedInEnvvar,
            false,
        );
        self.add_option(
            "--experimental-test-snapshots",
            "",
            OptionType::NoOp,
            OptionEnvvarSettings::DisallowedInEnvvar,
            false,
        );
        self.add_option(
            "--test-name-pattern",
            "run tests whose name matches this regular expression",
            OptionType::StringList,
            OptionEnvvarSettings::AllowedInEnvvar,
            false,
        );
        self.add_option(
            "--test-reporter",
            "report test output using the given reporter",
            OptionType::StringList,
            OptionEnvvarSettings::AllowedInEnvvar,
            false,
        );
        self.add_option(
            "--test-reporter-destination",
            "report given reporter to the given destination",
            OptionType::StringList,
            OptionEnvvarSettings::AllowedInEnvvar,
            false,
        );
        self.add_option(
            "--test-only",
            "run tests with 'only' option set",
            OptionType::Boolean,
            OptionEnvvarSettings::AllowedInEnvvar,
            false,
        );
        self.add_option(
            "--test-shard",
            "run test at specific shard",
            OptionType::String,
            OptionEnvvarSettings::AllowedInEnvvar,
            false,
        );
        self.add_option(
            "--test-skip-pattern",
            "run tests whose name do not match this regular expression",
            OptionType::StringList,
            OptionEnvvarSettings::AllowedInEnvvar,
            false,
        );
        self.add_option(
            "--test-coverage-include",
            "include files in coverage report that match this glob pattern",
            OptionType::StringList,
            OptionEnvvarSettings::AllowedInEnvvar,
            false,
        );
        self.add_option(
            "--test-coverage-exclude",
            "exclude files from coverage report that match this glob pattern",
            OptionType::StringList,
            OptionEnvvarSettings::AllowedInEnvvar,
            false,
        );
        self.add_option(
            "--test-global-setup",
            "specifies the path to the global setup file",
            OptionType::String,
            OptionEnvvarSettings::AllowedInEnvvar,
            false,
        );
        self.add_option(
            "--test-udp-no-try-send",
            "",
            OptionType::Boolean,
            OptionEnvvarSettings::DisallowedInEnvvar,
            false,
        );
        self.add_option(
            "--throw-deprecation",
            "throw an exception on deprecations",
            OptionType::Boolean,
            OptionEnvvarSettings::AllowedInEnvvar,
            false,
        );
        self.add_option(
            "--trace-deprecation",
            "show stack traces on deprecations",
            OptionType::Boolean,
            OptionEnvvarSettings::AllowedInEnvvar,
            false,
        );
        self.add_option(
            "--trace-exit",
            "show stack trace when an environment exits",
            OptionType::Boolean,
            OptionEnvvarSettings::AllowedInEnvvar,
            false,
        );
        self.add_option(
            "--trace-sync-io",
            "show stack trace when use of sync IO is detected after the first tick",
            OptionType::Boolean,
            OptionEnvvarSettings::AllowedInEnvvar,
            false,
        );
        self.add_option(
            "--trace-tls",
            "prints TLS packet trace information to stderr",
            OptionType::Boolean,
            OptionEnvvarSettings::AllowedInEnvvar,
            false,
        );
        self.add_option(
            "--trace-uncaught",
            "show stack traces for the `throw` behind uncaught exceptions",
            OptionType::Boolean,
            OptionEnvvarSettings::AllowedInEnvvar,
            false,
        );
        self.add_option(
            "--trace-warnings",
            "show stack traces on process warnings",
            OptionType::Boolean,
            OptionEnvvarSettings::AllowedInEnvvar,
            false,
        );
        self.add_option(
            "--trace-promises",
            "show stack traces on promise initialization and resolution",
            OptionType::Boolean,
            OptionEnvvarSettings::AllowedInEnvvar,
            false,
        );
        self.add_option(
            "--trace-env",
            "Print accesses to the environment variables",
            OptionType::Boolean,
            OptionEnvvarSettings::AllowedInEnvvar,
            false,
        );
        self.add_option(
            "--trace-env-js-stack",
            "Print accesses to the environment variables and the JavaScript stack trace",
            OptionType::Boolean,
            OptionEnvvarSettings::AllowedInEnvvar,
            false,
        );
        self.add_option(
            "--trace-env-native-stack",
            "Print accesses to the environment variables and the native stack trace",
            OptionType::Boolean,
            OptionEnvvarSettings::AllowedInEnvvar,
            false,
        );
        self.add_option("--trace-require-module", "Print access to require(esm). Options are 'all' (print all usage) and 'no-node-modules' (excluding usage from the node_modules folder)", OptionType::String, OptionEnvvarSettings::AllowedInEnvvar, false);
        self.add_option(
            "--extra-info-on-fatal-exception",
            "hide extra information on fatal exception that causes exit",
            OptionType::Boolean,
            OptionEnvvarSettings::AllowedInEnvvar,
            true,
        );
        self.add_option("--unhandled-rejections", "define unhandled rejections behavior. Options are 'strict' (always raise an error), 'throw' (raise an error unless 'unhandledRejection' hook is set), 'warn' (log a warning), 'none' (silence warnings), 'warn-with-error-code' (log a warning and set exit code 1 unless 'unhandledRejection' hook is set). (default: throw)", OptionType::String, OptionEnvvarSettings::AllowedInEnvvar, false);
        self.add_option(
            "--verify-base-objects",
            "",
            OptionType::Boolean,
            OptionEnvvarSettings::AllowedInEnvvar,
            false,
        );
        self.add_option(
            "--watch",
            "run in watch mode",
            OptionType::Boolean,
            OptionEnvvarSettings::AllowedInEnvvar,
            false,
        );
        self.add_option(
            "--watch-path",
            "path to watch",
            OptionType::StringList,
            OptionEnvvarSettings::AllowedInEnvvar,
            false,
        );
        self.add_option(
            "--watch-kill-signal",
            "kill signal to send to the process on watch mode restarts (default: SIGTERM)",
            OptionType::String,
            OptionEnvvarSettings::AllowedInEnvvar,
            false,
        );
        self.add_option(
            "--watch-preserve-output",
            "preserve outputs on watch mode restart",
            OptionType::Boolean,
            OptionEnvvarSettings::AllowedInEnvvar,
            false,
        );
        self.add_option(
            "--check",
            "syntax check script without executing",
            OptionType::Boolean,
            OptionEnvvarSettings::DisallowedInEnvvar,
            false,
        );
        self.add_option(
            "[has_eval_string]",
            "",
            OptionType::Boolean,
            OptionEnvvarSettings::DisallowedInEnvvar,
            false,
        );
        self.add_option(
            "--eval",
            "evaluate script",
            OptionType::String,
            OptionEnvvarSettings::DisallowedInEnvvar,
            false,
        );
        self.add_option(
            "--print",
            "evaluate script and print result",
            OptionType::Boolean,
            OptionEnvvarSettings::DisallowedInEnvvar,
            false,
        );
        self.add_option(
            "--require",
            "CommonJS module to preload (option can be repeated)",
            OptionType::StringList,
            OptionEnvvarSettings::AllowedInEnvvar,
            false,
        );
        self.add_option(
            "--import",
            "ES module to preload (option can be repeated)",
            OptionType::StringList,
            OptionEnvvarSettings::AllowedInEnvvar,
            false,
        );
        self.add_option(
            "--experimental-strip-types",
            "Experimental type-stripping for TypeScript files.",
            OptionType::Boolean,
            OptionEnvvarSettings::AllowedInEnvvar,
            true,
        );
        self.add_option(
            "--experimental-transform-types",
            "enable transformation of TypeScript-only syntax into JavaScript code",
            OptionType::Boolean,
            OptionEnvvarSettings::AllowedInEnvvar,
            false,
        );
        self.add_option(
            "--interactive",
            "always enter the REPL even if stdin does not appear to be a terminal",
            OptionType::Boolean,
            OptionEnvvarSettings::DisallowedInEnvvar,
            false,
        );
        self.add_option(
            "--napi-modules",
            "",
            OptionType::NoOp,
            OptionEnvvarSettings::AllowedInEnvvar,
            false,
        );
        self.add_option(
            "--tls-keylog",
            "log TLS decryption keys to named file for traffic analysis",
            OptionType::String,
            OptionEnvvarSettings::AllowedInEnvvar,
            false,
        );
        self.add_option(
            "--tls-min-v1.0",
            "set default TLS minimum to TLSv1.0 (default: TLSv1.2)",
            OptionType::Boolean,
            OptionEnvvarSettings::AllowedInEnvvar,
            false,
        );
        self.add_option(
            "--tls-min-v1.1",
            "set default TLS minimum to TLSv1.1 (default: TLSv1.2)",
            OptionType::Boolean,
            OptionEnvvarSettings::AllowedInEnvvar,
            false,
        );
        self.add_option(
            "--tls-min-v1.2",
            "set default TLS minimum to TLSv1.2 (default: TLSv1.2)",
            OptionType::Boolean,
            OptionEnvvarSettings::AllowedInEnvvar,
            false,
        );
        self.add_option(
            "--tls-min-v1.3",
            "set default TLS minimum to TLSv1.3 (default: TLSv1.2)",
            OptionType::Boolean,
            OptionEnvvarSettings::AllowedInEnvvar,
            false,
        );
        self.add_option(
            "--tls-max-v1.2",
            "set default TLS maximum to TLSv1.2 (default: TLSv1.3)",
            OptionType::Boolean,
            OptionEnvvarSettings::AllowedInEnvvar,
            false,
        );
        self.add_option(
            "--tls-max-v1.3",
            "set default TLS maximum to TLSv1.3 (default: TLSv1.3)",
            OptionType::Boolean,
            OptionEnvvarSettings::AllowedInEnvvar,
            false,
        );
        self.add_option(
            "--report-exclude-env",
            "Exclude environment variables when generating report (default: false)",
            OptionType::Boolean,
            OptionEnvvarSettings::AllowedInEnvvar,
            false,
        );
        self.add_option(
            "--report-exclude-network",
            "exclude network interface diagnostics. (default: false)",
            OptionType::Boolean,
            OptionEnvvarSettings::AllowedInEnvvar,
            false,
        );

        // Per-isolate options
        self.add_option(
            "--track-heap-objects",
            "track heap object allocations for heap snapshots",
            OptionType::Boolean,
            OptionEnvvarSettings::AllowedInEnvvar,
            false,
        );
        self.add_option(
            "--abort-on-uncaught-exception",
            "aborting instead of exiting causes a core file to be generated for analysis",
            OptionType::V8Option,
            OptionEnvvarSettings::AllowedInEnvvar,
            false,
        );
        self.add_option(
            "--interpreted-frames-native-stack",
            "help system profilers to translate JavaScript interpreted frames",
            OptionType::V8Option,
            OptionEnvvarSettings::AllowedInEnvvar,
            false,
        );
        self.add_option(
            "--max-old-space-size",
            "",
            OptionType::V8Option,
            OptionEnvvarSettings::AllowedInEnvvar,
            false,
        );
        self.add_option(
            "--max-semi-space-size",
            "",
            OptionType::V8Option,
            OptionEnvvarSettings::AllowedInEnvvar,
            false,
        );
        self.add_option(
            "--perf-basic-prof",
            "",
            OptionType::V8Option,
            OptionEnvvarSettings::AllowedInEnvvar,
            false,
        );
        self.add_option(
            "--perf-basic-prof-only-functions",
            "",
            OptionType::V8Option,
            OptionEnvvarSettings::AllowedInEnvvar,
            false,
        );
        self.add_option(
            "--perf-prof",
            "",
            OptionType::V8Option,
            OptionEnvvarSettings::AllowedInEnvvar,
            false,
        );
        self.add_option(
            "--perf-prof-unwinding-info",
            "",
            OptionType::V8Option,
            OptionEnvvarSettings::AllowedInEnvvar,
            false,
        );
        self.add_option(
            "--stack-trace-limit",
            "",
            OptionType::Integer,
            OptionEnvvarSettings::AllowedInEnvvar,
            false,
        );
        self.add_option(
            "--disallow-code-generation-from-strings",
            "disallow eval and friends",
            OptionType::V8Option,
            OptionEnvvarSettings::AllowedInEnvvar,
            false,
        );
        self.add_option(
            "--jitless",
            "disable runtime allocation of executable memory",
            OptionType::V8Option,
            OptionEnvvarSettings::AllowedInEnvvar,
            false,
        );
        self.add_option(
            "--report-uncaught-exception",
            "generate diagnostic report on uncaught exceptions",
            OptionType::Boolean,
            OptionEnvvarSettings::AllowedInEnvvar,
            false,
        );
        self.add_option(
            "--report-on-signal",
            "generate diagnostic report upon receiving signals",
            OptionType::Boolean,
            OptionEnvvarSettings::AllowedInEnvvar,
            false,
        );
        self.add_option("--report-signal", "causes diagnostic report to be produced on provided signal, unsupported in Windows. (default: SIGUSR2)", OptionType::String, OptionEnvvarSettings::AllowedInEnvvar, false);
        self.add_option(
            "--enable-etw-stack-walking",
            "provides heap data to ETW Windows native tracing",
            OptionType::V8Option,
            OptionEnvvarSettings::AllowedInEnvvar,
            false,
        );
        self.add_option(
            "--experimental-top-level-await",
            "",
            OptionType::NoOp,
            OptionEnvvarSettings::AllowedInEnvvar,
            false,
        );
        self.add_option(
            "--experimental-shadow-realm",
            "",
            OptionType::Boolean,
            OptionEnvvarSettings::AllowedInEnvvar,
            false,
        );
        self.add_option(
            "--harmony-shadow-realm",
            "",
            OptionType::V8Option,
            OptionEnvvarSettings::DisallowedInEnvvar,
            false,
        );
        self.add_option(
            "--build-snapshot",
            "Generate a snapshot blob when the process exits.",
            OptionType::Boolean,
            OptionEnvvarSettings::DisallowedInEnvvar,
            false,
        );
        self.add_option("--build-snapshot-config", "Generate a snapshot blob when the process exits using a JSON configuration in the specified path.", OptionType::String, OptionEnvvarSettings::DisallowedInEnvvar, false);

        // Per-process options
        self.add_option(
            "--title",
            "the process title to use on startup",
            OptionType::String,
            OptionEnvvarSettings::AllowedInEnvvar,
            false,
        );
        self.add_option(
            "--trace-event-categories",
            "comma separated list of trace event categories to record",
            OptionType::String,
            OptionEnvvarSettings::AllowedInEnvvar,
            false,
        );
        self.add_option("--trace-event-file-pattern", "Template string specifying the filepath for the trace-events data, it supports ${rotation} and ${pid}.", OptionType::String, OptionEnvvarSettings::AllowedInEnvvar, false);
        self.add_option(
            "--v8-pool-size",
            "set V8's thread pool size",
            OptionType::Integer,
            OptionEnvvarSettings::AllowedInEnvvar,
            false,
        );
        self.add_option(
            "--zero-fill-buffers",
            "automatically zero-fill all newly allocated Buffer instances",
            OptionType::Boolean,
            OptionEnvvarSettings::AllowedInEnvvar,
            false,
        );
        self.add_option(
            "--debug-arraybuffer-allocations",
            "",
            OptionType::Boolean,
            OptionEnvvarSettings::AllowedInEnvvar,
            false,
        );
        self.add_option(
            "--disable-proto",
            "disable Object.prototype.__proto__",
            OptionType::String,
            OptionEnvvarSettings::AllowedInEnvvar,
            false,
        );
        self.add_option(
            "--node-snapshot",
            "",
            OptionType::Boolean,
            OptionEnvvarSettings::AllowedInEnvvar,
            false,
        );
        self.add_option("--snapshot-blob", "Path to the snapshot blob that's either the result of snapshot building, or the blob that is used to restore the application state", OptionType::String, OptionEnvvarSettings::AllowedInEnvvar, false);
        self.add_option(
            "--security-revert",
            "",
            OptionType::StringList,
            OptionEnvvarSettings::DisallowedInEnvvar,
            false,
        );
        self.add_option(
            "--completion-bash",
            "print source-able bash completion script",
            OptionType::Boolean,
            OptionEnvvarSettings::DisallowedInEnvvar,
            false,
        );
        self.add_option(
            "--help",
            "print node command line options",
            OptionType::Boolean,
            OptionEnvvarSettings::DisallowedInEnvvar,
            false,
        );
        self.add_option(
            "--version",
            "print Node.js version",
            OptionType::Boolean,
            OptionEnvvarSettings::DisallowedInEnvvar,
            false,
        );
        self.add_option(
            "--v8-options",
            "print V8 command line options",
            OptionType::Boolean,
            OptionEnvvarSettings::DisallowedInEnvvar,
            false,
        );
        self.add_option(
            "--report-compact",
            "output compact single-line JSON",
            OptionType::Boolean,
            OptionEnvvarSettings::AllowedInEnvvar,
            false,
        );
        self.add_option(
            "--report-dir",
            "define custom report pathname. (default: current working directory)",
            OptionType::String,
            OptionEnvvarSettings::AllowedInEnvvar,
            false,
        );
        self.add_option(
            "--report-filename",
            "define custom report file name. (default: YYYYMMDD.HHMMSS.PID.SEQUENCE#.txt)",
            OptionType::String,
            OptionEnvvarSettings::AllowedInEnvvar,
            false,
        );
        self.add_option(
            "--report-on-fatalerror",
            "generate diagnostic report on fatal (internal) errors",
            OptionType::Boolean,
            OptionEnvvarSettings::AllowedInEnvvar,
            false,
        );
        self.add_option(
            "--icu-data-dir",
            "set ICU data load path to dir (overrides NODE_ICU_DATA)",
            OptionType::String,
            OptionEnvvarSettings::AllowedInEnvvar,
            false,
        );
        self.add_option(
            "--openssl-config",
            "load OpenSSL configuration from the specified file (overrides OPENSSL_CONF)",
            OptionType::String,
            OptionEnvvarSettings::AllowedInEnvvar,
            false,
        );
        self.add_option(
            "--tls-cipher-list",
            "use an alternative default TLS cipher list",
            OptionType::String,
            OptionEnvvarSettings::AllowedInEnvvar,
            false,
        );
        self.add_option(
            "--use-openssl-ca",
            "use OpenSSL's default CA store",
            OptionType::Boolean,
            OptionEnvvarSettings::AllowedInEnvvar,
            false,
        );
        self.add_option(
            "--use-system-ca",
            "use system's CA store",
            OptionType::Boolean,
            OptionEnvvarSettings::AllowedInEnvvar,
            false,
        );
        self.add_option(
            "--use-bundled-ca",
            "use bundled CA store",
            OptionType::Boolean,
            OptionEnvvarSettings::AllowedInEnvvar,
            false,
        );
        self.add_option(
            "[ssl_openssl_cert_store]",
            "",
            OptionType::Boolean,
            OptionEnvvarSettings::DisallowedInEnvvar,
            false,
        );
        self.add_option(
            "--enable-fips",
            "enable FIPS crypto at startup",
            OptionType::Boolean,
            OptionEnvvarSettings::AllowedInEnvvar,
            false,
        );
        self.add_option(
            "--force-fips",
            "force FIPS crypto (cannot be disabled)",
            OptionType::Boolean,
            OptionEnvvarSettings::AllowedInEnvvar,
            false,
        );
        self.add_option(
            "--secure-heap",
            "total size of the OpenSSL secure heap",
            OptionType::Integer,
            OptionEnvvarSettings::AllowedInEnvvar,
            false,
        );
        self.add_option(
            "--secure-heap-min",
            "minimum allocation size from the OpenSSL secure heap",
            OptionType::Integer,
            OptionEnvvarSettings::AllowedInEnvvar,
            false,
        );
        self.add_option(
            "--openssl-legacy-provider",
            "enable OpenSSL 3.0 legacy provider",
            OptionType::Boolean,
            OptionEnvvarSettings::AllowedInEnvvar,
            false,
        );
        self.add_option(
            "--openssl-shared-config",
            "enable OpenSSL shared configuration",
            OptionType::Boolean,
            OptionEnvvarSettings::AllowedInEnvvar,
            false,
        );
        self.add_option("--use-largepages", "Map the Node.js static code to large pages. Options are 'off' (the default value, meaning do not map), 'on' (map and ignore failure, reporting it to stderr), or 'silent' (map and silently ignore failure)", OptionType::String, OptionEnvvarSettings::AllowedInEnvvar, false);
        self.add_option(
            "--trace-sigint",
            "enable printing JavaScript stacktrace on SIGINT",
            OptionType::Boolean,
            OptionEnvvarSettings::AllowedInEnvvar,
            false,
        );
        self.add_option(
            "--node-memory-debug",
            "Run with extra debug checks for memory leaks in Node.js itself",
            OptionType::NoOp,
            OptionEnvvarSettings::AllowedInEnvvar,
            false,
        );
        self.add_option(
            "--experimental-sea-config",
            "Generate a blob that can be embedded into the single executable application",
            OptionType::String,
            OptionEnvvarSettings::DisallowedInEnvvar,
            false,
        );
        self.add_option(
            "--run",
            "Run a script specified in package.json",
            OptionType::String,
            OptionEnvvarSettings::DisallowedInEnvvar,
            false,
        );
        self.add_option("--disable-wasm-trap-handler", "Disable trap-handler-based WebAssembly bound checks. V8 will insert inline bound checks when compiling WebAssembly which may slow down performance.", OptionType::Boolean, OptionEnvvarSettings::AllowedInEnvvar, false);

        // Setup aliases
        self.add_alias("--debug-port", vec!["--inspect-port"]);
        self.add_alias("--inspect=", vec!["--inspect-port", "--inspect"]);
        self.add_alias("--debug=", vec!["--debug"]);
        self.add_alias("--debug-brk=", vec!["--debug-brk"]);
        self.add_alias("--inspect-brk=", vec!["--inspect-port", "--inspect-brk"]);
        self.add_alias(
            "--inspect-brk-node=",
            vec!["--inspect-port", "--inspect-brk-node"],
        );
        self.add_alias("--inspect-wait=", vec!["--inspect-port", "--inspect-wait"]);
        self.add_alias("-C", vec!["--conditions"]);
        self.add_alias("--loader", vec!["--experimental-loader"]);
        self.add_alias(
            "--enable-network-family-autoselection",
            vec!["--network-family-autoselection"],
        );
        self.add_alias(
            "--es-module-specifier-resolution",
            vec!["--experimental-specifier-resolution"],
        );
        self.add_alias("--prof-process", vec!["--prof-process", "--"]);
        self.add_alias("--experimental-test-isolation", vec!["--test-isolation"]);
        self.add_alias("-c", vec!["--check"]);
        self.add_alias("-e", vec!["--eval"]);
        self.add_alias("--print <arg>", vec!["-pe"]);
        self.add_alias("-pe", vec!["--print", "--eval"]);
        self.add_alias("-p", vec!["--print"]);
        self.add_alias("-r", vec!["--require"]);
        self.add_alias("-i", vec!["--interactive"]);
        self.add_alias("--security-reverts", vec!["--security-revert"]);
        self.add_alias("-h", vec!["--help"]);
        self.add_alias("-v", vec!["--version"]);
        self.add_alias("--report-directory", vec!["--report-dir"]);
        self.add_alias(
            "--trace-events-enabled",
            vec!["--trace-event-categories", "v8,node,node.async_hooks"],
        );

        // Setup implications
        self.add_implication("--inspect-brk", vec!["--inspect"]);
        self.add_implication("--inspect-brk-node", vec!["--inspect"]);
        self.add_implication("--inspect-wait", vec!["--inspect"]);
        self.add_implication("--env-file", vec!["[has_env_file_string]"]);
        self.add_implication("--env-file-if-exists", vec!["[has_env_file_string]"]);
        self.add_implication("--eval", vec!["[has_eval_string]"]);
        self.add_implication(
            "--experimental-transform-types",
            vec!["--experimental-strip-types", "--enable-source-maps"],
        );
        self.add_implication("--watch-path", vec!["--watch"]);
        self.add_implication("--trace-env-js-stack", vec!["--trace-env"]);
        self.add_implication("--trace-env-native-stack", vec!["--trace-env"]);
        self.add_implication("--report-signal", vec!["--report-on-signal"]);
        self.add_implication(
            "--experimental-shadow-realm",
            vec!["--harmony-shadow-realm"],
        );
        self.add_implication(
            "--harmony-shadow-realm",
            vec!["--experimental-shadow-realm"],
        );
        self.add_implication("--build-snapshot-config", vec!["--build-snapshot"]);
        self.add_implication("--use-openssl-ca", vec!["[ssl_openssl_cert_store]"]);
        self.add_implication(
            "--node-memory-debug",
            vec!["--debug-arraybuffer-allocations", "--verify-base-objects"],
        );
    }

    fn apply_option_value(
        &self,
        options: &mut PerProcessOptions,
        name: &str,
        value: &str,
        is_negation: bool,
        option_info: &OptionInfo,
        errors: &mut Vec<String>,
    ) {
        match option_info.option_type {
            OptionType::Boolean => {
                self.set_boolean_field(options, name, !is_negation);
            }
            OptionType::Integer => {
                if let Ok(val) = value.parse::<i64>() {
                    self.set_integer_field(options, name, val);
                } else {
                    errors.push(format!("Invalid integer value for {}: {}", name, value));
                }
            }
            OptionType::UInteger => {
                if let Ok(val) = value.parse::<u64>() {
                    self.set_uinteger_field(options, name, val);
                } else {
                    errors.push(format!(
                        "Invalid unsigned integer value for {}: {}",
                        name, value
                    ));
                }
            }
            OptionType::String => {
                self.set_string_field(options, name, value.to_string());
            }
            OptionType::StringList => {
                self.add_to_string_list_field(options, name, value.to_string());
            }
            OptionType::HostPort => {
                let host_port = split_host_port(value, errors);
                self.set_host_port_field(options, name, host_port);
            }
            OptionType::NoOp | OptionType::V8Option => {
                // No-op or handled elsewhere
            }
        }
    }

    fn set_boolean_field(&self, options: &mut PerProcessOptions, name: &str, value: bool) {
        match name {
            // Debug options
            "--inspect" => options.per_isolate.per_env.debug_options.inspector_enabled = value,
            "--inspect-wait" => options.per_isolate.per_env.debug_options.inspect_wait = value,
            "--debug" | "--debug-brk" => {
                options.per_isolate.per_env.debug_options.deprecated_debug = value
            }
            "--inspect-brk" => options.per_isolate.per_env.debug_options.break_first_line = value,
            "--inspect-brk-node" => {
                options
                    .per_isolate
                    .per_env
                    .debug_options
                    .break_node_first_line = value
            }

            // Environment options
            "--experimental-detect-module" => options.per_isolate.per_env.detect_module = value,
            "--disable-sigusr1" => options.per_isolate.per_env.disable_sigusr1 = value,
            "--experimental-print-required-tla" => {
                options.per_isolate.per_env.print_required_tla = value
            }
            "--experimental-require-module" => options.per_isolate.per_env.require_module = value,
            "--enable-source-maps" => options.per_isolate.per_env.enable_source_maps = value,
            "--entry-url" => options.per_isolate.per_env.entry_is_url = value,
            "--experimental-addon-modules" => {
                options.per_isolate.per_env.experimental_addon_modules = value
            }
            "--experimental-eventsource" => {
                options.per_isolate.per_env.experimental_eventsource = value
            }
            "--experimental-websocket" => {
                options.per_isolate.per_env.experimental_websocket = value
            }
            "--experimental-sqlite" => options.per_isolate.per_env.experimental_sqlite = value,
            "--experimental-quic" => options.per_isolate.per_env.experimental_quic = value,
            "--experimental-webstorage" => {
                options.per_isolate.per_env.experimental_webstorage = value
            }
            "--experimental-global-navigator" => {
                options.per_isolate.per_env.experimental_global_navigator = value
            }
            "--experimental-wasm-modules" => {
                options.per_isolate.per_env.experimental_wasm_modules = value
            }
            "--experimental-import-meta-resolve" => {
                options.per_isolate.per_env.experimental_import_meta_resolve = value
            }
            "--permission" => options.per_isolate.per_env.permission = value,
            "--allow-addons" => options.per_isolate.per_env.allow_addons = value,
            "--allow-child-process" => options.per_isolate.per_env.allow_child_process = value,
            "--allow-wasi" => options.per_isolate.per_env.allow_wasi = value,
            "--allow-worker" => options.per_isolate.per_env.allow_worker_threads = value,
            "--experimental-repl-await" => {
                options.per_isolate.per_env.experimental_repl_await = value
            }
            "--experimental-vm-modules" => {
                options.per_isolate.per_env.experimental_vm_modules = value
            }
            "--async-context-frame" => options.per_isolate.per_env.async_context_frame = value,
            "--expose-internals" => options.per_isolate.per_env.expose_internals = value,
            "--frozen-intrinsics" => options.per_isolate.per_env.frozen_intrinsics = value,
            "--network-family-autoselection" => {
                options.per_isolate.per_env.network_family_autoselection = value
            }
            "--deprecation" => options.per_isolate.per_env.deprecation = value,
            "--force-async-hooks-checks" => {
                options.per_isolate.per_env.force_async_hooks_checks = value
            }
            "--force-node-api-uncaught-exceptions-policy" => {
                options
                    .per_isolate
                    .per_env
                    .force_node_api_uncaught_exceptions_policy = value
            }
            "--addons" => options.per_isolate.per_env.allow_native_addons = value,
            "--global-search-paths" => options.per_isolate.per_env.global_search_paths = value,
            "--warnings" => options.per_isolate.per_env.warnings = value,
            "--force-context-aware" => options.per_isolate.per_env.force_context_aware = value,
            "--pending-deprecation" => options.per_isolate.per_env.pending_deprecation = value,
            "--preserve-symlinks" => options.per_isolate.per_env.preserve_symlinks = value,
            "--preserve-symlinks-main" => {
                options.per_isolate.per_env.preserve_symlinks_main = value
            }
            "--prof-process" => options.per_isolate.per_env.prof_process = value,
            "--cpu-prof" => options.per_isolate.per_env.cpu_prof = value,
            "--experimental-network-inspection" => {
                options.per_isolate.per_env.experimental_network_inspection = value
            }
            "--experimental-worker-inspection" => {
                options.per_isolate.per_env.experimental_worker_inspection = value
            }
            "--heap-prof" => options.per_isolate.per_env.heap_prof = value,
            "--insecure-http-parser" => options.per_isolate.per_env.insecure_http_parser = value,
            "[has_env_file_string]" => options.per_isolate.per_env.has_env_file_string = value,
            "--experimental-default-config-file" => {
                options.per_isolate.per_env.experimental_default_config_file = value
            }
            "--test" => options.per_isolate.per_env.test_runner = value,
            "--test-force-exit" => options.per_isolate.per_env.test_runner_force_exit = value,
            "--test-update-snapshots" => {
                options.per_isolate.per_env.test_runner_update_snapshots = value
            }
            "--experimental-test-coverage" => {
                options.per_isolate.per_env.test_runner_coverage = value
            }
            "--experimental-test-module-mocks" => {
                options.per_isolate.per_env.test_runner_module_mocks = value
            }
            "--test-only" => options.per_isolate.per_env.test_only = value,
            "--test-udp-no-try-send" => options.per_isolate.per_env.test_udp_no_try_send = value,
            "--throw-deprecation" => options.per_isolate.per_env.throw_deprecation = value,
            "--trace-deprecation" => options.per_isolate.per_env.trace_deprecation = value,
            "--trace-exit" => options.per_isolate.per_env.trace_exit = value,
            "--trace-sync-io" => options.per_isolate.per_env.trace_sync_io = value,
            "--trace-tls" => options.per_isolate.per_env.trace_tls = value,
            "--trace-uncaught" => options.per_isolate.per_env.trace_uncaught = value,
            "--trace-warnings" => options.per_isolate.per_env.trace_warnings = value,
            "--trace-promises" => options.per_isolate.per_env.trace_promises = value,
            "--trace-env" => options.per_isolate.per_env.trace_env = value,
            "--trace-env-js-stack" => options.per_isolate.per_env.trace_env_js_stack = value,
            "--trace-env-native-stack" => {
                options.per_isolate.per_env.trace_env_native_stack = value
            }
            "--extra-info-on-fatal-exception" => {
                options.per_isolate.per_env.extra_info_on_fatal_exception = value
            }
            "--verify-base-objects" => options.per_isolate.per_env.verify_base_objects = value,
            "--watch" => options.per_isolate.per_env.watch_mode = value,
            "--watch-preserve-output" => {
                options.per_isolate.per_env.watch_mode_preserve_output = value
            }
            "--check" => options.per_isolate.per_env.syntax_check_only = value,
            "[has_eval_string]" => options.per_isolate.per_env.has_eval_string = value,
            "--print" => options.per_isolate.per_env.print_eval = value,
            "--experimental-strip-types" => {
                options.per_isolate.per_env.experimental_strip_types = value
            }
            "--experimental-transform-types" => {
                options.per_isolate.per_env.experimental_transform_types = value
            }
            "--interactive" => options.per_isolate.per_env.force_repl = value,
            "--tls-min-v1.0" => options.per_isolate.per_env.tls_min_v1_0 = value,
            "--tls-min-v1.1" => options.per_isolate.per_env.tls_min_v1_1 = value,
            "--tls-min-v1.2" => options.per_isolate.per_env.tls_min_v1_2 = value,
            "--tls-min-v1.3" => options.per_isolate.per_env.tls_min_v1_3 = value,
            "--tls-max-v1.2" => options.per_isolate.per_env.tls_max_v1_2 = value,
            "--tls-max-v1.3" => options.per_isolate.per_env.tls_max_v1_3 = value,
            "--report-exclude-env" => options.per_isolate.per_env.report_exclude_env = value,
            "--report-exclude-network" => {
                options.per_isolate.per_env.report_exclude_network = value
            }

            // Per-isolate options
            "--track-heap-objects" => options.per_isolate.track_heap_objects = value,
            "--report-uncaught-exception" => options.per_isolate.report_uncaught_exception = value,
            "--report-on-signal" => options.per_isolate.report_on_signal = value,
            "--experimental-shadow-realm" => options.per_isolate.experimental_shadow_realm = value,
            "--build-snapshot" => options.per_isolate.build_snapshot = value,

            // Per-process options
            "--zero-fill-buffers" => options.zero_fill_all_buffers = value,
            "--debug-arraybuffer-allocations" => options.debug_arraybuffer_allocations = value,
            "--node-snapshot" => options.node_snapshot = value,
            "--completion-bash" => options.print_bash_completion = value,
            "--help" => options.print_help = value,
            "--version" => options.print_version = value,
            "--v8-options" => options.print_v8_help = value,
            "--report-compact" => options.report_compact = value,
            "--report-on-fatalerror" => options.report_on_fatalerror = value,
            "--use-openssl-ca" => options.use_openssl_ca = value,
            "--use-system-ca" => options.use_system_ca = value,
            "--use-bundled-ca" => options.use_bundled_ca = value,
            "[ssl_openssl_cert_store]" => options.ssl_openssl_cert_store = value,
            "--enable-fips" => options.enable_fips_crypto = value,
            "--force-fips" => options.force_fips_crypto = value,
            "--openssl-legacy-provider" => options.openssl_legacy_provider = value,
            "--openssl-shared-config" => options.openssl_shared_config = value,
            "--disable-wasm-trap-handler" => options.disable_wasm_trap_handler = value,
            "--trace-sigint" => options.trace_sigint = value,

            _ => {
                // Unknown boolean option - this is OK, might be a V8 option
            }
        }
    }

    fn set_integer_field(&self, options: &mut PerProcessOptions, name: &str, value: i64) {
        match name {
            "--heapsnapshot-near-heap-limit" => {
                options.per_isolate.per_env.heap_snapshot_near_heap_limit = value
            }
            "--stack-trace-limit" => options.per_isolate.stack_trace_limit = value,
            "--v8-pool-size" => options.v8_thread_pool_size = value,
            "--secure-heap" => options.secure_heap = value,
            "--secure-heap-min" => options.secure_heap_min = value,
            _ => {
                // Unknown integer option
            }
        }
    }

    fn set_uinteger_field(&self, options: &mut PerProcessOptions, name: &str, value: u64) {
        match name {
            "--network-family-autoselection-attempt-timeout" => {
                options
                    .per_isolate
                    .per_env
                    .network_family_autoselection_attempt_timeout = value
            }
            "--max-http-header-size" => options.per_isolate.per_env.max_http_header_size = value,
            "--cpu-prof-interval" => options.per_isolate.per_env.cpu_prof_interval = value,
            "--heap-prof-interval" => options.per_isolate.per_env.heap_prof_interval = value,
            "--test-concurrency" => options.per_isolate.per_env.test_runner_concurrency = value,
            "--test-timeout" => options.per_isolate.per_env.test_runner_timeout = value,
            "--test-coverage-branches" => {
                options.per_isolate.per_env.test_coverage_branches = value
            }
            "--test-coverage-functions" => {
                options.per_isolate.per_env.test_coverage_functions = value
            }
            "--test-coverage-lines" => options.per_isolate.per_env.test_coverage_lines = value,
            _ => {
                // Unknown unsigned integer option
            }
        }
    }

    fn set_string_field(&self, options: &mut PerProcessOptions, name: &str, value: String) {
        match name {
            // Debug options
            "--inspect-publish-uid" => {
                options
                    .per_isolate
                    .per_env
                    .debug_options
                    .inspect_publish_uid_string = value
            }

            // Environment options
            "--dns-result-order" => options.per_isolate.per_env.dns_result_order = value,
            "--diagnostic-dir" => options.per_isolate.per_env.diagnostic_dir = value,
            "--localstorage-file" => options.per_isolate.per_env.localstorage_file = value,
            "--input-type" => options.per_isolate.per_env.input_type = value,
            "--heapsnapshot-signal" => options.per_isolate.per_env.heapsnapshot_signal = value,
            "--cpu-prof-name" => options.per_isolate.per_env.cpu_prof_name = value,
            "--cpu-prof-dir" => options.per_isolate.per_env.cpu_prof_dir = value,
            "--heap-prof-name" => options.per_isolate.per_env.heap_prof_name = value,
            "--heap-prof-dir" => options.per_isolate.per_env.heap_prof_dir = value,
            "--redirect-warnings" => options.per_isolate.per_env.redirect_warnings = value,
            "--env-file" => options.per_isolate.per_env.env_file = value,
            "--env-file-if-exists" => options.per_isolate.per_env.optional_env_file = value,
            "--experimental-config-file" => {
                options.per_isolate.per_env.experimental_config_file_path = value
            }
            "--test-isolation" => options.per_isolate.per_env.test_isolation = value,
            "--test-global-setup" => options.per_isolate.per_env.test_global_setup_path = value,
            "--test-shard" => options.per_isolate.per_env.test_shard = value,
            "--trace-require-module" => options.per_isolate.per_env.trace_require_module = value,
            "--unhandled-rejections" => options.per_isolate.per_env.unhandled_rejections = value,
            "--watch-kill-signal" => options.per_isolate.per_env.watch_mode_kill_signal = value,
            "--eval" => options.per_isolate.per_env.eval_string = value,
            "--tls-keylog" => options.per_isolate.per_env.tls_keylog = value,

            // Per-isolate options
            "--report-signal" => options.per_isolate.report_signal = value,
            "--build-snapshot-config" => options.per_isolate.build_snapshot_config = value,

            // Per-process options
            "--title" => options.title = value,
            "--trace-event-categories" => options.trace_event_categories = value,
            "--trace-event-file-pattern" => options.trace_event_file_pattern = value,
            "--disable-proto" => options.disable_proto = value,
            "--snapshot-blob" => options.snapshot_blob = value,
            "--experimental-sea-config" => options.experimental_sea_config = value,
            "--run" => options.run = value,
            "--icu-data-dir" => options.icu_data_dir = value,
            "--openssl-config" => options.openssl_config = value,
            "--tls-cipher-list" => options.tls_cipher_list = value,
            "--report-dir" => options.report_directory = value,
            "--report-filename" => options.report_filename = value,
            "--use-largepages" => options.use_largepages = value,

            _ => {
                // Unknown string option
            }
        }
    }

    fn add_to_string_list_field(&self, options: &mut PerProcessOptions, name: &str, value: String) {
        match name {
            "--conditions" => options.per_isolate.per_env.conditions.push(value),
            "--allow-fs-read" => options.per_isolate.per_env.allow_fs_read.push(value),
            "--allow-fs-write" => options.per_isolate.per_env.allow_fs_write.push(value),
            "--allow-net" => options.per_isolate.per_env.allow_net.push(value),
            "--experimental-loader" => options.per_isolate.per_env.userland_loaders.push(value),
            "--disable-warning" => options.per_isolate.per_env.disable_warnings.push(value),
            "--test-name-pattern" => options.per_isolate.per_env.test_name_pattern.push(value),
            "--test-reporter" => options.per_isolate.per_env.test_reporter.push(value),
            "--test-reporter-destination" => options
                .per_isolate
                .per_env
                .test_reporter_destination
                .push(value),
            "--test-skip-pattern" => options.per_isolate.per_env.test_skip_pattern.push(value),
            "--test-coverage-include" => options
                .per_isolate
                .per_env
                .coverage_include_pattern
                .push(value),
            "--test-coverage-exclude" => options
                .per_isolate
                .per_env
                .coverage_exclude_pattern
                .push(value),
            "--watch-path" => options.per_isolate.per_env.watch_mode_paths.push(value),
            "--require" => options.per_isolate.per_env.preload_cjs_modules.push(value),
            "--import" => options.per_isolate.per_env.preload_esm_modules.push(value),
            "--security-revert" => options.security_reverts.push(value),
            _ => {
                // Unknown string list option
            }
        }
    }

    fn set_host_port_field(&self, options: &mut PerProcessOptions, name: &str, value: HostPort) {
        match name {
            "--inspect-port" => options
                .per_isolate
                .per_env
                .debug_options
                .host_port
                .update(&value),
            _ => {
                // Unknown host port option
            }
        }
    }

    pub fn parse(&self, args: Vec<String>) -> Result<ParseResult, Vec<String>> {
        let mut args = args;

        let mut v8_args = Vec::new();
        let mut errors = Vec::new();
        let mut options = PerProcessOptions::default();
        let mut synthetic_args = Vec::new();

        // The args does not contain the executable name, so we do not need to skip it.
        let mut i = 0;

        while i < args.len() + synthetic_args.len() && errors.is_empty() {
            let arg = if !synthetic_args.is_empty() {
                synthetic_args.remove(0)
            } else {
                if i >= args.len() {
                    break;
                }
                let arg = args[i].clone();
                i += 1;
                arg
            };

            if arg.len() <= 1 || !arg.starts_with('-') {
                // Not an option, stop processing
                if synthetic_args.is_empty() {
                    i -= 1; // Put it back
                }
                break;
            }

            if arg == "--" {
                break;
            }

            let (name, value, has_equals) = if arg.starts_with("--") {
                if let Some(eq_pos) = arg.find('=') {
                    (
                        arg[..eq_pos].to_string(),
                        Some(arg[eq_pos + 1..].to_string()),
                        true,
                    )
                } else {
                    (arg.clone(), None, false)
                }
            } else {
                (arg.clone(), None, false)
            };

            let original_name = if has_equals {
                name.clone() + "="
            } else {
                name.clone()
            };

            // Normalize underscores to dashes
            let mut normalized_name = name.clone();
            if normalized_name.starts_with("--") {
                normalized_name = normalized_name.replace('_', "-");
            }

            // Handle negation
            let (is_negation, final_name) =
                if let Some(stripped) = normalized_name.strip_prefix("--no-") {
                    (true, "--".to_string() + stripped)
                } else {
                    (false, normalized_name)
                };

            // Expand aliases
            let mut current_name = final_name.clone();
            let mut expansion_count = 0;
            while expansion_count < 10 && current_name != "--" {
                if let Some(alias_expansion) = self.aliases.get(&current_name) {
                    if !alias_expansion.is_empty() {
                        let new_name = alias_expansion[0].clone();
                        // Stop if alias expands to itself (e.g., --prof-process -> [--prof-process, --])
                        if new_name == current_name {
                            if alias_expansion.len() > 1 {
                                for item in alias_expansion[1..].iter().rev() {
                                    synthetic_args.insert(0, item.clone());
                                }
                            }
                            break;
                        }
                        current_name = new_name;
                        if alias_expansion.len() > 1 {
                            for item in alias_expansion[1..].iter().rev() {
                                synthetic_args.insert(0, item.clone());
                            }
                        }
                        expansion_count += 1;
                    } else {
                        break;
                    }
                } else if has_equals {
                    if let Some(alias_expansion) = self.aliases.get(&(current_name.clone() + "=")) {
                        if !alias_expansion.is_empty() {
                            let new_name = alias_expansion[0].clone();
                            // Stop if alias expands to itself
                            if new_name == current_name {
                                if alias_expansion.len() > 1 {
                                    for item in alias_expansion[1..].iter().rev() {
                                        synthetic_args.insert(0, item.clone());
                                    }
                                }
                                break;
                            }
                            current_name = new_name;
                            if alias_expansion.len() > 1 {
                                for item in alias_expansion[1..].iter().rev() {
                                    synthetic_args.insert(0, item.clone());
                                }
                            }
                            expansion_count += 1;
                        } else {
                            break;
                        }
                    } else {
                        break;
                    }
                } else {
                    break;
                }
            }

            // Handle implications
            let implied_name = if is_negation {
                "--no-".to_string() + &current_name[2..]
            } else {
                current_name.clone()
            };

            if let Some(implications) = self.implications.get(&implied_name) {
                for implication in implications {
                    if implication.starts_with("--") {
                        if let Some(stripped) = implication.strip_prefix("--no-") {
                            let target_name = "--".to_string() + stripped;
                            if let Some(option_info) = self.options.get(&target_name)
                                && option_info.option_type == OptionType::Boolean
                            {
                                self.set_boolean_field(&mut options, &target_name, false);
                            }
                        } else {
                            // Check if it's a boolean option we handle
                            if let Some(option_info) = self.options.get(implication) {
                                if option_info.option_type == OptionType::Boolean {
                                    self.set_boolean_field(&mut options, implication, true);
                                } else {
                                    v8_args.push(implication.clone());
                                }
                            } else {
                                v8_args.push(implication.clone());
                            }
                        }
                    } else {
                        // Handle special implications like [has_eval_string]
                        if let Some(option_info) = self.options.get(implication)
                            && option_info.option_type == OptionType::Boolean
                        {
                            self.set_boolean_field(&mut options, implication, true);
                        }
                    }
                }
            }

            // Check if option exists
            if let Some(option_info) = self.options.get(&current_name) {
                // Validate negation
                if is_negation
                    && option_info.option_type != OptionType::Boolean
                    && option_info.option_type != OptionType::V8Option
                {
                    errors.push(format!(
                        "{} is an invalid negation because it is not a boolean option",
                        arg
                    ));
                    break;
                }

                // Get value for non-boolean options
                let option_value = if matches!(
                    option_info.option_type,
                    OptionType::Boolean | OptionType::NoOp | OptionType::V8Option
                ) {
                    String::new()
                } else if let Some(val) = value {
                    if val.is_empty() {
                        errors.push(format!("{} requires an argument", original_name));
                        break;
                    }
                    val
                } else {
                    // Need to get next argument
                    let next_val = if !synthetic_args.is_empty() {
                        synthetic_args.remove(0)
                    } else if i < args.len() {
                        let val = args[i].clone();
                        i += 1;
                        val
                    } else {
                        errors.push(format!("{} requires an argument", original_name));
                        break;
                    };

                    if next_val.starts_with('-')
                        && (next_val.len() == 1
                            || !next_val[1..].chars().all(|c| c.is_ascii_digit()))
                    {
                        errors.push(format!("{} requires an argument", original_name));
                        break;
                    }

                    // Handle escaped dash
                    if next_val.starts_with("\\-") {
                        next_val[1..].to_string()
                    } else {
                        next_val
                    }
                };

                // Apply option
                match option_info.option_type {
                    OptionType::V8Option => {
                        v8_args.push(arg);
                    }
                    _ => {
                        self.apply_option_value(
                            &mut options,
                            &current_name,
                            &option_value,
                            is_negation,
                            option_info,
                            &mut errors,
                        );
                    }
                }
            } else {
                // Unknown option, pass to V8
                v8_args.push(arg);
            }
        }

        // Remove processed arguments from original args array
        args.drain(0..i);
        args.splice(0..0, synthetic_args);

        // Watch mode validation - check if --watch is used without files and not in test mode
        if options.per_isolate.per_env.watch_mode
            && !options.per_isolate.per_env.test_runner
            && args.is_empty()
        {
            errors.push("--watch requires specifying a file".to_string());
        }

        // Run option validation
        options.check_options(&mut errors);

        if errors.is_empty() {
            Ok(ParseResult {
                options,
                remaining_args: args,
                v8_args,
            })
        } else {
            Err(errors)
        }
    }
}

#[derive(Debug, Clone)]
pub struct ParseResult {
    pub options: PerProcessOptions,
    pub remaining_args: Vec<String>,
    pub v8_args: Vec<String>,
}

/// Parse NODE_OPTIONS environment variable
pub fn parse_node_options_env_var(node_options: &str) -> Result<Vec<String>, Vec<String>> {
    let mut env_argv = Vec::new();
    let mut errors = Vec::new();
    let mut is_in_string = false;
    let mut will_start_new_arg = true;

    let chars: Vec<char> = node_options.chars().collect();
    let mut index = 0;

    while index < chars.len() {
        let mut c = chars[index];

        // Backslashes escape the following character
        if c == '\\' && is_in_string {
            if index + 1 == chars.len() {
                errors.push("invalid value for NODE_OPTIONS (invalid escape)".to_string());
                return Err(errors);
            } else {
                index += 1;
                c = chars[index];
            }
        } else if c == ' ' && !is_in_string {
            will_start_new_arg = true;
            index += 1;
            continue;
        } else if c == '"' {
            is_in_string = !is_in_string;
            index += 1;
            continue;
        }

        if will_start_new_arg {
            env_argv.push(c.to_string());
            will_start_new_arg = false;
        } else if let Some(last) = env_argv.last_mut() {
            last.push(c);
        }

        index += 1;
    }

    if is_in_string {
        errors.push("invalid value for NODE_OPTIONS (unterminated string)".to_string());
        return Err(errors);
    }

    Ok(env_argv)
}

/// Main parsing function
pub fn parse_args(args: Vec<String>) -> Result<ParseResult, Vec<String>> {
    let parser = OptionsParser::new();
    parser.parse(args)
}

/// Options for controlling translation behavior
#[derive(Debug, Clone, Default)]
pub struct TranslateOptions {
    /// Use "deno node" as base command (for standalone CLI)
    /// When false, uses "deno run" (for child_process spawning)
    pub use_node_subcommand: bool,
    /// Add unstable flags (--unstable-node-globals, etc.)
    /// Typically true for standalone CLI, false for child_process
    pub add_unstable_flags: bool,
    /// Wrap eval code for Node.js compatibility (builtin modules as globals)
    pub wrap_eval_code: bool,
}

impl TranslateOptions {
    /// Options for standalone node shim CLI
    pub fn for_node_cli() -> Self {
        Self {
            use_node_subcommand: true,
            add_unstable_flags: true,
            wrap_eval_code: false,
        }
    }

    /// Options for child_process spawning within Deno
    pub fn for_child_process() -> Self {
        Self {
            use_node_subcommand: false,
            add_unstable_flags: false,
            wrap_eval_code: true,
        }
    }
}

/// Result of translating Node.js CLI args to Deno args
#[derive(Debug, Clone, Default)]
pub struct TranslatedArgs {
    /// The Deno CLI arguments
    pub deno_args: Vec<String>,
    /// Node options that should be added to NODE_OPTIONS env var
    pub node_options: Vec<String>,
    /// Whether to set DENO_TLS_CA_STORE=system
    pub use_system_ca: bool,
}

/// Wraps eval code for Node.js compatibility.
/// Makes builtin modules available as global variables.
pub fn wrap_eval_code(source_code: &str) -> String {
    // Use serde_json to properly escape the source code
    let json_escaped = serde_json::to_string(source_code).unwrap_or_else(|_| {
        // Fallback: basic escaping
        format!(
            "\"{}\"",
            source_code.replace('\\', "\\\\").replace('"', "\\\"")
        )
    });

    format!(
        r#"(
    globalThis.require = process.getBuiltinModule("module").createRequire(import.meta.url),
    process.getBuiltinModule("module").builtinModules
      .filter((m) => !/\/|crypto|process/.test(m))
      .forEach((m) => {{ globalThis[m] = process.getBuiltinModule(m); }}),
    process.getBuiltinModule("vm").runInThisContext({})
  )"#,
        json_escaped
    )
}

/// Deno subcommands - if the first arg is one of these, pass through unchanged
const DENO_SUBCOMMANDS: &[&str] = &[
    "add",
    "bench",
    "cache",
    "check",
    "compile",
    "completions",
    "coverage",
    "doc",
    "eval",
    "fmt",
    "help",
    "info",
    "init",
    "install",
    "lint",
    "lsp",
    "publish",
    "repl",
    "run",
    "task",
    "tasks",
    "test",
    "types",
    "uninstall",
    "upgrade",
    "vendor",
];

/// Check if a string is a Deno subcommand
pub fn is_deno_subcommand(arg: &str) -> bool {
    DENO_SUBCOMMANDS.contains(&arg)
}

/// Translate parsed Node.js CLI arguments to Deno CLI arguments.
pub fn translate_to_deno_args(
    parsed_args: ParseResult,
    options: &TranslateOptions,
) -> TranslatedArgs {
    let mut result = TranslatedArgs::default();
    let deno_args = &mut result.deno_args;
    let node_options = &mut result.node_options;

    // Check if the args already look like Deno args (e.g., from vitest workers)
    // If the first remaining arg is a Deno subcommand, pass through unchanged
    if let Some(first_arg) = parsed_args.remaining_args.first()
        && is_deno_subcommand(first_arg)
    {
        // Already Deno-style args, return unchanged
        result.deno_args = parsed_args.remaining_args;
        return result;
    }

    let opts = &parsed_args.options;
    let env_opts = &opts.per_isolate.per_env;

    // Check for system CA usage
    if opts.use_system_ca || opts.use_openssl_ca {
        result.use_system_ca = true;
    }

    // Handle --version flag
    if opts.print_version {
        if options.use_node_subcommand {
            deno_args.push("node".to_string());
        }
        deno_args.push("--version".to_string());
        return result;
    }

    // Handle --v8-options flag (print V8 help and exit)
    if opts.print_v8_help {
        if options.use_node_subcommand {
            deno_args.push("node".to_string());
            deno_args.push("run".to_string());
        }
        deno_args.push("--v8-flags=--help".to_string());
        return result;
    }

    // Handle --help flag
    if opts.print_help {
        if options.use_node_subcommand {
            // For CLI, we handle help specially
            deno_args.push("node".to_string());
        }
        deno_args.push("--help".to_string());
        return result;
    }

    // Handle --completion-bash flag (translate to Deno completions)
    if opts.print_bash_completion {
        deno_args.push("completions".to_string());
        deno_args.push("bash".to_string());
        return result;
    }

    // Handle --run flag (run package.json script via deno task)
    if !opts.run.is_empty() {
        if options.use_node_subcommand {
            deno_args.push("node".to_string());
        }
        deno_args.push("task".to_string());
        deno_args.push(opts.run.clone());
        deno_args.extend(parsed_args.remaining_args);
        return result;
    }

    // Handle -e/--eval or -p/--print
    // Note: -p/--print alone (without -e) uses the first remaining arg as eval code
    let eval_string_for_print = if !env_opts.has_eval_string
        && env_opts.print_eval
        && !parsed_args.remaining_args.is_empty()
    {
        Some(parsed_args.remaining_args[0].clone())
    } else {
        None
    };

    if env_opts.has_eval_string || eval_string_for_print.is_some() {
        if options.use_node_subcommand {
            deno_args.push("node".to_string());
        }
        deno_args.push("eval".to_string());
        // Note: deno eval has implicit permissions, so we don't add -A

        if options.add_unstable_flags {
            deno_args.push("--unstable-node-globals".to_string());
            deno_args.push("--unstable-bare-node-builtins".to_string());
            deno_args.push("--unstable-detect-cjs".to_string());
            deno_args.push("--node-modules-dir=manual".to_string());
            deno_args.push("--no-config".to_string());
        }

        if env_opts.has_env_file_string {
            if env_opts.env_file.is_empty() {
                deno_args.push("--env-file".to_string());
            } else {
                deno_args.push(format!("--env-file={}", env_opts.env_file));
            }
        }

        if env_opts.print_eval {
            deno_args.push("--print".to_string());
        }

        if !parsed_args.v8_args.is_empty() {
            deno_args.push(format!("--v8-flags={}", parsed_args.v8_args.join(",")));
        }

        // Add conditions and inspector flags for eval
        add_conditions(deno_args, env_opts);
        add_inspector_flags(deno_args, env_opts);

        // Get the eval code from either the explicit eval_string or the first remaining arg (for -p)
        let raw_eval_code = eval_string_for_print
            .as_ref()
            .unwrap_or(&env_opts.eval_string);
        let eval_code = if options.wrap_eval_code {
            wrap_eval_code(raw_eval_code)
        } else {
            raw_eval_code.clone()
        };
        deno_args.push(eval_code);

        if options.use_node_subcommand {
            deno_args.push("--".to_string());
        }
        // For -p with first arg as eval code, skip that arg
        let remaining_args = if eval_string_for_print.is_some() {
            parsed_args.remaining_args[1..].to_vec()
        } else {
            parsed_args.remaining_args
        };
        deno_args.extend(remaining_args);
        return result;
    }

    // Handle --test flag (run tests via deno test)
    if env_opts.test_runner {
        if options.use_node_subcommand {
            deno_args.push("node".to_string());
        }
        deno_args.push("test".to_string());
        deno_args.push("-A".to_string());

        if options.add_unstable_flags {
            deno_args.push("--unstable-node-globals".to_string());
            deno_args.push("--unstable-bare-node-builtins".to_string());
            deno_args.push("--unstable-detect-cjs".to_string());
            deno_args.push("--node-modules-dir=manual".to_string());
            deno_args.push("--no-config".to_string());
        }

        add_common_flags(deno_args, &parsed_args, env_opts);
        deno_args.extend(parsed_args.remaining_args);
        return result;
    }

    // Handle REPL (no arguments or force_repl)
    if parsed_args.remaining_args.is_empty() || env_opts.force_repl {
        if options.use_node_subcommand {
            deno_args.push("node".to_string());
            deno_args.push("repl".to_string());
            deno_args.push("-A".to_string());

            if !parsed_args.v8_args.is_empty() {
                deno_args.push(format!("--v8-flags={}", parsed_args.v8_args.join(",")));
            }

            add_conditions(deno_args, env_opts);
            add_inspector_flags(deno_args, env_opts);

            deno_args.push("--".to_string());
            deno_args.extend(parsed_args.remaining_args);
        } else {
            // For child_process, return empty args to trigger REPL behavior
            if !parsed_args.v8_args.is_empty() {
                deno_args.push(format!("--v8-flags={}", parsed_args.v8_args.join(",")));
            }
        }
        return result;
    }

    // Handle running a script
    if options.use_node_subcommand {
        deno_args.push("node".to_string());
    }
    deno_args.push("run".to_string());
    deno_args.push("-A".to_string());

    if options.add_unstable_flags {
        deno_args.push("--unstable-node-globals".to_string());
        deno_args.push("--unstable-bare-node-builtins".to_string());
        deno_args.push("--unstable-detect-cjs".to_string());
        deno_args.push("--node-modules-dir=manual".to_string());
        deno_args.push("--no-config".to_string());
    }

    add_common_flags(deno_args, &parsed_args, env_opts);

    // Handle --no-warnings -> --quiet
    if !env_opts.warnings {
        deno_args.push("--quiet".to_string());
        node_options.push("--no-warnings".to_string());
    }

    // Handle --pending-deprecation (pass to NODE_OPTIONS)
    if env_opts.pending_deprecation {
        node_options.push("--pending-deprecation".to_string());
    }

    // Add the script and remaining args
    deno_args.extend(parsed_args.remaining_args);

    result
}

fn add_common_flags(
    deno_args: &mut Vec<String>,
    parsed_args: &ParseResult,
    env_opts: &EnvironmentOptions,
) {
    // Add watch mode if enabled
    if env_opts.watch_mode {
        if env_opts.watch_mode_paths.is_empty() {
            deno_args.push("--watch".to_string());
        } else {
            deno_args.push(format!(
                "--watch={}",
                env_opts
                    .watch_mode_paths
                    .iter()
                    .map(|p| p.replace(',', ",,"))
                    .collect::<Vec<String>>()
                    .join(",")
            ));
        }
    }

    // Add env file if specified
    if env_opts.has_env_file_string {
        if env_opts.env_file.is_empty() {
            deno_args.push("--env-file".to_string());
        } else {
            deno_args.push(format!("--env-file={}", env_opts.env_file));
        }
    }

    // Add V8 flags
    if !parsed_args.v8_args.is_empty() {
        deno_args.push(format!("--v8-flags={}", parsed_args.v8_args.join(",")));
    }

    // Add conditions
    add_conditions(deno_args, env_opts);

    // Add inspector flags
    add_inspector_flags(deno_args, env_opts);
}

fn add_conditions(deno_args: &mut Vec<String>, env_opts: &EnvironmentOptions) {
    if !env_opts.conditions.is_empty() {
        for condition in &env_opts.conditions {
            deno_args.push(format!("--conditions={}", condition));
        }
    }
}

fn add_inspector_flags(deno_args: &mut Vec<String>, env_opts: &EnvironmentOptions) {
    if env_opts.debug_options.inspector_enabled {
        let arg = if env_opts.debug_options.break_first_line {
            "--inspect-brk"
        } else if env_opts.debug_options.inspect_wait {
            "--inspect-wait"
        } else {
            "--inspect"
        };
        deno_args.push(format!(
            "{}={}:{}",
            arg, env_opts.debug_options.host_port.host, env_opts.debug_options.host_port.port
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Macro to create a Vec<String> from string literals
    macro_rules! svec {
        ($($x:expr),* $(,)?) => {
            vec![$($x.to_string()),*]
        };
    }

    #[test]
    fn test_basic_parsing() {
        let result = parse_args(svec!["--version"]).unwrap();
        assert!(result.options.print_version);
    }

    #[test]
    fn test_help_parsing() {
        let result = parse_args(svec!["--help"]).unwrap();
        assert!(result.options.print_help);
    }

    #[test]
    fn test_debug_options() {
        let result = parse_args(svec!["--inspect"]).unwrap();
        assert!(
            result
                .options
                .per_isolate
                .per_env
                .debug_options
                .inspector_enabled
        );
    }

    #[test]
    fn test_string_option() {
        let result = parse_args(svec!["--title", "myapp"]).unwrap();
        assert_eq!(result.options.title, "myapp");
    }

    #[test]
    fn test_boolean_negation() {
        let result = parse_args(svec!["--no-warnings"]).unwrap();
        assert!(!result.options.per_isolate.per_env.warnings);
    }

    #[test]
    fn test_alias_expansion() {
        let result = parse_args(svec!["-v"]).unwrap();
        assert!(result.options.print_version);
    }

    #[test]
    fn test_node_options_parsing() {
        let env_args = parse_node_options_env_var("--inspect --title \"my app\"").unwrap();
        assert_eq!(env_args, vec!["--inspect", "--title", "my app"]);
    }

    #[test]
    fn test_host_port_parsing() {
        let result = parse_args(svec!["--inspect-port", "127.0.0.1:9229"]).unwrap();
        assert_eq!(
            result
                .options
                .per_isolate
                .per_env
                .debug_options
                .host_port
                .host,
            "127.0.0.1"
        );
        assert_eq!(
            result
                .options
                .per_isolate
                .per_env
                .debug_options
                .host_port
                .port,
            9229
        );
    }

    // Tests for incompatible argument combinations
    #[test]
    fn test_check_eval_incompatible() {
        let result = parse_args(svec!["--check", "--eval", "console.log(42)"]);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(
            errors
                .iter()
                .any(|e| e.contains("either --check or --eval can be used, not both"))
        );
    }

    #[test]
    fn test_test_check_incompatible() {
        let result = parse_args(svec!["--test", "--check"]);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(
            errors
                .iter()
                .any(|e| e.contains("either --test or --check can be used, not both"))
        );
    }

    #[test]
    fn test_test_eval_incompatible() {
        let result = parse_args(svec!["--test", "--eval", "console.log(42)"]);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(
            errors
                .iter()
                .any(|e| e.contains("either --test or --eval can be used, not both"))
        );
    }

    #[test]
    fn test_test_interactive_incompatible() {
        let result = parse_args(svec!["--test", "--interactive"]);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(
            errors
                .iter()
                .any(|e| { e.contains("either --test or --interactive can be used, not both") })
        );
    }

    #[test]
    fn test_test_watch_path_incompatible() {
        let result = parse_args(svec!["--test", "--watch-path", "."]);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(
            errors
                .iter()
                .any(|e| { e.contains("--watch-path cannot be used in combination with --test") })
        );
    }

    #[test]
    fn test_watch_check_incompatible() {
        let result = parse_args(svec!["--watch", "--check"]);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(
            errors
                .iter()
                .any(|e| e.contains("either --watch or --check can be used, not both"))
        );
    }

    #[test]
    fn test_watch_eval_incompatible() {
        let result = parse_args(svec!["--watch", "--eval", "console.log(42)"]);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(
            errors
                .iter()
                .any(|e| e.contains("either --watch or --eval can be used, not both"))
        );
    }

    #[test]
    fn test_watch_interactive_incompatible() {
        let result = parse_args(svec!["--watch", "--interactive"]);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(
            errors
                .iter()
                .any(|e| { e.contains("either --watch or --interactive can be used, not both") })
        );
    }

    #[test]
    fn test_watch_test_force_exit_incompatible() {
        let result = parse_args(svec!["--watch", "--test-force-exit"]);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(
            errors.iter().any(|e| {
                e.contains("either --watch or --test-force-exit can be used, not both")
            })
        );
    }

    #[test]
    fn test_tls_min_max_incompatible() {
        let result = parse_args(svec!["--tls-min-v1.3", "--tls-max-v1.2"]);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(
            errors.iter().any(
                |e| e.contains("either --tls-min-v1.3 or --tls-max-v1.2 can be used, not both")
            )
        );
    }

    #[test]
    fn test_openssl_ca_bundled_ca_incompatible() {
        let result = parse_args(svec!["--use-openssl-ca", "--use-bundled-ca"]);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors.iter().any(|e| {
            e.contains("either --use-openssl-ca or --use-bundled-ca can be used, not both")
        }));
    }

    #[test]
    fn test_cpu_prof_name_without_cpu_prof() {
        let result = parse_args(svec!["--cpu-prof-name", "profile.log"]);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(
            errors
                .iter()
                .any(|e| e.contains("--cpu-prof-name must be used with --cpu-prof"))
        );
    }

    #[test]
    fn test_cpu_prof_dir_without_cpu_prof() {
        let result = parse_args(svec!["--cpu-prof-dir", "/tmp"]);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(
            errors
                .iter()
                .any(|e| e.contains("--cpu-prof-dir must be used with --cpu-prof"))
        );
    }

    #[test]
    fn test_cpu_prof_interval_without_cpu_prof() {
        let result = parse_args(svec!["--cpu-prof-interval", "500"]);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(
            errors
                .iter()
                .any(|e| e.contains("--cpu-prof-interval must be used with --cpu-prof"))
        );
    }

    #[test]
    fn test_heap_prof_name_without_heap_prof() {
        let result = parse_args(svec!["--heap-prof-name", "heap.log"]);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(
            errors
                .iter()
                .any(|e| e.contains("--heap-prof-name must be used with --heap-prof"))
        );
    }

    #[test]
    fn test_heap_prof_dir_without_heap_prof() {
        let result = parse_args(svec!["--heap-prof-dir", "/tmp"]);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(
            errors
                .iter()
                .any(|e| e.contains("--heap-prof-dir must be used with --heap-prof"))
        );
    }

    #[test]
    fn test_heap_prof_interval_without_heap_prof() {
        let result = parse_args(svec!["--heap-prof-interval", "1024"]);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(
            errors
                .iter()
                .any(|e| { e.contains("--heap-prof-interval must be used with --heap-prof") })
        );
    }

    #[test]
    fn test_invalid_input_type() {
        let result = parse_args(svec!["--input-type", "invalid"]);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors.iter().any(|e| e.contains("--input-type must be \"module\", \"commonjs\", \"module-typescript\" or \"commonjs-typescript\"")));
    }

    #[test]
    fn test_invalid_unhandled_rejections() {
        let result = parse_args(svec!["--unhandled-rejections", "invalid"]);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(
            errors
                .iter()
                .any(|e| e.contains("invalid value for --unhandled-rejections"))
        );
    }

    #[test]
    fn test_invalid_trace_require_module() {
        let result = parse_args(svec!["--trace-require-module", "invalid"]);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(
            errors
                .iter()
                .any(|e| e.contains("invalid value for --trace-require-module"))
        );
    }

    #[test]
    fn test_invalid_test_isolation() {
        let result = parse_args(svec!["--test", "--test-isolation", "invalid"]);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(
            errors
                .iter()
                .any(|e| e.contains("invalid value for --test-isolation"))
        );
    }

    #[test]
    fn test_invalid_use_largepages() {
        let result = parse_args(svec!["--use-largepages", "invalid"]);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(
            errors
                .iter()
                .any(|e| e.contains("invalid value for --use-largepages"))
        );
    }

    #[test]
    fn test_negative_heapsnapshot_near_heap_limit() {
        let result = parse_args(svec!["--heapsnapshot-near-heap-limit", "-1"]);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(
            errors
                .iter()
                .any(|e| { e.contains("--heapsnapshot-near-heap-limit must not be negative") })
        );
    }

    #[test]
    fn test_secure_heap_not_power_of_two() {
        let result = parse_args(svec!["--secure-heap", "3"]);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(
            errors
                .iter()
                .any(|e| e.contains("--secure-heap must be a power of 2"))
        );
    }

    #[test]
    fn test_secure_heap_min_not_power_of_two() {
        let result = parse_args(svec!["--secure-heap", "4", "--secure-heap-min", "3"]);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(
            errors
                .iter()
                .any(|e| e.contains("--secure-heap-min must be a power of 2"))
        );
    }

    #[test]
    fn test_deprecated_debug_options() {
        let result = parse_args(svec!["--debug"]);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors.iter().any(|e| e.contains("[DEP0062]: `node --debug` and `node --debug-brk` are invalid. Please use `node --inspect` and `node --inspect-brk` instead.")));
    }

    #[test]
    fn test_deprecated_debug_brk_options() {
        let result = parse_args(svec!["--debug-brk"]);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors.iter().any(|e| e.contains("[DEP0062]: `node --debug` and `node --debug-brk` are invalid. Please use `node --inspect` and `node --inspect-brk` instead.")));
    }

    #[test]
    fn test_invalid_inspect_publish_uid() {
        let result = parse_args(svec!["--inspect-publish-uid", "invalid"]);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(
            errors
                .iter()
                .any(|e| { e.contains("--inspect-publish-uid destination can be stderr or http") })
        );
    }

    #[test]
    fn test_watch_requires_file_when_not_test() {
        let result = parse_args(svec!["--watch"]);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(
            errors
                .iter()
                .any(|e| e.contains("--watch requires specifying a file"))
        );
    }

    #[test]
    fn test_invalid_negation_for_non_boolean() {
        let result = parse_args(svec!["--no-title"]);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(
            errors.iter().any(|e| {
                e.contains("is an invalid negation because it is not a boolean option")
            })
        );
    }

    #[test]
    fn test_option_requires_argument() {
        let result = parse_args(svec!["--title"]);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(
            errors
                .iter()
                .any(|e| e.contains("--title requires an argument"))
        );
    }

    #[test]
    fn test_option_with_empty_equals_value() {
        let result = parse_args(svec!["--title="]);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(
            errors
                .iter()
                .any(|e| e.contains("--title= requires an argument"))
        );
    }

    #[test]
    fn test_option_with_dash_as_value() {
        let result = parse_args(svec!["--title", "-"]);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(
            errors
                .iter()
                .any(|e| e.contains("--title requires an argument"))
        );
    }

    #[test]
    fn test_invalid_port_range() {
        let result = parse_args(svec!["--inspect-port", "99999"]);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(
            errors
                .iter()
                .any(|e| e.contains("must be 0 or in range 1024 to 65535"))
        );
    }

    #[test]
    fn test_invalid_port_low() {
        let result = parse_args(svec!["--inspect-port", "500"]);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(
            errors
                .iter()
                .any(|e| e.contains("must be 0 or in range 1024 to 65535"))
        );
    }

    #[test]
    fn test_escaped_dash_in_value() {
        let result = parse_args(svec!["--title", "\\-mytitle"]).unwrap();
        assert_eq!(result.options.title, "-mytitle");
    }

    #[test]
    fn test_compatible_profiling_options() {
        let result = parse_args(svec![
            "--cpu-prof",
            "--cpu-prof-name",
            "profile.log",
            "--cpu-prof-dir",
            "/tmp",
            "--cpu-prof-interval",
            "500"
        ])
        .unwrap();
        assert!(result.options.per_isolate.per_env.cpu_prof);
        assert_eq!(
            result.options.per_isolate.per_env.cpu_prof_name,
            "profile.log"
        );
        assert_eq!(result.options.per_isolate.per_env.cpu_prof_dir, "/tmp");
        assert_eq!(result.options.per_isolate.per_env.cpu_prof_interval, 500);
    }

    #[test]
    fn test_compatible_heap_profiling_options() {
        let result = parse_args(svec![
            "--heap-prof",
            "--heap-prof-name",
            "heap.log",
            "--heap-prof-dir",
            "/tmp",
            "--heap-prof-interval",
            "1024"
        ])
        .unwrap();
        assert!(result.options.per_isolate.per_env.heap_prof);
        assert_eq!(
            result.options.per_isolate.per_env.heap_prof_name,
            "heap.log"
        );
        assert_eq!(result.options.per_isolate.per_env.heap_prof_dir, "/tmp");
        assert_eq!(result.options.per_isolate.per_env.heap_prof_interval, 1024);
    }

    #[test]
    fn test_diagnostic_dir_used_for_prof_dirs() {
        let result = parse_args(svec![
            "--cpu-prof",
            "--heap-prof",
            "--diagnostic-dir",
            "/tmp/diag"
        ])
        .unwrap();
        assert_eq!(result.options.per_isolate.per_env.cpu_prof_dir, "/tmp/diag");
        assert_eq!(
            result.options.per_isolate.per_env.heap_prof_dir,
            "/tmp/diag"
        );
    }

    #[test]
    fn test_valid_input_types() {
        for input_type in &[
            "commonjs",
            "module",
            "commonjs-typescript",
            "module-typescript",
        ] {
            let result = parse_args(svec!["--input-type", input_type]).unwrap();
            assert_eq!(result.options.per_isolate.per_env.input_type, *input_type);
        }
    }

    #[test]
    fn test_valid_unhandled_rejections() {
        for rejection_type in &["warn-with-error-code", "throw", "strict", "warn", "none"] {
            let result = parse_args(svec!["--unhandled-rejections", rejection_type]).unwrap();
            assert_eq!(
                result.options.per_isolate.per_env.unhandled_rejections,
                *rejection_type
            );
        }
    }

    #[test]
    fn test_valid_trace_require_module() {
        for trace_type in &["all", "no-node-modules"] {
            let result = parse_args(svec!["--trace-require-module", trace_type]).unwrap();
            assert_eq!(
                result.options.per_isolate.per_env.trace_require_module,
                *trace_type
            );
        }
    }

    #[test]
    fn test_valid_test_isolation() {
        for isolation_type in &["process", "none"] {
            let result = parse_args(svec!["--test", "--test-isolation", isolation_type]).unwrap();
            assert_eq!(
                result.options.per_isolate.per_env.test_isolation,
                *isolation_type
            );
        }
    }

    #[test]
    fn test_valid_use_largepages() {
        for largepages_type in &["off", "on", "silent"] {
            let result = parse_args(svec!["--use-largepages", largepages_type]).unwrap();
            assert_eq!(result.options.use_largepages, *largepages_type);
        }
    }

    #[test]
    fn test_valid_inspect_publish_uid() {
        let result = parse_args(svec!["--inspect-publish-uid", "stderr,http"]).unwrap();
        assert!(
            result
                .options
                .per_isolate
                .per_env
                .debug_options
                .inspect_publish_uid
                .console
        );
        assert!(
            result
                .options
                .per_isolate
                .per_env
                .debug_options
                .inspect_publish_uid
                .http
        );
    }

    #[test]
    fn test_valid_secure_heap_power_of_two() {
        let result = parse_args(svec!["--secure-heap", "4", "--secure-heap-min", "2"]).unwrap();
        assert_eq!(result.options.secure_heap, 4);
        assert_eq!(result.options.secure_heap_min, 2);
    }

    #[test]
    fn test_implications_work() {
        // Test that --inspect-brk implies --inspect
        let result = parse_args(svec!["--inspect-brk"]).unwrap();
        assert!(
            result
                .options
                .per_isolate
                .per_env
                .debug_options
                .break_first_line
        );
        assert!(
            result
                .options
                .per_isolate
                .per_env
                .debug_options
                .inspector_enabled
        );
    }

    #[test]
    fn test_alias_expansion_works() {
        // Test that -v expands to --version
        let result = parse_args(svec!["-v"]).unwrap();
        assert!(result.options.print_version);

        // Test that -pe expands to --print --eval
        let result = parse_args(svec!["-pe", "console.log(42)"]).unwrap();
        assert!(result.options.per_isolate.per_env.print_eval);
        assert!(result.options.per_isolate.per_env.has_eval_string);
        assert_eq!(
            result.options.per_isolate.per_env.eval_string,
            "console.log(42)"
        );
    }

    #[test]
    fn test_underscore_normalization() {
        // Test that underscores get normalized to dashes
        let result = parse_args(svec!["--zero_fill_buffers"]).unwrap();
        assert!(result.options.zero_fill_all_buffers);
    }

    // ==================== Alias Expansion Tests ====================

    #[test]
    fn test_prof_process_alias_does_not_infinite_loop() {
        // --prof-process should expand to ["--prof-process", "--"] but not recurse infinitely
        let result = parse_args(svec!["--prof-process", "somefile.log"]).unwrap();
        assert!(result.options.per_isolate.per_env.prof_process);
        // The remaining args should contain somefile.log, not multiple "--"
        assert_eq!(result.remaining_args, svec!["somefile.log"]);
    }

    #[test]
    fn test_alias_short_c_to_check() {
        let result = parse_args(svec!["-c", "script.js"]).unwrap();
        assert!(result.options.per_isolate.per_env.syntax_check_only);
    }

    #[test]
    fn test_alias_short_e_to_eval() {
        let result = parse_args(svec!["-e", "console.log(1)"]).unwrap();
        assert!(result.options.per_isolate.per_env.has_eval_string);
        assert_eq!(
            result.options.per_isolate.per_env.eval_string,
            "console.log(1)"
        );
    }

    #[test]
    fn test_alias_short_p_to_print() {
        // -p expands to --print which is a boolean flag
        // The argument "42" becomes the script file, not the eval string
        // To get eval string behavior, use -pe (--print --eval)
        let result = parse_args(svec!["-p", "42"]).unwrap();
        assert!(result.options.per_isolate.per_env.print_eval);
        assert_eq!(result.remaining_args, svec!["42"]);
    }

    #[test]
    fn test_alias_short_r_to_require() {
        let result = parse_args(svec!["-r", "dotenv/config", "script.js"]).unwrap();
        assert_eq!(
            result.options.per_isolate.per_env.preload_cjs_modules,
            svec!["dotenv/config"]
        );
    }

    #[test]
    fn test_alias_short_i_to_interactive() {
        let result = parse_args(svec!["-i"]).unwrap();
        assert!(result.options.per_isolate.per_env.force_repl);
    }

    #[test]
    fn test_alias_short_h_to_help() {
        let result = parse_args(svec!["-h"]).unwrap();
        assert!(result.options.print_help);
    }

    #[test]
    fn test_alias_loader_to_experimental_loader() {
        let result = parse_args(svec!["--loader", "./my-loader.js", "script.js"]).unwrap();
        assert_eq!(
            result.options.per_isolate.per_env.userland_loaders,
            svec!["./my-loader.js"]
        );
    }

    #[test]
    fn test_alias_conditions_short() {
        let result = parse_args(svec!["-C", "development", "script.js"]).unwrap();
        assert_eq!(
            result.options.per_isolate.per_env.conditions,
            svec!["development"]
        );
    }

    // ==================== V8 Options Tests ====================

    #[test]
    fn test_v8_option_max_old_space_size() {
        let result = parse_args(svec!["--max-old-space-size=4096", "script.js"]).unwrap();
        assert!(
            result
                .v8_args
                .contains(&"--max-old-space-size=4096".to_string())
        );
    }

    #[test]
    fn test_v8_option_max_semi_space_size() {
        let result = parse_args(svec!["--max-semi-space-size=64", "script.js"]).unwrap();
        assert!(
            result
                .v8_args
                .contains(&"--max-semi-space-size=64".to_string())
        );
    }

    #[test]
    fn test_v8_option_expose_gc() {
        let result = parse_args(svec!["--expose-gc", "script.js"]).unwrap();
        assert!(result.v8_args.contains(&"--expose-gc".to_string()));
    }

    #[test]
    fn test_multiple_v8_options() {
        let result = parse_args(svec![
            "--max-old-space-size=4096",
            "--expose-gc",
            "script.js"
        ])
        .unwrap();
        assert!(
            result
                .v8_args
                .contains(&"--max-old-space-size=4096".to_string())
        );
        assert!(result.v8_args.contains(&"--expose-gc".to_string()));
    }

    // ==================== Remaining Args / Script Parsing Tests ====================

    #[test]
    fn test_script_with_args() {
        let result = parse_args(svec!["script.js", "arg1", "arg2", "--flag"]).unwrap();
        assert_eq!(
            result.remaining_args,
            svec!["script.js", "arg1", "arg2", "--flag"]
        );
    }

    #[test]
    fn test_options_before_script() {
        let result = parse_args(svec!["--no-warnings", "script.js", "--my-arg"]).unwrap();
        assert!(!result.options.per_isolate.per_env.warnings);
        assert_eq!(result.remaining_args, svec!["script.js", "--my-arg"]);
    }

    #[test]
    fn test_double_dash_stops_option_parsing() {
        let result = parse_args(svec!["--", "--version"]).unwrap();
        // --version after -- should be treated as a script name, not an option
        assert!(!result.options.print_version);
        assert_eq!(result.remaining_args, svec!["--version"]);
    }

    #[test]
    fn test_double_dash_with_script_and_args() {
        let result = parse_args(svec!["--no-warnings", "--", "script.js", "--help"]).unwrap();
        assert!(!result.options.per_isolate.per_env.warnings);
        assert_eq!(result.remaining_args, svec!["script.js", "--help"]);
    }

    // ==================== --run, --eval, --test Options Tests ====================

    #[test]
    fn test_run_option() {
        let result = parse_args(svec!["--run", "build"]).unwrap();
        assert_eq!(result.options.run, "build");
    }

    #[test]
    fn test_eval_option_with_code() {
        let result = parse_args(svec!["--eval", "console.log('hello')"]).unwrap();
        assert!(result.options.per_isolate.per_env.has_eval_string);
        assert_eq!(
            result.options.per_isolate.per_env.eval_string,
            "console.log('hello')"
        );
    }

    #[test]
    fn test_print_eval_option() {
        // --print is a boolean flag; use -pe for print+eval
        let result = parse_args(svec!["-pe", "1 + 1"]).unwrap();
        assert!(result.options.per_isolate.per_env.print_eval);
        assert!(result.options.per_isolate.per_env.has_eval_string);
        assert_eq!(result.options.per_isolate.per_env.eval_string, "1 + 1");
    }

    #[test]
    fn test_test_runner_option() {
        let result = parse_args(svec!["--test"]).unwrap();
        assert!(result.options.per_isolate.per_env.test_runner);
    }

    #[test]
    fn test_test_with_files() {
        let result = parse_args(svec!["--test", "test/*.js"]).unwrap();
        assert!(result.options.per_isolate.per_env.test_runner);
        assert_eq!(result.remaining_args, svec!["test/*.js"]);
    }

    #[test]
    fn test_test_timeout_option() {
        let result = parse_args(svec!["--test", "--test-timeout", "5000"]).unwrap();
        assert!(result.options.per_isolate.per_env.test_runner);
        assert_eq!(result.options.per_isolate.per_env.test_runner_timeout, 5000);
    }

    #[test]
    fn test_test_concurrency_option() {
        let result = parse_args(svec!["--test", "--test-concurrency", "4"]).unwrap();
        assert!(result.options.per_isolate.per_env.test_runner);
        assert_eq!(
            result.options.per_isolate.per_env.test_runner_concurrency,
            4
        );
    }

    #[test]
    fn test_test_name_pattern_option() {
        let result = parse_args(svec!["--test", "--test-name-pattern", "should.*work"]).unwrap();
        assert!(result.options.per_isolate.per_env.test_runner);
        assert_eq!(
            result.options.per_isolate.per_env.test_name_pattern,
            svec!["should.*work"]
        );
    }

    #[test]
    fn test_test_skip_pattern_option() {
        let result = parse_args(svec!["--test", "--test-skip-pattern", "slow"]).unwrap();
        assert!(result.options.per_isolate.per_env.test_runner);
        assert_eq!(
            result.options.per_isolate.per_env.test_skip_pattern,
            svec!["slow"]
        );
    }

    // ==================== String List Options Tests ====================

    #[test]
    fn test_multiple_conditions() {
        let result = parse_args(svec![
            "--conditions",
            "development",
            "--conditions",
            "browser",
            "script.js"
        ])
        .unwrap();
        assert_eq!(
            result.options.per_isolate.per_env.conditions,
            svec!["development", "browser"]
        );
    }

    #[test]
    fn test_multiple_require() {
        let result = parse_args(svec![
            "--require",
            "dotenv/config",
            "--require",
            "./setup.js",
            "script.js"
        ])
        .unwrap();
        assert_eq!(
            result.options.per_isolate.per_env.preload_cjs_modules,
            svec!["dotenv/config", "./setup.js"]
        );
    }

    #[test]
    fn test_import_option() {
        let result = parse_args(svec!["--import", "./register.js", "script.js"]).unwrap();
        assert_eq!(
            result.options.per_isolate.per_env.preload_esm_modules,
            svec!["./register.js"]
        );
    }

    #[test]
    fn test_multiple_import() {
        let result = parse_args(svec![
            "--import",
            "./a.js",
            "--import",
            "./b.js",
            "script.js"
        ])
        .unwrap();
        assert_eq!(
            result.options.per_isolate.per_env.preload_esm_modules,
            svec!["./a.js", "./b.js"]
        );
    }

    // ==================== Watch Mode Tests ====================

    #[test]
    fn test_watch_with_script() {
        let result = parse_args(svec!["--watch", "script.js"]).unwrap();
        assert!(result.options.per_isolate.per_env.watch_mode);
        assert_eq!(result.remaining_args, svec!["script.js"]);
    }

    #[test]
    fn test_watch_with_test() {
        let result = parse_args(svec!["--test", "--watch"]).unwrap();
        assert!(result.options.per_isolate.per_env.test_runner);
        assert!(result.options.per_isolate.per_env.watch_mode);
    }

    #[test]
    fn test_watch_path_option() {
        let result = parse_args(svec!["--watch", "--watch-path", "./src", "script.js"]).unwrap();
        assert!(result.options.per_isolate.per_env.watch_mode);
        assert_eq!(
            result.options.per_isolate.per_env.watch_mode_paths,
            svec!["./src"]
        );
    }

    #[test]
    fn test_watch_preserve_output() {
        let result = parse_args(svec!["--watch", "--watch-preserve-output", "script.js"]).unwrap();
        assert!(result.options.per_isolate.per_env.watch_mode);
        assert!(
            result
                .options
                .per_isolate
                .per_env
                .watch_mode_preserve_output
        );
    }

    // ==================== NODE_OPTIONS Edge Cases Tests ====================

    #[test]
    fn test_node_options_with_escaped_quotes() {
        let env_args = parse_node_options_env_var("--title \"hello \\\"world\\\"\"").unwrap();
        assert_eq!(env_args, vec!["--title", "hello \"world\""]);
    }

    #[test]
    fn test_node_options_with_backslash() {
        let env_args = parse_node_options_env_var("--title \"path\\\\to\\\\file\"").unwrap();
        assert_eq!(env_args, vec!["--title", "path\\to\\file"]);
    }

    #[test]
    fn test_node_options_multiple_spaces() {
        let env_args = parse_node_options_env_var("--no-warnings   --no-deprecation").unwrap();
        assert_eq!(env_args, vec!["--no-warnings", "--no-deprecation"]);
    }

    #[test]
    fn test_node_options_unterminated_string_error() {
        let result = parse_node_options_env_var("--title \"unterminated");
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors.iter().any(|e| e.contains("unterminated string")));
    }

    #[test]
    fn test_node_options_invalid_escape_error() {
        let result = parse_node_options_env_var("--title \"test\\");
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors.iter().any(|e| e.contains("invalid escape")));
    }

    #[test]
    fn test_node_options_empty_string() {
        let env_args = parse_node_options_env_var("").unwrap();
        assert!(env_args.is_empty());
    }

    #[test]
    fn test_node_options_only_spaces() {
        let env_args = parse_node_options_env_var("   ").unwrap();
        assert!(env_args.is_empty());
    }

    // ==================== Debug / Inspector Options Tests ====================

    #[test]
    fn test_inspect_brk_option() {
        let result = parse_args(svec!["--inspect-brk"]).unwrap();
        assert!(
            result
                .options
                .per_isolate
                .per_env
                .debug_options
                .inspector_enabled
        );
        assert!(
            result
                .options
                .per_isolate
                .per_env
                .debug_options
                .break_first_line
        );
    }

    #[test]
    fn test_inspect_wait_option() {
        let result = parse_args(svec!["--inspect-wait"]).unwrap();
        assert!(
            result
                .options
                .per_isolate
                .per_env
                .debug_options
                .inspector_enabled
        );
        assert!(
            result
                .options
                .per_isolate
                .per_env
                .debug_options
                .inspect_wait
        );
    }

    #[test]
    fn test_inspect_with_custom_port() {
        let result = parse_args(svec!["--inspect=9230"]).unwrap();
        assert!(
            result
                .options
                .per_isolate
                .per_env
                .debug_options
                .inspector_enabled
        );
        assert_eq!(
            result
                .options
                .per_isolate
                .per_env
                .debug_options
                .host_port
                .port,
            9230
        );
    }

    #[test]
    fn test_inspect_with_host_and_port() {
        let result = parse_args(svec!["--inspect=0.0.0.0:9230"]).unwrap();
        assert!(
            result
                .options
                .per_isolate
                .per_env
                .debug_options
                .inspector_enabled
        );
        assert_eq!(
            result
                .options
                .per_isolate
                .per_env
                .debug_options
                .host_port
                .host,
            "0.0.0.0"
        );
        assert_eq!(
            result
                .options
                .per_isolate
                .per_env
                .debug_options
                .host_port
                .port,
            9230
        );
    }

    #[test]
    fn test_inspect_port_zero() {
        let result = parse_args(svec!["--inspect-port", "0"]).unwrap();
        assert_eq!(
            result
                .options
                .per_isolate
                .per_env
                .debug_options
                .host_port
                .port,
            0
        );
    }

    // ==================== Env File Options Tests ====================

    #[test]
    fn test_env_file_option() {
        let result = parse_args(svec!["--env-file", ".env", "script.js"]).unwrap();
        assert!(result.options.per_isolate.per_env.has_env_file_string);
        assert_eq!(result.options.per_isolate.per_env.env_file, ".env");
    }

    #[test]
    fn test_env_file_if_exists_option() {
        let result = parse_args(svec!["--env-file-if-exists", ".env.local", "script.js"]).unwrap();
        assert!(result.options.per_isolate.per_env.has_env_file_string);
        assert_eq!(
            result.options.per_isolate.per_env.optional_env_file,
            ".env.local"
        );
    }

    // ==================== Boolean Options Tests ====================

    #[test]
    fn test_no_deprecation() {
        let result = parse_args(svec!["--no-deprecation"]).unwrap();
        assert!(!result.options.per_isolate.per_env.deprecation);
    }

    #[test]
    fn test_throw_deprecation() {
        let result = parse_args(svec!["--throw-deprecation"]).unwrap();
        assert!(result.options.per_isolate.per_env.throw_deprecation);
    }

    #[test]
    fn test_trace_deprecation() {
        let result = parse_args(svec!["--trace-deprecation"]).unwrap();
        assert!(result.options.per_isolate.per_env.trace_deprecation);
    }

    #[test]
    fn test_pending_deprecation() {
        let result = parse_args(svec!["--pending-deprecation"]).unwrap();
        assert!(result.options.per_isolate.per_env.pending_deprecation);
    }

    #[test]
    fn test_preserve_symlinks() {
        let result = parse_args(svec!["--preserve-symlinks"]).unwrap();
        assert!(result.options.per_isolate.per_env.preserve_symlinks);
    }

    #[test]
    fn test_preserve_symlinks_main() {
        let result = parse_args(svec!["--preserve-symlinks-main"]).unwrap();
        assert!(result.options.per_isolate.per_env.preserve_symlinks_main);
    }

    #[test]
    fn test_no_extra_info_on_fatal_exception() {
        let result = parse_args(svec!["--no-extra-info-on-fatal-exception"]).unwrap();
        assert!(
            !result
                .options
                .per_isolate
                .per_env
                .extra_info_on_fatal_exception
        );
    }

    #[test]
    fn test_enable_source_maps() {
        let result = parse_args(svec!["--enable-source-maps"]).unwrap();
        assert!(result.options.per_isolate.per_env.enable_source_maps);
    }

    #[test]
    fn test_experimental_strip_types() {
        let result = parse_args(svec!["--experimental-strip-types"]).unwrap();
        assert!(result.options.per_isolate.per_env.experimental_strip_types);
    }

    // ==================== Equals Syntax Tests ====================

    #[test]
    fn test_option_with_equals() {
        let result = parse_args(svec!["--title=myapp"]).unwrap();
        assert_eq!(result.options.title, "myapp");
    }

    #[test]
    fn test_option_with_equals_and_spaces_in_value() {
        // Value with spaces requires quoting at shell level, but once parsed it works
        let result = parse_args(svec!["--title=my app"]).unwrap();
        assert_eq!(result.options.title, "my app");
    }

    #[test]
    fn test_boolean_option_positive() {
        // Boolean options are set to true by using them directly
        let result = parse_args(svec!["--warnings"]).unwrap();
        assert!(result.options.per_isolate.per_env.warnings);
    }

    #[test]
    fn test_boolean_option_negative() {
        // Boolean options are set to false using --no- prefix
        let result = parse_args(svec!["--no-warnings"]).unwrap();
        assert!(!result.options.per_isolate.per_env.warnings);
    }

    #[test]
    fn test_boolean_option_double_negation() {
        // --no-deprecation disables deprecation warnings
        let result = parse_args(svec!["--no-deprecation"]).unwrap();
        assert!(!result.options.per_isolate.per_env.deprecation);
    }

    // ==================== Unknown Option Tests ====================

    #[test]
    fn test_unknown_option_passed_to_v8() {
        // Unknown options are passed through as V8 args, not treated as errors
        let result =
            parse_args(svec!["--unknown-option-that-does-not-exist", "script.js"]).unwrap();
        assert!(
            result
                .v8_args
                .contains(&"--unknown-option-that-does-not-exist".to_string())
        );
    }

    // ==================== Integer Option Tests ====================

    #[test]
    fn test_stack_trace_limit() {
        let result = parse_args(svec!["--stack-trace-limit", "50"]).unwrap();
        assert_eq!(result.options.per_isolate.stack_trace_limit, 50);
    }

    #[test]
    fn test_v8_pool_size() {
        let result = parse_args(svec!["--v8-pool-size", "8"]).unwrap();
        assert_eq!(result.options.v8_thread_pool_size, 8);
    }

    // ==================== Complex Scenarios Tests ====================

    #[test]
    fn test_combined_options_for_debugging() {
        let result = parse_args(svec![
            "--inspect-brk=9230",
            "--no-warnings",
            "--enable-source-maps",
            "script.js",
            "--arg1"
        ])
        .unwrap();
        assert!(
            result
                .options
                .per_isolate
                .per_env
                .debug_options
                .inspector_enabled
        );
        assert!(
            result
                .options
                .per_isolate
                .per_env
                .debug_options
                .break_first_line
        );
        assert_eq!(
            result
                .options
                .per_isolate
                .per_env
                .debug_options
                .host_port
                .port,
            9230
        );
        assert!(!result.options.per_isolate.per_env.warnings);
        assert!(result.options.per_isolate.per_env.enable_source_maps);
        assert_eq!(result.remaining_args, svec!["script.js", "--arg1"]);
    }

    #[test]
    fn test_combined_options_for_testing() {
        let result = parse_args(svec![
            "--test",
            "--test-timeout",
            "10000",
            "--test-concurrency",
            "2",
            "--test-reporter",
            "spec",
            "test/**/*.test.js"
        ])
        .unwrap();
        assert!(result.options.per_isolate.per_env.test_runner);
        assert_eq!(
            result.options.per_isolate.per_env.test_runner_timeout,
            10000
        );
        assert_eq!(
            result.options.per_isolate.per_env.test_runner_concurrency,
            2
        );
        assert_eq!(
            result.options.per_isolate.per_env.test_reporter,
            svec!["spec"]
        );
        assert_eq!(result.remaining_args, svec!["test/**/*.test.js"]);
    }

    #[test]
    fn test_combined_options_for_esm_loader() {
        let result = parse_args(svec![
            "--import",
            "./register.js",
            "--conditions",
            "development",
            "script.js"
        ])
        .unwrap();
        assert_eq!(
            result.options.per_isolate.per_env.preload_esm_modules,
            svec!["./register.js"]
        );
        assert_eq!(
            result.options.per_isolate.per_env.conditions,
            svec!["development"]
        );
        assert_eq!(result.remaining_args, svec!["script.js"]);
    }
}
