// Copyright 2018-2026 the Deno authors. MIT license.

import { fileURLToPath } from "node:url";
import { Buffer } from "node:buffer";
import { primordials } from "ext:core/mod.js";
import { validateObject } from "ext:deno_node/internal/validators.mjs";
const {
  Boolean,
  Number,
  ObjectPrototypeIsPrototypeOf,
  StringPrototypeSlice,
  StringPrototypeStartsWith,
  Symbol,
  decodeURIComponent,
} = primordials;

interface HttpOptions {
  protocol: string;
  hostname: string;
  hash: string;
  search: string;
  pathname: string;
  path: string;
  href: string;
  port?: number;
  auth?: string;
}

const searchParams = Symbol("query");

export function isURL(self: unknown): self is URL {
  return Boolean(
    self?.href && self.protocol && self.auth === undefined &&
      self.path === undefined,
  );
}

export function toPathIfFileURL(
  fileURLOrPath: string | Buffer | URL,
): string | Buffer {
  if (!(ObjectPrototypeIsPrototypeOf(URL.prototype, fileURLOrPath))) {
    return fileURLOrPath;
  }
  return fileURLToPath(fileURLOrPath);
}

// Utility function that converts a URL object into an ordinary
// options object as expected by the http.request and https.request
// APIs.
export function urlToHttpOptions(url: URL): HttpOptions {
  validateObject(url, "url", { allowArray: true, allowFunction: true });
  const options: HttpOptions = {
    protocol: url.protocol,
    hostname: typeof url.hostname === "string" &&
        StringPrototypeStartsWith(url.hostname, "[")
      ? StringPrototypeSlice(url.hostname, 1, -1)
      : url.hostname,
    hash: url.hash,
    search: url.search,
    pathname: url.pathname,
    path: `${url.pathname || ""}${url.search || ""}`,
    href: url.href,
  };
  if (url.port !== "") {
    options.port = Number(url.port);
  }
  if (url.username || url.password) {
    options.auth = `${decodeURIComponent(url.username)}:${
      decodeURIComponent(url.password)
    }`;
  }
  return options;
}

export { searchParams as searchParamsSymbol };

export default {
  toPathIfFileURL,
};
