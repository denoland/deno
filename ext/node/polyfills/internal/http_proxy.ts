// Copyright 2018-2026 the Deno authors. MIT license.
// Copyright Joyent and Node contributors. All rights reserved. MIT license.

// Proxy resolution for node:http / node:https Agent, matching Node's
// NODE_USE_ENV_PROXY and http.setGlobalProxyFromEnv() behavior.

// deno-lint-ignore-file no-explicit-any prefer-primordials

import * as net from "node:net";
import * as tls from "node:tls";
import process from "node:process";

// Global proxy configuration. Set by setGlobalProxyFromEnv() or
// lazily from NODE_USE_ENV_PROXY=1.
let globalProxyEnv: Record<string, string> | null = null;
let nodeUseEnvProxyChecked = false;

/**
 * Returns the proxy env object to use for the given agent, or null if
 * proxying is not enabled. Checks (in order):
 * 1. Agent-level proxyEnv option
 * 2. Global proxy config (setGlobalProxyFromEnv)
 * 3. NODE_USE_ENV_PROXY=1 env var (lazy init)
 */
export function getProxyEnv(agent: any): Record<string, string> | null {
  if (agent?._proxyEnv) return agent._proxyEnv;
  if (globalProxyEnv) return globalProxyEnv;
  if (!nodeUseEnvProxyChecked) {
    nodeUseEnvProxyChecked = true;
    if (process.env.NODE_USE_ENV_PROXY === "1") {
      globalProxyEnv = process.env as Record<string, string>;
    }
  }
  return globalProxyEnv;
}

/**
 * Given a target URL and an env-like object, returns the proxy URL string
 * or null if no proxy applies (due to NO_PROXY or no matching env var).
 */
export function getProxyForUrl(
  protocol: string,
  hostname: string,
  port: string | number | undefined,
  proxyEnv: Record<string, string>,
): string | null {
  const isHttps = protocol === "https:";

  // Read proxy URL from env. Prefer lowercase, fall back to uppercase.
  let proxyUrl: string | undefined;
  if (isHttps) {
    proxyUrl = proxyEnv.https_proxy ?? proxyEnv.HTTPS_PROXY;
  } else {
    proxyUrl = proxyEnv.http_proxy ?? proxyEnv.HTTP_PROXY;
  }
  // Fall back to ALL_PROXY
  if (!proxyUrl) {
    proxyUrl = proxyEnv.all_proxy ?? proxyEnv.ALL_PROXY;
  }
  if (!proxyUrl) return null;

  // Check NO_PROXY
  const noProxy = proxyEnv.no_proxy ?? proxyEnv.NO_PROXY ?? "";
  if (shouldBypassProxy(hostname, port, noProxy)) return null;

  return proxyUrl;
}

/**
 * Parse and validate a proxy URL. Throws TypeError for invalid URLs (matching
 * what `new URL()` throws), which is exactly what Node tests expect.
 */
export function parseProxyUrl(
  proxyUrl: string,
): { hostname: string; port: number; auth?: string; protocol: string } {
  const parsed = new URL(proxyUrl);
  const port = parsed.port
    ? Number(parsed.port)
    : parsed.protocol === "https:"
    ? 443
    : 1080;
  const auth = parsed.username
    ? `${decodeURIComponent(parsed.username)}:${
      decodeURIComponent(parsed.password)
    }`
    : undefined;
  return {
    hostname: parsed.hostname,
    port,
    auth,
    protocol: parsed.protocol,
  };
}

/**
 * Check if a hostname:port should bypass the proxy based on NO_PROXY rules.
 */
function shouldBypassProxy(
  hostname: string,
  port: string | number | undefined,
  noProxy: string,
): boolean {
  if (!noProxy) return false;
  const noProxyTrimmed = noProxy.trim();
  if (noProxyTrimmed === "*") return true;

  const hostLower = hostname.toLowerCase();
  const entries = noProxyTrimmed.split(",");

  for (const rawEntry of entries) {
    const entry = rawEntry.trim().toLowerCase();
    if (!entry) continue;

    // Check for port-specific match: host:port
    const colonIdx = entry.lastIndexOf(":");
    if (colonIdx > 0) {
      const entryHost = entry.slice(0, colonIdx);
      const entryPort = entry.slice(colonIdx + 1);
      if (port !== undefined && String(port) === entryPort) {
        if (domainMatch(hostLower, entryHost)) return true;
      }
      continue;
    }

    if (domainMatch(hostLower, entry)) return true;
  }

  return false;
}

function domainMatch(hostname: string, pattern: string): boolean {
  if (hostname === pattern) return true;
  // .example.com matches sub.example.com
  if (pattern.startsWith(".") && hostname.endsWith(pattern)) return true;
  // example.com also matches sub.example.com (more permissive, matches curl)
  if (
    !pattern.startsWith(".") && hostname.endsWith("." + pattern)
  ) {
    return true;
  }
  return false;
}

/**
 * Create a connection through an HTTP proxy using the CONNECT method.
 * Used for HTTPS requests through an HTTP proxy.
 */
export function createProxyConnection(
  proxy: { hostname: string; port: number; auth?: string },
  target: { host: string; port: number; servername?: string },
  tlsOptions: any,
  cb: (err: Error | null, socket?: any) => void,
): void {
  const connectReq = `CONNECT ${target.host}:${target.port} HTTP/1.1\r\n` +
    `Host: ${target.host}:${target.port}\r\n` +
    (proxy.auth ? `Proxy-Authorization: Basic ${btoa(proxy.auth)}\r\n` : "") +
    `Proxy-Connection: keep-alive\r\n` +
    `\r\n`;

  const socket = net.connect(
    { host: proxy.hostname, port: proxy.port },
    () => {
      socket.write(connectReq);
    },
  );

  let buffer = "";
  socket.on("data", onData);
  socket.on("error", onError);

  // deno-lint-ignore no-node-globals
  function onData(chunk: Buffer) {
    buffer += chunk.toString();
    // Wait for full HTTP response headers
    const headerEnd = buffer.indexOf("\r\n\r\n");
    if (headerEnd === -1) return;

    socket.removeListener("data", onData);
    socket.removeListener("error", onError);

    const statusLine = buffer.slice(0, buffer.indexOf("\r\n"));
    const statusCode = parseInt(statusLine.split(" ")[1], 10);

    if (statusCode !== 200) {
      socket.destroy();
      cb(
        new Error(
          `tunneling socket could not be established, statusCode=${statusCode}`,
        ),
      );
      return;
    }

    // Upgrade the plain socket to TLS
    const tlsSocket = tls.connect({
      ...tlsOptions,
      socket,
      servername: target.servername || target.host,
    });

    tlsSocket.on("secureConnect", () => {
      cb(null, tlsSocket);
    });

    tlsSocket.on("error", (err: Error) => {
      cb(err);
    });
  }

  function onError(err: Error) {
    socket.removeListener("data", onData);
    cb(err);
  }
}

/**
 * Set the global proxy configuration from environment variables.
 * Returns a restore function.
 */
export function setGlobalProxyFromEnv(
  config?: Record<string, string>,
): () => void {
  const prev = globalProxyEnv;
  globalProxyEnv = config ?? (process.env as Record<string, string>);
  return () => {
    globalProxyEnv = prev;
  };
}
