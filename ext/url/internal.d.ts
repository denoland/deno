// Copyright 2018-2025 the Deno authors. MIT license.

/// <reference no-default-lib="true" />
/// <reference lib="esnext" />

declare module "ext:deno_url/00_url.js" {
  const URL: typeof URL;
  const URLSearchParams: typeof URLSearchParams;
  function parseUrlEncoded(bytes: Uint8Array): [string, string][];
}

declare module "ext:deno_url/01_urlpattern.js" {
  const URLPattern: typeof URLPattern;
}
