// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import * as urlSearchParams from "./url_search_params.ts";
import * as domTypes from "./dom_types.ts";
import { getRandomValues } from "./get_random_values.ts";
import { window } from "./window.ts";
import { customInspect } from "./console.ts";

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
  protocol: "(?:([^:/?#]+):)",
  authority: "(?://([^/?#]*))",
  path: "([^?#]*)",
  query: "(\\?[^#]*)",
  hash: "(#.*)",

  authentication: "(?:([^:]*)(?::([^@]*))?@)",
  hostname: "([^:]+)",
  port: "(?::(\\d+))"
};

const urlRegExp = new RegExp(
  `^${patterns.protocol}?${patterns.authority}?${patterns.path}${
    patterns.query
  }?${patterns.hash}?`
);

const authorityRegExp = new RegExp(
  `^${patterns.authentication}?${patterns.hostname}${patterns.port}?$`
);

const searchParamsMethods: Array<keyof urlSearchParams.URLSearchParams> = [
  "append",
  "delete",
  "set"
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
        hash: urlMatch[5] || ""
      };
    }
  }
  return undefined;
}

// Based on https://github.com/kelektiv/node-uuid
// TODO(kevinkassimo): Use deno_std version once possible.
function generateUUID(): string {
  return "00000000-0000-4000-8000-000000000000".replace(
    /[0]/g,
    (): string =>
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

export class URL {
  private _parts: URLParts;
  private _searchParams!: urlSearchParams.URLSearchParams;

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
      "search"
    ];
    const objectString = keys
      .map((key: string) => `${key}: "${this[key] || ""}"`)
      .join(", ");
    return `URL { ${objectString} }`;
  }

  private _updateSearchParams(): void {
    const searchParams = new urlSearchParams.URLSearchParams(this.search);

    for (const methodName of searchParamsMethods) {
      /* eslint-disable @typescript-eslint/no-explicit-any */
      const method: (...args: any[]) => any = searchParams[methodName];
      searchParams[methodName] = (...args: unknown[]): any => {
        method.apply(searchParams, args);
        this.search = searchParams.toString();
      };
      /* eslint-enable */
    }
    this._searchParams = searchParams;

    // convert to `any` that has avoided the private limit
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    (this._searchParams as any).url = this;
  }

  get hash(): string {
    return this._parts.hash;
  }

  set hash(value: string) {
    value = unescape(String(value));
    if (!value) {
      this._parts.hash = "";
    } else {
      if (value.charAt(0) !== "#") {
        value = `#${value}`;
      }
      // hashes can contain % and # unescaped
      this._parts.hash = escape(value)
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
    this._parts.hostname = url.hostname;
    this._parts.port = url.port;
  }

  get hostname(): string {
    return this._parts.hostname;
  }

  set hostname(value: string) {
    value = String(value);
    this._parts.hostname = encodeURIComponent(value);
  }

  get href(): string {
    const authentication =
      this.username || this.password
        ? `${this.username}${this.password ? ":" + this.password : ""}@`
        : "";

    return `${this.protocol}//${authentication}${this.host}${this.pathname}${
      this.search
    }${this.hash}`;
  }

  set href(value: string) {
    value = String(value);
    if (value !== this.href) {
      const url = new URL(value);
      this._parts = { ...url._parts };
      this._updateSearchParams();
    }
  }

  get origin(): string {
    return `${this.protocol}//${this.host}`;
  }

  get password(): string {
    return this._parts.password;
  }

  set password(value: string) {
    value = String(value);
    this._parts.password = encodeURIComponent(value);
  }

  get pathname(): string {
    return this._parts.path ? this._parts.path : "/";
  }

  set pathname(value: string) {
    value = unescape(String(value));
    if (!value || value.charAt(0) !== "/") {
      value = `/${value}`;
    }
    // paths can contain % unescaped
    this._parts.path = escape(value).replace(/%25/g, "%");
  }

  get port(): string {
    return this._parts.port;
  }

  set port(value: string) {
    const port = parseInt(String(value), 10);
    this._parts.port = isNaN(port)
      ? ""
      : Math.max(0, port % 2 ** 16).toString();
  }

  get protocol(): string {
    return `${this._parts.protocol}:`;
  }

  set protocol(value: string) {
    value = String(value);
    if (value) {
      if (value.charAt(value.length - 1) === ":") {
        value = value.slice(0, -1);
      }
      this._parts.protocol = encodeURIComponent(value);
    }
  }

  get search(): string {
    if (this._parts.query === null || this._parts.query === "") {
      return "";
    }

    return this._parts.query;
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

    this._parts.query = query;
    this._updateSearchParams();
  }

  get username(): string {
    return this._parts.username;
  }

  set username(value: string) {
    value = String(value);
    this._parts.username = encodeURIComponent(value);
  }

  get searchParams(): urlSearchParams.URLSearchParams {
    return this._searchParams;
  }

  constructor(url: string, base?: string | URL) {
    let baseParts: URLParts | undefined;
    if (base) {
      baseParts = typeof base === "string" ? parse(base) : base._parts;
      if (!baseParts || baseParts.protocol == "") {
        throw new TypeError("Invalid base URL.");
      }
    }

    const urlParts = parse(url);
    if (!urlParts) {
      throw new TypeError("Invalid URL.");
    }

    if (urlParts.protocol) {
      this._parts = urlParts;
    } else if (baseParts) {
      this._parts = {
        protocol: baseParts.protocol,
        username: baseParts.username,
        password: baseParts.password,
        hostname: baseParts.hostname,
        port: baseParts.port,
        path: resolvePathFromBase(urlParts.path, baseParts.path || "/"),
        query: urlParts.query,
        hash: urlParts.hash
      };
    } else {
      throw new TypeError("URL requires a base URL.");
    }
    this._updateSearchParams();
  }

  toString(): string {
    return this.href;
  }

  toJSON(): string {
    return this.href;
  }

  // TODO(kevinkassimo): implement MediaSource version in the future.
  static createObjectURL(b: domTypes.Blob): string {
    const origin = window.location.origin || "http://deno-opaque-origin";
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
    // persisten storage for per window.location.origin at some point.
    blobURLMap.delete(url);
  }
}
