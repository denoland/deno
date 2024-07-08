// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

/// <reference no-default-lib="true" />
/// <reference lib="esnext" />

declare module "ext:deno_url/00_url.js" {
  const URL: typeof URL;
  const URLSearchParams: typeof URLSearchParams;
  function parseUrlEncoded(bytes: Uint8Array): [string, string][];

  const URLPrototype: typeof URL.prototype;
}

declare module "ext:deno_url/01_urlpattern.js" {
  const URLPattern: typeof URLPattern;
}
