// Copyright 2018-2026 the Deno authors. MIT license.

// Implements env-driven proxy support for node:http and node:https.
//
// Sources of proxy configuration, in order of precedence:
//   1. Explicit `proxyEnv` option on an http.Agent (per-agent only)
//   2. `http.setGlobalProxyFromEnv(config)` (process-wide override)
//   3. NODE_USE_ENV_PROXY=1 env var or --use-env-proxy CLI flag
//      (reads HTTP_PROXY / HTTPS_PROXY / NO_PROXY from the environment)
//
// The proxy-related env vars are read with op_get_env_no_permission_check so
// they work without --allow-env, matching how Node reads them and how other
// privileged node env vars (NODE_EXTRA_CA_CERTS, ...) are read in the runtime.
//
// When a proxy applies to a request:
//   - For http:// targets, http.request() rewrites to an absolute URL and
//     routes the TCP connection to the proxy host (via createConnection
//     override on the agent).
//   - For https:// targets, https.request() opens a CONNECT tunnel through
//     the proxy and then performs TLS on the tunneled socket.

import { core, primordials } from "ext:core/mod.js";
import { op_get_env_no_permission_check } from "ext:core/ops";
const {
  ArrayIsArray,
  ArrayPrototypePush,
  Number,
  NumberIsInteger,
  RegExpPrototypeExec,
  SafeRegExp,
  String,
  StringPrototypeEndsWith,
  StringPrototypeIndexOf,
  StringPrototypeLastIndexOf,
  StringPrototypeSlice,
  StringPrototypeSplit,
  StringPrototypeStartsWith,
  StringPrototypeToLowerCase,
  StringPrototypeTrim,
  SymbolFor,
  TypeError,
  decodeURIComponent,
} = primordials;
const { Buffer } = core.loadExtScript("ext:deno_node/internal/buffer.mjs");
const {
  ERR_INVALID_ARG_TYPE,
  ERR_PROXY_INVALID_CONFIG,
} = core.loadExtScript("ext:deno_node/internal/errors.ts");

// Lowercase keys that we look for, in priority order (lowercase
// first to match Node's `http_proxy` > `HTTP_PROXY` precedence).
const HTTP_PROXY_KEYS = ["http_proxy", "HTTP_PROXY"];
const HTTPS_PROXY_KEYS = ["https_proxy", "HTTPS_PROXY"];
const NO_PROXY_KEYS = ["no_proxy", "NO_PROXY"];

// All proxy-related env vars we read from the process environment. These are
// treated as privileged runtime configuration and read without a --allow-env
// permission check, the same way NODE_EXTRA_CA_CERTS / NODE_TLS_REJECT_*  are
// read elsewhere in the node polyfills. Reading them through process.env would
// otherwise require the user to pass --allow-env just to use proxy support.
const PRIVILEGED_ENV_KEYS = [
  "http_proxy",
  "HTTP_PROXY",
  "https_proxy",
  "HTTPS_PROXY",
  "no_proxy",
  "NO_PROXY",
  "NODE_USE_ENV_PROXY",
];

// Builds a plain env-like object from the privileged ops, so the rest of this
// module can read proxy config without touching process.env (which is
// permission-checked).
function readPrivilegedEnv() {
  const env = { __proto__: null };
  for (let i = 0; i < PRIVILEGED_ENV_KEYS.length; i++) {
    const key = PRIVILEGED_ENV_KEYS[i];
    const value = op_get_env_no_permission_check(key);
    if (value !== undefined && value !== null) {
      env[key] = value;
    }
  }
  return env;
}

function isPlainObject(value) {
  if (value === null || typeof value !== "object") return false;
  if (ArrayIsArray(value)) return false;
  return true;
}

function readEnvKey(env, keys) {
  for (let i = 0; i < keys.length; i++) {
    const k = keys[i];
    if (env[k] !== undefined && env[k] !== "") {
      return env[k];
    }
  }
  return undefined;
}

const CRLF_RE = new SafeRegExp(/[\r\n]/);

function parseProxyUrl(raw, kind, mode) {
  if (raw === undefined || raw === null || raw === "") return null;
  if (typeof raw !== "string") {
    if (mode === "env") {
      throw new TypeError(`Invalid URL: ${raw}`);
    }
    throw new ERR_PROXY_INVALID_CONFIG(
      `Invalid proxy URL for ${kind}: must be a string`,
    );
  }
  // CRLF injection guard - check raw string before URL parsing strips them.
  // Matches Node's CRLF rejection in the proxy URL validator. We surface this
  // even for env-derived URLs so the auth tests get the expected error class.
  if (RegExpPrototypeExec(CRLF_RE, raw) !== null) {
    throw new ERR_PROXY_INVALID_CONFIG(`Invalid proxy URL: ${raw}`);
  }
  let url;
  try {
    url = new URL(raw);
  } catch {
    if (mode === "env") {
      throw new TypeError(`Invalid URL: ${raw}`);
    }
    throw new ERR_PROXY_INVALID_CONFIG(`Invalid proxy URL: ${raw}`);
  }
  if (url.protocol !== "http:" && url.protocol !== "https:") {
    if (mode === "env") {
      throw new TypeError(`Invalid URL: ${raw}`);
    }
    throw new ERR_PROXY_INVALID_CONFIG(
      `Invalid proxy URL: ${raw} (unsupported scheme ${url.protocol})`,
    );
  }
  const username = url.username ? decodeURIComponent(url.username) : "";
  const password = url.password ? decodeURIComponent(url.password) : "";
  // Strip square brackets from IPv6 literals so net.connect can resolve them.
  let hostname = url.hostname;
  if (
    StringPrototypeStartsWith(hostname, "[") &&
    StringPrototypeEndsWith(hostname, "]")
  ) {
    hostname = StringPrototypeSlice(hostname, 1, -1);
  }
  return {
    raw,
    protocol: url.protocol,
    hostname,
    port: url.port ? Number(url.port) : (url.protocol === "https:" ? 443 : 80),
    username,
    password,
    auth: username
      // deno-lint-ignore deno-internal/prefer-primordials -- Buffer.from()/.toString(encoding) are not primordials.
      ? "Basic " + Buffer.from(`${username}:${password}`).toString("base64")
      : undefined,
  };
}

// Returns the numeric value of an IPv4 dotted-quad string, or null if not
// a valid IPv4 literal.
const IPV4_RE = new SafeRegExp(
  /^(\d{1,3})\.(\d{1,3})\.(\d{1,3})\.(\d{1,3})$/,
);

function parseIPv4(s) {
  const m = RegExpPrototypeExec(IPV4_RE, s);
  if (!m) return null;
  let acc = 0;
  for (let i = 1; i <= 4; i++) {
    const oct = Number(m[i]);
    if (oct > 255) return null;
    acc = (acc * 256) + oct;
  }
  return acc;
}

// Returns { lo, hi } for a CIDR (a.b.c.d/N) or IP range (a-b), where lo/hi
// are 32-bit IPv4 numbers; null if the input isn't recognized.
function parseIPv4Range(s) {
  const slash = StringPrototypeIndexOf(s, "/");
  if (slash !== -1) {
    const ip = parseIPv4(StringPrototypeSlice(s, 0, slash));
    if (ip === null) return null;
    const bits = Number(StringPrototypeSlice(s, slash + 1));
    if (!NumberIsInteger(bits) || bits < 0 || bits > 32) return null;
    const mask = bits === 0 ? 0 : (0xffffffff << (32 - bits)) >>> 0;
    const lo = ip & mask;
    const hi = lo | (~mask >>> 0);
    return { lo, hi };
  }
  const dash = StringPrototypeIndexOf(s, "-");
  if (dash !== -1) {
    const lo = parseIPv4(StringPrototypeSlice(s, 0, dash));
    const hi = parseIPv4(StringPrototypeSlice(s, dash + 1));
    if (lo === null || hi === null || lo > hi) return null;
    return { lo, hi };
  }
  return null;
}

function parseNoProxy(raw) {
  if (!raw) return null;
  if (typeof raw !== "string") return null;
  const trimmed = StringPrototypeTrim(raw);
  if (trimmed === "") return null;
  if (trimmed === "*") return { all: true, entries: [] };
  const entries = [];
  const parts = StringPrototypeSplit(trimmed, ",");
  for (let i = 0; i < parts.length; i++) {
    const part = StringPrototypeTrim(parts[i]);
    if (part === "") continue;
    let host = part;
    let port = null;
    // Support [::1]:8080 and host:port forms.
    if (StringPrototypeStartsWith(host, "[")) {
      const bracket = StringPrototypeIndexOf(host, "]");
      if (bracket !== -1) {
        const rest = StringPrototypeSlice(host, bracket + 1);
        host = StringPrototypeSlice(host, 1, bracket);
        if (StringPrototypeStartsWith(rest, ":")) {
          port = StringPrototypeSlice(rest, 1);
        }
      }
    } else {
      const lastColon = StringPrototypeLastIndexOf(host, ":");
      // Only treat trailing :NNN as a port when the host has no other colons
      // (so we don't mistake a bare IPv6 like ::1 for host:port). Also skip
      // when the host already contains "/" or "-" (CIDR / range syntax).
      if (
        lastColon !== -1 && StringPrototypeIndexOf(host, ":") === lastColon &&
        StringPrototypeIndexOf(host, "/") === -1 &&
        StringPrototypeIndexOf(host, "-") === -1
      ) {
        port = StringPrototypeSlice(host, lastColon + 1);
        host = StringPrototypeSlice(host, 0, lastColon);
      }
    }
    // Try CIDR or IPv4 range first (a-b, a/n).
    const range = parseIPv4Range(host);
    if (range) {
      ArrayPrototypePush(entries, {
        host: null,
        ipRange: range,
        port: port === null ? null : Number(port),
        suffixMatch: false,
      });
      continue;
    }
    // Leading dot or "*." prefix means "matches this domain and any
    // subdomain of it". `*.example.com` is the wildcard form; `.example.com`
    // is the bare-suffix form. Both normalize to the same matcher.
    let suffixMatch = false;
    if (StringPrototypeStartsWith(host, "*.")) {
      suffixMatch = true;
      host = StringPrototypeSlice(host, 2);
    } else if (StringPrototypeStartsWith(host, ".")) {
      suffixMatch = true;
      host = StringPrototypeSlice(host, 1);
    }
    ArrayPrototypePush(entries, {
      host: StringPrototypeToLowerCase(host),
      ipRange: null,
      port: port === null ? null : Number(port),
      suffixMatch,
    });
  }
  return { all: false, entries };
}

function stripIpv6Brackets(host) {
  if (
    host && StringPrototypeStartsWith(host, "[") &&
    StringPrototypeEndsWith(host, "]")
  ) {
    return StringPrototypeSlice(host, 1, -1);
  }
  return host;
}

function shouldBypassProxy(noProxy, host, port) {
  if (!noProxy) return false;
  if (noProxy.all) return true;
  const normalizedHost = StringPrototypeToLowerCase(
    stripIpv6Brackets(String(host || "")),
  );
  const portNum = port == null ? null : Number(port);
  const hostAsIp = parseIPv4(normalizedHost);
  for (let i = 0; i < noProxy.entries.length; i++) {
    const entry = noProxy.entries[i];
    if (entry.port !== null && entry.port !== portNum) {
      continue;
    }
    if (entry.ipRange !== null) {
      if (
        hostAsIp !== null &&
        hostAsIp >= entry.ipRange.lo &&
        hostAsIp <= entry.ipRange.hi
      ) {
        return true;
      }
      continue;
    }
    if (entry.host === "" || entry.host === "*") {
      return true;
    }
    if (entry.suffixMatch) {
      if (
        normalizedHost === entry.host ||
        StringPrototypeEndsWith(normalizedHost, "." + entry.host)
      ) {
        return true;
      }
    } else {
      if (normalizedHost === entry.host) {
        return true;
      }
      // Bare hostnames in NO_PROXY also match subdomains.
      if (StringPrototypeEndsWith(normalizedHost, "." + entry.host)) {
        return true;
      }
    }
  }
  return false;
}

// Builds a ProxyConfig from a raw env-like object. `mode` controls which
// error class invalid URLs surface as: "env" throws TypeError("Invalid URL"),
// matching Node's behavior when NODE_USE_ENV_PROXY=1 sees a bad HTTP_PROXY;
// "strict" throws ERR_PROXY_INVALID_CONFIG, matching setGlobalProxyFromEnv().
function buildProxyConfig(env, mode) {
  const httpRaw = readEnvKey(env, HTTP_PROXY_KEYS);
  const httpsRaw = readEnvKey(env, HTTPS_PROXY_KEYS);
  const noProxyRaw = readEnvKey(env, NO_PROXY_KEYS);
  const http = parseProxyUrl(httpRaw, "http_proxy", mode);
  const https = parseProxyUrl(httpsRaw, "https_proxy", mode);
  if (http === null && https === null) return null;
  return {
    http,
    https,
    noProxy: parseNoProxy(noProxyRaw),
  };
}

// Returns the proxy entry that should handle a request targeting
// (protocol, host, port), or null to go direct.
function selectProxy(config, protocol, host, port) {
  if (!config) return null;
  if (shouldBypassProxy(config.noProxy, host, port)) return null;
  if (protocol === "https:" || protocol === "wss:") {
    return config.https || config.http;
  }
  return config.http;
}

// === Global proxy state ===
let globalProxyConfig = null;
let initializedFromEnv = false;

function maybeInitFromEnv() {
  if (initializedFromEnv) return;
  initializedFromEnv = true;
  const env = readPrivilegedEnv();
  // CLI flags (parsed at startup) override the env var, but if the flag
  // wasn't set, fall back to NODE_USE_ENV_PROXY.
  const cliOverride = globalThis[SymbolFor("Deno.internal.useEnvProxy")];
  let enabled;
  if (cliOverride === true || cliOverride === false) {
    enabled = cliOverride;
  } else {
    const v = env.NODE_USE_ENV_PROXY;
    enabled = v === "1" || v === "true";
  }
  if (!enabled) return;
  // Use "env" mode so an invalid HTTP_PROXY surfaces as TypeError: Invalid URL,
  // matching Node v24's behavior at request time. The error propagates out
  // through resolveAgentProxyConfig and surfaces on the offending request.
  const cfg = buildProxyConfig(env, "env");
  if (cfg) globalProxyConfig = cfg;
}

function getGlobalProxyConfig() {
  maybeInitFromEnv();
  return globalProxyConfig;
}

function setGlobalProxyFromEnv(input) {
  let env;
  if (input === undefined) {
    env = readPrivilegedEnv();
  } else if (!isPlainObject(input)) {
    throw new ERR_INVALID_ARG_TYPE(
      "proxyEnv",
      "Object",
      input,
    );
  } else {
    env = input;
  }
  // Calling setGlobalProxyFromEnv replaces any NODE_USE_ENV_PROXY-derived
  // state; ensure that read-once gate is closed.
  try {
    maybeInitFromEnv();
  } catch {
    // Env-derived init may throw on invalid URLs, but the explicit caller
    // is replacing it anyway - swallow so we can install the new config.
  }
  initializedFromEnv = true;
  const newConfig = buildProxyConfig(env, "strict");
  const prev = globalProxyConfig;
  globalProxyConfig = newConfig;
  let restored = false;
  return function restore() {
    if (restored) return;
    restored = true;
    globalProxyConfig = prev;
  };
}

// Resolves the proxy configuration to use for a given Agent. Agents with
// their own `proxyEnv` option take precedence over the global state.
function resolveAgentProxyConfig(agent) {
  if (agent && agent.__proxyConfig !== undefined) return agent.__proxyConfig;
  maybeInitFromEnv();
  return globalProxyConfig;
}

// Initializes the per-agent proxy state. Called from Agent constructor.
// Validates proxyEnv option and stores parsed config under __proxyConfig.
function initAgentProxy(agent, options) {
  if (options && options.proxyEnv !== undefined && options.proxyEnv !== null) {
    if (!isPlainObject(options.proxyEnv)) {
      throw new ERR_INVALID_ARG_TYPE(
        "options.proxyEnv",
        "Object",
        options.proxyEnv,
      );
    }
    agent.__proxyConfig = buildProxyConfig(options.proxyEnv, "strict");
  }
}

// Convenience: returns true if the global proxy state is "active" - either
// because setGlobalProxyFromEnv() has been called or NODE_USE_ENV_PROXY=1
// was honored at startup.
function hasGlobalProxy() {
  maybeInitFromEnv();
  return globalProxyConfig !== null;
}

// Called by the runtime bootstrap if NODE_USE_ENV_PROXY=1 (or
// --use-env-proxy) is set. Reads the process.env at that time.
function initFromStartupEnv() {
  const env = readPrivilegedEnv();
  // Don't throw on startup if env vars are malformed - fall back to direct.
  try {
    const cfg = buildProxyConfig(env, "env");
    if (cfg) globalProxyConfig = cfg;
  } catch {
    // ignore
  }
}

export {
  buildProxyConfig,
  getGlobalProxyConfig,
  hasGlobalProxy,
  initAgentProxy,
  initFromStartupEnv,
  parseNoProxy,
  parseProxyUrl,
  resolveAgentProxyConfig,
  selectProxy,
  setGlobalProxyFromEnv,
  shouldBypassProxy,
};

export default {
  buildProxyConfig,
  getGlobalProxyConfig,
  hasGlobalProxy,
  initAgentProxy,
  initFromStartupEnv,
  parseNoProxy,
  parseProxyUrl,
  resolveAgentProxyConfig,
  selectProxy,
  setGlobalProxyFromEnv,
  shouldBypassProxy,
};
