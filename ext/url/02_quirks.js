// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

// @ts-check
/// <reference path="../../core/internal.d.ts" />
/// <reference path="../../core/lib.deno_core.d.ts" />
/// <reference path="../webidl/internal.d.ts" />
/// <reference path="./internal.d.ts" />
/// <reference path="./lib.deno_url.d.ts" />

const core = globalThis.Deno.core;
const ops = core.ops;

export function processMatchInput(input, baseURL) {
  if (typeof input == "string") {
    try {
      return parseMatchInput(new URL(input));
    } catch {
      return null;
    }
  }

  return processInit(parseMatchInput(input), baseURL, input);

  // TODO: input init
}

export function parseMatchInput(input) {
  return {
    protocol: input.protocol,
    username: input.username,
    password: input.password,
    hostname: input.hostname,
    port: input.port,
    pathname: input.pathname,
    search: input.search,
    hash: input.hash,
  };
}

// https://wicg.github.io/urlpattern/#process-a-urlpatterninit
function processInit(init, baseURL, i) {
  let result = {
    protocol: i.protocol || "",
    username: i.username || "",
    password: i.password || "",
    hostname: i.hostname || "",
    port: i.port || "",
    pathname: i.pathname || "",
    search: i.search || "",
    hash: i.hash || "",
  };

  // TODO: other components.

  if (init.pathname !== undefined) {
    // TODO: base URL
    result.pathname = processPathnameForInit(init.pathname, i.protocol);
  }

  return result;
}

// https://wicg.github.io/urlpattern/#process-pathname-for-init
function processPathnameForInit(pathname, protocol) {
  if (!protocol) {
    return canonicalizeOpaquePathname(pathname);
  }

  // TODO
}

// https://wicg.github.io/urlpattern/#canonicalize-an-opaque-pathname
function canonicalizeOpaquePathname(pathname) {
  if (pathname === "") {
    return pathname;
  }

  if (pathname === "/") {
    return pathname;
  }

  let url = new URL("http://example");
  url.pathname = pathname;
  return url.pathname;
}
