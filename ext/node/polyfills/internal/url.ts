// Copyright 2018-2026 the Deno authors. MIT license.

(function () {
const { core, primordials } = globalThis.__bootstrap;
const lazyUrl = core.createLazyLoader("node:url");
const { validateObject } = core.loadExtScript(
  "ext:deno_node/internal/validators.mjs",
);
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

function isURL(self: unknown): self is URL {
  return Boolean(
    // deno-lint-ignore no-explicit-any
    (self as any)?.href && (self as any).protocol &&
      // deno-lint-ignore no-explicit-any
      (self as any).auth === undefined && (self as any).path === undefined,
  );
}

function toPathIfFileURL(
  // deno-lint-ignore no-explicit-any
  fileURLOrPath: any,
) {
  if (!(ObjectPrototypeIsPrototypeOf(URL.prototype, fileURLOrPath))) {
    return fileURLOrPath;
  }
  return lazyUrl().fileURLToPath(fileURLOrPath);
}

// Utility function that converts a URL object into an ordinary
// options object as expected by the http.request and https.request
// APIs.
function urlToHttpOptions(url: URL): HttpOptions {
  validateObject(url, "url", { allowArray: true, allowFunction: true });
  const options: HttpOptions = {
    ...url, // In case the url object was extended by the user.
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

return {
  isURL,
  toPathIfFileURL,
  urlToHttpOptions,
  searchParamsSymbol: searchParams,
};
})();
