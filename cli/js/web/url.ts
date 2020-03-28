// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { customInspect } from "./console.ts";
import * as domTypes from "./dom_types.ts";
import { urls, URLSearchParams } from "./url_search_params.ts";
import { getRandomValues } from "../ops/get_random_values.ts";

interface URLParts {
  protocol: string;
  username: string;
  password: string;
  hostname: string;
  port: string;
  path: string;
  query: string | null;
  hash: string;
}

const patterns = {
  protocol: "(?:([a-z]+):)",
  authority: "(?://([^/?#]*))",
  path: "([^?#]*)",
  query: "(\\?[^#]*)",
  hash: "(#.*)",

  authentication: "(?:([^:]*)(?::([^@]*))?@)",
  hostname: "([^:]+)",
  port: "(?::(\\d+))",
};

const urlRegExp = new RegExp(
  `^${patterns.protocol}?${patterns.authority}?${patterns.path}${patterns.query}?${patterns.hash}?`
);

const authorityRegExp = new RegExp(
  `^${patterns.authentication}?${patterns.hostname}${patterns.port}?$`
);

const searchParamsMethods: Array<keyof URLSearchParams> = [
  "append",
  "delete",
  "set",
];

function parse(url: string): URLParts | undefined {
  const urlMatch = urlRegExp.exec(url);
  if (urlMatch) {
    const [, , authority] = urlMatch;
    const authorityMatch = authority
      ? authorityRegExp.exec(authority)
      : [null, null, null, null, null];
    if (authorityMatch) {
      return {
        protocol: urlMatch[1] || "",
        username: authorityMatch[1] || "",
        password: authorityMatch[2] || "",
        hostname: authorityMatch[3] || "",
        port: authorityMatch[4] || "",
        path: urlMatch[3] || "",
        query: urlMatch[4] || "",
        hash: urlMatch[5] || "",
      };
    }
  }
  return undefined;
}

// Based on https://github.com/kelektiv/node-uuid
// TODO(kevinkassimo): Use deno_std version once possible.
function generateUUID(): string {
  return "00000000-0000-4000-8000-000000000000".replace(/[0]/g, (): string =>
    // random integer from 0 to 15 as a hex digit.
    (getRandomValues(new Uint8Array(1))[0] % 16).toString(16)
  );
}

// Keep it outside of URL to avoid any attempts of access.
export const blobURLMap = new Map<string, domTypes.Blob>();

function isAbsolutePath(path: string): boolean {
  return path.startsWith("/");
}

// Resolves `.`s and `..`s where possible.
// Preserves repeating and trailing `/`s by design.
function normalizePath(path: string): string {
  const isAbsolute = isAbsolutePath(path);
  path = path.replace(/^\//, "");
  const pathSegments = path.split("/");

  const newPathSegments: string[] = [];
  for (let i = 0; i < pathSegments.length; i++) {
    const previous = newPathSegments[newPathSegments.length - 1];
    if (
      pathSegments[i] == ".." &&
      previous != ".." &&
      (previous != undefined || isAbsolute)
    ) {
      newPathSegments.pop();
    } else if (pathSegments[i] != ".") {
      newPathSegments.push(pathSegments[i]);
    }
  }

  let newPath = newPathSegments.join("/");
  if (!isAbsolute) {
    if (newPathSegments.length == 0) {
      newPath = ".";
    }
  } else {
    newPath = `/${newPath}`;
  }
  return newPath;
}

// Standard URL basing logic, applied to paths.
function resolvePathFromBase(path: string, basePath: string): string {
  const normalizedPath = normalizePath(path);
  if (isAbsolutePath(normalizedPath)) {
    return normalizedPath;
  }
  const normalizedBasePath = normalizePath(basePath);
  if (!isAbsolutePath(normalizedBasePath)) {
    throw new TypeError("Base path must be absolute.");
  }

  // Special case.
  if (path == "") {
    return normalizedBasePath;
  }

  // Remove everything after the last `/` in `normalizedBasePath`.
  const prefix = normalizedBasePath.replace(/[^\/]*$/, "");
  // If `normalizedPath` ends with `.` or `..`, add a trailing space.
  const suffix = normalizedPath.replace(/(?<=(^|\/)(\.|\.\.))$/, "/");

  return normalizePath(prefix + suffix);
}

/** @internal */
export const parts = new WeakMap<URL, URLParts>();

export class URL implements domTypes.URL {
  #searchParams!: URLSearchParams;

  [customInspect](): string {
    const keys = [
      "href",
      "origin",
      "protocol",
      "username",
      "password",
      "host",
      "hostname",
      "port",
      "pathname",
      "hash",
      "search",
    ];
    const objectString = keys
      .map((key: string) => `${key}: "${this[key as keyof this] || ""}"`)
      .join(", ");
    return `URL { ${objectString} }`;
  }

  #updateSearchParams = (): void => {
    const searchParams = new URLSearchParams(this.search);

    for (const methodName of searchParamsMethods) {
      /* eslint-disable @typescript-eslint/no-explicit-any */
      const method: (...args: any[]) => any = searchParams[methodName];
      searchParams[methodName] = (...args: unknown[]): any => {
        method.apply(searchParams, args);
        this.search = searchParams.toString();
      };
      /* eslint-enable */
    }
    this.#searchParams = searchParams;

    urls.set(searchParams, this);
  };

  get hash(): string {
    return parts.get(this)!.hash;
  }

  set hash(value: string) {
    value = unescape(String(value));
    if (!value) {
      parts.get(this)!.hash = "";
    } else {
      if (value.charAt(0) !== "#") {
        value = `#${value}`;
      }
      // hashes can contain % and # unescaped
      parts.get(this)!.hash = escape(value)
        .replace(/%25/g, "%")
        .replace(/%23/g, "#");
    }
  }

  get host(): string {
    return `${this.hostname}${this.port ? `:${this.port}` : ""}`;
  }

  set host(value: string) {
    value = String(value);
    const url = new URL(`http://${value}`);
    parts.get(this)!.hostname = url.hostname;
    parts.get(this)!.port = url.port;
  }

  get hostname(): string {
    return parts.get(this)!.hostname;
  }

  set hostname(value: string) {
    value = String(value);
    parts.get(this)!.hostname = encodeURIComponent(value);
  }

  get href(): string {
    const authentication =
      this.username || this.password
        ? `${this.username}${this.password ? ":" + this.password : ""}@`
        : "";
    let slash = "";
    if (this.host || this.protocol === "file:") {
      slash = "//";
    }
    return `${this.protocol}${slash}${authentication}${this.host}${this.pathname}${this.search}${this.hash}`;
  }

  set href(value: string) {
    value = String(value);
    if (value !== this.href) {
      const url = new URL(value);
      parts.set(this, { ...parts.get(url)! });
      this.#updateSearchParams();
    }
  }

  get origin(): string {
    if (this.host) {
      return `${this.protocol}//${this.host}`;
    }
    return "null";
  }

  get password(): string {
    return parts.get(this)!.password;
  }

  set password(value: string) {
    value = String(value);
    parts.get(this)!.password = encodeURIComponent(value);
  }

  get pathname(): string {
    return parts.get(this)?.path || "/";
  }

  set pathname(value: string) {
    value = unescape(String(value));
    if (!value || value.charAt(0) !== "/") {
      value = `/${value}`;
    }
    // paths can contain % unescaped
    parts.get(this)!.path = escape(value).replace(/%25/g, "%");
  }

  get port(): string {
    return parts.get(this)!.port;
  }

  set port(value: string) {
    const port = parseInt(String(value), 10);
    parts.get(this)!.port = isNaN(port)
      ? ""
      : Math.max(0, port % 2 ** 16).toString();
  }

  get protocol(): string {
    return `${parts.get(this)!.protocol}:`;
  }

  set protocol(value: string) {
    value = String(value);
    if (value) {
      if (value.charAt(value.length - 1) === ":") {
        value = value.slice(0, -1);
      }
      parts.get(this)!.protocol = encodeURIComponent(value);
    }
  }

  get search(): string {
    const query = parts.get(this)!.query;
    if (query === null || query === "") {
      return "";
    }

    return query;
  }

  set search(value: string) {
    value = String(value);
    let query: string | null;

    if (value === "") {
      query = null;
    } else if (value.charAt(0) !== "?") {
      query = `?${value}`;
    } else {
      query = value;
    }

    parts.get(this)!.query = query;
    this.#updateSearchParams();
  }

  get username(): string {
    return parts.get(this)!.username;
  }

  set username(value: string) {
    value = String(value);
    parts.get(this)!.username = encodeURIComponent(value);
  }

  get searchParams(): URLSearchParams {
    return this.#searchParams;
  }

  constructor(url: string, base?: string | URL) {
    let baseParts: URLParts | undefined;
    if (base) {
      baseParts = typeof base === "string" ? parse(base) : parts.get(base);
      if (!baseParts || baseParts.protocol == "") {
        throw new TypeError("Invalid base URL.");
      }
    }

    const urlParts = parse(url);
    if (!urlParts) {
      throw new TypeError("Invalid URL.");
    }

    if (urlParts.protocol) {
      parts.set(this, urlParts);
    } else if (baseParts) {
      parts.set(this, {
        protocol: baseParts.protocol,
        username: baseParts.username,
        password: baseParts.password,
        hostname: baseParts.hostname,
        port: baseParts.port,
        path: resolvePathFromBase(urlParts.path, baseParts.path || "/"),
        query: urlParts.query,
        hash: urlParts.hash,
      });
    } else {
      throw new TypeError("URL requires a base URL.");
    }
    this.#updateSearchParams();
  }

  toString(): string {
    return this.href;
  }

  toJSON(): string {
    return this.href;
  }

  // TODO(kevinkassimo): implement MediaSource version in the future.
  static createObjectURL(b: domTypes.Blob): string {
    const origin = globalThis.location.origin || "http://deno-opaque-origin";
    const key = `blob:${origin}/${generateUUID()}`;
    blobURLMap.set(key, b);
    return key;
  }

  static revokeObjectURL(url: string): void {
    let urlObject;
    try {
      urlObject = new URL(url);
    } catch {
      throw new TypeError("Provided URL string is not valid");
    }
    if (urlObject.protocol !== "blob:") {
      return;
    }
    // Origin match check seems irrelevant for now, unless we implement
    // persisten storage for per globalThis.location.origin at some point.
    blobURLMap.delete(url);
  }
}
