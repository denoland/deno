// Copyright 2018-2026 the Deno authors. MIT license.

// Implements env-driven proxy support for node:http and node:https.
//
// Sources of proxy configuration, in order of precedence:
//   1. Explicit `proxyEnv` option on an http.Agent (per-agent only)
//   2. `http.setGlobalProxyFromEnv(config)` (process-wide override)
//   3. NODE_USE_ENV_PROXY=1 env var or --use-env-proxy CLI flag
//      (reads HTTP_PROXY / HTTPS_PROXY / NO_PROXY from process.env)
//
// When a proxy applies to a request:
//   - For http:// targets, http.request() rewrites to an absolute URL and
//     routes the TCP connection to the proxy host (via createConnection
//     override on the agent).
//   - For https:// targets, https.request() opens a CONNECT tunnel through
//     the proxy and then performs TLS on the tunneled socket.

// deno-lint-ignore-file prefer-primordials

import { core } from "ext:core/mod.js";
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

function isPlainObject(value) {
  if (value === null || typeof value !== "object") return false;
  if (Array.isArray(value)) return false;
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

function parseProxyUrl(raw, kind) {
  if (raw === undefined || raw === null || raw === "") return null;
  if (typeof raw !== "string") {
    throw new ERR_PROXY_INVALID_CONFIG(
      `Invalid proxy URL for ${kind}: must be a string`,
    );
  }
  // CRLF injection guard - check raw string before URL parsing strips them.
  if (/[\r\n]/.test(raw)) {
    throw new ERR_PROXY_INVALID_CONFIG(`Invalid proxy URL: ${raw}`);
  }
  let url;
  try {
    // Allow `host:port` form (Node accepts this; URL parser doesn't).
    url = new URL(
      /^[a-zA-Z][a-zA-Z0-9+.\-]*:\/\//.test(raw) ? raw : `http://${raw}`,
    );
  } catch {
    throw new ERR_PROXY_INVALID_CONFIG(`Invalid proxy URL: ${raw}`);
  }
  if (url.protocol !== "http:" && url.protocol !== "https:") {
    throw new ERR_PROXY_INVALID_CONFIG(
      `Invalid proxy URL: ${raw} (unsupported scheme ${url.protocol})`,
    );
  }
  const username = url.username ? decodeURIComponent(url.username) : "";
  const password = url.password ? decodeURIComponent(url.password) : "";
  // Strip square brackets from IPv6 literals so net.connect can resolve them.
  let hostname = url.hostname;
  if (hostname.startsWith("[") && hostname.endsWith("]")) {
    hostname = hostname.slice(1, -1);
  }
  return {
    raw,
    protocol: url.protocol,
    hostname,
    port: url.port ? Number(url.port) : (url.protocol === "https:" ? 443 : 80),
    username,
    password,
    auth: username
      ? "Basic " +
        Buffer.from(`${username}:${password}`).toString("base64")
      : undefined,
  };
}

function parseNoProxy(raw) {
  if (!raw) return null;
  if (typeof raw !== "string") return null;
  const trimmed = raw.trim();
  if (trimmed === "") return null;
  if (trimmed === "*") return { all: true, entries: [] };
  const entries = [];
  const parts = trimmed.split(",");
  for (let i = 0; i < parts.length; i++) {
    const part = parts[i].trim();
    if (part === "") continue;
    let host = part;
    let port = null;
    // Support [::1]:8080 and host:port forms.
    if (host.startsWith("[")) {
      const bracket = host.indexOf("]");
      if (bracket !== -1) {
        const rest = host.slice(bracket + 1);
        host = host.slice(1, bracket);
        if (rest.startsWith(":")) {
          port = rest.slice(1);
        }
      }
    } else {
      const lastColon = host.lastIndexOf(":");
      // Only treat trailing :NNN as a port when the host has no other colons
      // (so we don't mistake a bare IPv6 like ::1 for host:port).
      if (lastColon !== -1 && host.indexOf(":") === lastColon) {
        port = host.slice(lastColon + 1);
        host = host.slice(0, lastColon);
      }
    }
    // Leading dot means "matches any subdomain of"; normalize for matching.
    let suffixMatch = false;
    if (host.startsWith(".")) {
      suffixMatch = true;
      host = host.slice(1);
    }
    entries.push({
      host: host.toLowerCase(),
      port: port === null ? null : Number(port),
      suffixMatch,
    });
  }
  return { all: false, entries };
}

function stripIpv6Brackets(host) {
  if (host && host.startsWith("[") && host.endsWith("]")) {
    return host.slice(1, -1);
  }
  return host;
}

function shouldBypassProxy(noProxy, host, port) {
  if (!noProxy) return false;
  if (noProxy.all) return true;
  const normalizedHost = stripIpv6Brackets(String(host || "")).toLowerCase();
  const portNum = port == null ? null : Number(port);
  for (let i = 0; i < noProxy.entries.length; i++) {
    const entry = noProxy.entries[i];
    if (entry.port !== null && entry.port !== portNum) {
      continue;
    }
    if (entry.host === "" || entry.host === "*") {
      return true;
    }
    if (entry.suffixMatch) {
      if (
        normalizedHost === entry.host ||
        normalizedHost.endsWith("." + entry.host)
      ) {
        return true;
      }
    } else {
      if (normalizedHost === entry.host) {
        return true;
      }
      // Bare hostnames in NO_PROXY also match subdomains.
      if (normalizedHost.endsWith("." + entry.host)) {
        return true;
      }
    }
  }
  return false;
}

// Builds a ProxyConfig from a raw env-like object.
// Throws ERR_PROXY_INVALID_CONFIG on invalid proxy URLs.
function buildProxyConfig(env) {
  const httpRaw = readEnvKey(env, HTTP_PROXY_KEYS);
  const httpsRaw = readEnvKey(env, HTTPS_PROXY_KEYS);
  const noProxyRaw = readEnvKey(env, NO_PROXY_KEYS);
  const http = parseProxyUrl(httpRaw, "http_proxy");
  const https = parseProxyUrl(httpsRaw, "https_proxy");
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
  const env = (globalThis.process && globalThis.process.env) || {};
  // CLI flags (parsed at startup) override the env var, but if the flag
  // wasn't set, fall back to NODE_USE_ENV_PROXY.
  const cliOverride = globalThis[Symbol.for("Deno.internal.useEnvProxy")];
  let enabled;
  if (cliOverride === true || cliOverride === false) {
    enabled = cliOverride;
  } else {
    const v = env.NODE_USE_ENV_PROXY;
    enabled = v === "1" || v === "true";
  }
  if (!enabled) return;
  try {
    const cfg = buildProxyConfig(env);
    if (cfg) globalProxyConfig = cfg;
  } catch {
    // ignore - invalid env should not crash; tests cover this path.
  }
}

function getGlobalProxyConfig() {
  maybeInitFromEnv();
  return globalProxyConfig;
}

function setGlobalProxyFromEnv(input) {
  let env;
  if (input === undefined) {
    env = (globalThis.process && globalThis.process.env) || {};
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
  maybeInitFromEnv();
  initializedFromEnv = true;
  const newConfig = buildProxyConfig(env);
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
    agent.__proxyConfig = buildProxyConfig(options.proxyEnv);
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
  const env = (globalThis.process && globalThis.process.env) || {};
  // Don't throw on startup if env vars are malformed - fall back to direct.
  try {
    const cfg = buildProxyConfig(env);
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
