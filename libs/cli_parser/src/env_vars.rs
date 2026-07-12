// Copyright 2018-2026 the Deno authors. MIT license.
//! Environment variables documented in `deno --help`.

#[derive(serde::Serialize)]
pub struct EnvVar {
  pub name: &'static str,
  pub description: &'static str,
  pub example: Option<&'static str>,
}

pub static ENV_VARS: &[EnvVar] = &[
  EnvVar {
    name: "DENO_AUTH_TOKENS",
    description: "A semi-colon separated list of bearer tokens and hostnames\nto use when fetching remote modules from private repositories",
    example: Some(r#"(e.g. "abcde12345@deno.land;54321edcba@github.com")"#),
  },
  EnvVar {
    name: "DENO_CACHE_DB_MODE",
    description: "Controls whether Web cache should use disk based or in-memory database.",
    example: None,
  },
  EnvVar {
    name: "DENO_CERT",
    description: "Load certificate authorities from PEM encoded file.",
    example: None,
  },
  EnvVar {
    name: "DENO_COMPAT",
    description: "Enable Node.js compatibility mode - extensionless imports, built-in\nNode.js modules, CommonJS detection and more.",
    example: None,
  },
  EnvVar {
    name: "DENO_CONDITIONS",
    description: "Comma-separated list of custom conditions to resolve npm package\nexports and imports with. Equivalent to using the --conditions flag.",
    example: None,
  },
  EnvVar {
    name: "DENO_COVERAGE_DIR",
    description: "Set the directory for collecting code coverage profiles.\nEquivalent to using the --coverage flag.",
    example: None,
  },
  EnvVar {
    name: "DENO_DIR",
    description: "Set the cache directory",
    example: None,
  },
  EnvVar {
    name: "DENO_INSTALL_ROOT",
    description: "Set deno install's output directory",
    example: Some("(defaults to $HOME/.deno/bin)"),
  },
  EnvVar {
    name: "DENO_JOBS",
    description: "Number of parallel workers used for the --parallel flag with the test\nsubcommand. Defaults to the number of available CPUs.",
    example: None,
  },
  EnvVar {
    name: "DENO_KV_DB_MODE",
    description: "Controls whether Deno.openKv() API should use disk based or in-memory\ndatabase.",
    example: None,
  },
  EnvVar {
    name: "DENO_KV_DEFAULT_PATH",
    description: "Set the default path for Deno.openKv() when no path is provided.",
    example: None,
  },
  EnvVar {
    name: "DENO_KV_PATH_PREFIX",
    description: "Set a prefix to be added to all Deno.openKv() paths.",
    example: None,
  },
  EnvVar {
    name: "DENO_KV_REQUIRES_DISTRIBUTED_DATABASE",
    description: "Require Deno.openKv() to resolve to a distributed (remote) database.\nWhen set to \"error\", Deno.openKv() is always exposed and rejects with a\nclear message unless the resolved path is remote. When set to \"warn\", a\nlocal/in-memory fallback is allowed but logs a warning. Used by Deno\nDeploy to surface a clear error when no KV database is attached.",
    example: None,
  },
  EnvVar {
    name: "DENO_EMIT_CACHE_MODE",
    description: "Control if the transpiled sources should be cached.",
    example: None,
  },
  EnvVar {
    name: "DENO_NO_PACKAGE_JSON",
    description: "Disables auto-resolution of package.json.",
    example: None,
  },
  EnvVar {
    name: "DENO_NO_PROMPT",
    description: "Set to disable permission prompts on access\n(alternative to passing --no-prompt on invocation).",
    example: None,
  },
  EnvVar {
    name: "DENO_NO_UPDATE_CHECK",
    description: "Set to disable checking if a newer Deno version is available",
    example: None,
  },
  EnvVar {
    name: "DENO_PATCH_REACT_CVE",
    description: "Enable load-time source patches mitigating known React Server\nComponents CVEs (CVE-2025-55182, CVE-2025-55184).",
    example: None,
  },
  EnvVar {
    name: "DENO_SERVE_ADDRESS",
    description: "Override address for Deno.serve",
    example: Some(
      r#"("tcp:0.0.0.0:8080", "unix:/tmp/deno.sock", or "vsock:1234:5678")"#,
    ),
  },
  EnvVar {
    name: "DENO_SERVE_AUTOMATIC_COMPRESSION",
    description: "Set to 1 or true to enable automatic response body compression in Deno.serve by default.",
    example: None,
  },
  EnvVar {
    name: "DENO_AUTO_SERVE",
    description: "If the entrypoint contains export default { fetch }, `deno run`\nbehaves like `deno serve`.",
    example: None,
  },
  EnvVar {
    name: "DENO_TLS_CA_STORE",
    description: "Comma-separated list of order dependent certificate stores.\nPossible values: \"system\", \"mozilla\" (defaults to \"mozilla\")",
    example: None,
  },
  EnvVar {
    name: "DENO_TRACE_PERMISSIONS",
    description: "Environmental variable to enable stack traces in permission prompts.",
    example: None,
  },
  EnvVar {
    name: "DENO_USE_CGROUPS",
    description: "Use cgroups to determine V8 memory limit.",
    example: None,
  },
  EnvVar {
    name: "DENO_V8_FLAGS",
    description: "Set V8 command line options. Equivalent to using the --v8-flags flag;\nflags passed via --v8-flags are appended after these.",
    example: None,
  },
  EnvVar {
    name: "FORCE_COLOR",
    description: "Set force color output even if stdout isn't a tty.",
    example: None,
  },
  EnvVar {
    name: "HTTP_PROXY",
    description: "Proxy address for HTTP requests.",
    example: Some("(module downloads, fetch)"),
  },
  EnvVar {
    name: "HTTPS_PROXY",
    description: "Proxy address for HTTPS requests.",
    example: Some("(module downloads, fetch)"),
  },
  EnvVar {
    name: "NO_COLOR",
    description: "Set to disable color.",
    example: None,
  },
  EnvVar {
    name: "NO_PROXY",
    description: "Comma-separated list of hosts which do not use a proxy.",
    example: Some("(module downloads, fetch)"),
  },
  EnvVar {
    name: "NODE_USE_ENV_PROXY",
    description: "If set to 1, node:http and node:https honor HTTP_PROXY,\nHTTPS_PROXY, and NO_PROXY from the environment.",
    example: None,
  },
  EnvVar {
    name: "NPM_CONFIG_REGISTRY",
    description: "URL to use for the npm registry.",
    example: None,
  },
  EnvVar {
    name: "SSLKEYLOGFILE",
    description: "Write TLS session keys to the specified file in NSS Key Log format\nfor debugging encrypted traffic with tools like Wireshark.",
    example: None,
  },
  EnvVar {
    name: "DENO_TRUST_PROXY_HEADERS",
    description: "If specified, removes X-deno-client-address header when serving HTTP.",
    example: None,
  },
  EnvVar {
    name: "DENO_USR2_MEMORY_TRIM",
    description: "If specified, listen for SIGUSR2 signal to try and free memory (Linux only).",
    example: None,
  },
];
