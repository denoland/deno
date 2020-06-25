// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { build } from "../build.ts";
import { getRandomValues } from "../ops/get_random_values.ts";
import { customInspect } from "./console.ts";
import { urls } from "./url_search_params.ts";

interface URLParts {
  protocol: string;
  username: string;
  password: string;
  hostname: string;
  port: string;
  path: string;
  query: string;
  hash: string;
}

const searchParamsMethods: Array<keyof URLSearchParams> = [
  "append",
  "delete",
  "set",
];

const specialSchemes = ["ftp", "file", "http", "https", "ws", "wss"];

// https://url.spec.whatwg.org/#special-scheme
const schemePorts: { [key: string]: string } = {
  ftp: "21",
  file: "",
  http: "80",
  https: "443",
  ws: "80",
  wss: "443",
};
const MAX_PORT = 2 ** 16 - 1;

// Remove the part of the string that matches the pattern and return the
// remainder (RHS) as well as the first captured group of the matched substring
// (LHS). e.g.
//      takePattern("https://deno.land:80", /^([a-z]+):[/]{2}/)
//        = ["http", "deno.land:80"]
//      takePattern("deno.land:80", /^(\[[0-9a-fA-F.:]{2,}\]|[^:]+)/)
//        = ["deno.land", "80"]
function takePattern(string: string, pattern: RegExp): [string, string] {
  let capture = "";
  const rest = string.replace(pattern, (_, capture_) => {
    capture = capture_;
    return "";
  });
  return [capture, rest];
}

function parse(url: string, isBase = true): URLParts | undefined {
  const parts: Partial<URLParts> = {};
  let restUrl;
  [parts.protocol, restUrl] = takePattern(url.trim(), /^([a-z]+):/);
  if (isBase && parts.protocol == "") {
    return undefined;
  }
  if (parts.protocol == "file") {
    parts.username = "";
    parts.password = "";
    [parts.hostname, restUrl] = takePattern(restUrl, /^[/\\]{2}([^/\\?#]*)/);
    parts.port = "";
  } else if (specialSchemes.includes(parts.protocol)) {
    let restAuthority;
    [restAuthority, restUrl] = takePattern(
      restUrl,
      /^[/\\]{2}[/\\]*([^/\\?#]+)/
    );
    if (isBase && restAuthority == "") {
      return undefined;
    }
    let restAuthentication;
    [restAuthentication, restAuthority] = takePattern(restAuthority, /^(.*)@/);
    [parts.username, restAuthentication] = takePattern(
      restAuthentication,
      /^([^:]*)/
    );
    parts.username = encodeUserinfo(parts.username);
    [parts.password] = takePattern(restAuthentication, /^:(.*)/);
    parts.password = encodeUserinfo(parts.password);
    [parts.hostname, restAuthority] = takePattern(
      restAuthority,
      /^(\[[0-9a-fA-F.:]{2,}\]|[^:]+)/
    );
    [parts.port] = takePattern(restAuthority, /^:(.*)/);
    if (!isValidPort(parts.port)) {
      return undefined;
    }
  } else {
    parts.username = "";
    parts.password = "";
    parts.hostname = "";
    parts.port = "";
  }
  try {
    const IPv6re = /^\[[0-9a-fA-F.:]{2,}\]$/;
    if (!IPv6re.test(parts.hostname)) {
      parts.hostname = encodeHostname(parts.hostname); // Non-IPv6 URLs
    }
    parts.hostname = parts.hostname.toLowerCase();
  } catch {
    return undefined;
  }
  [parts.path, restUrl] = takePattern(restUrl, /^([^?#]*)/);
  parts.path = encodePathname(parts.path.replace(/\\/g, "/"));
  [parts.query, restUrl] = takePattern(restUrl, /^(\?[^#]*)/);
  parts.query = encodeSearch(parts.query);
  [parts.hash] = takePattern(restUrl, /^(#.*)/);
  parts.hash = encodeHash(parts.hash);
  return parts as URLParts;
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
export const blobURLMap = new Map<string, Blob>();

function isAbsolutePath(path: string): boolean {
  return path.startsWith("/");
}

// Resolves `.`s and `..`s where possible.
// Preserves repeating and trailing `/`s by design.
// On Windows, drive letter paths will be given a leading slash, and also a
// trailing slash if there are no other components e.g. "C:" -> "/C:/".
function normalizePath(path: string, isFilePath = false): string {
  if (build.os == "windows" && isFilePath) {
    path = path.replace(/^\/*([A-Za-z]:)(\/|$)/, "/$1/");
  }
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
function resolvePathFromBase(
  path: string,
  basePath: string,
  isFilePath = false
): string {
  let normalizedPath = normalizePath(path, isFilePath);
  let normalizedBasePath = normalizePath(basePath, isFilePath);

  let driveLetterPrefix = "";
  if (build.os == "windows" && isFilePath) {
    let driveLetter = "";
    let baseDriveLetter = "";
    [driveLetter, normalizedPath] = takePattern(
      normalizedPath,
      /^(\/[A-Za-z]:)(?=\/)/
    );
    [baseDriveLetter, normalizedBasePath] = takePattern(
      normalizedBasePath,
      /^(\/[A-Za-z]:)(?=\/)/
    );
    driveLetterPrefix = driveLetter || baseDriveLetter;
  }

  if (isAbsolutePath(normalizedPath)) {
    return `${driveLetterPrefix}${normalizedPath}`;
  }
  if (!isAbsolutePath(normalizedBasePath)) {
    throw new TypeError("Base path must be absolute.");
  }

  // Special case.
  if (path == "") {
    return `${driveLetterPrefix}${normalizedBasePath}`;
  }

  // Remove everything after the last `/` in `normalizedBasePath`.
  const prefix = normalizedBasePath.replace(/[^\/]*$/, "");
  // If `normalizedPath` ends with `.` or `..`, add a trailing slash.
  const suffix = normalizedPath.replace(/(?<=(^|\/)(\.|\.\.))$/, "/");

  return `${driveLetterPrefix}${normalizePath(prefix + suffix)}`;
}

function isValidPort(value: string): boolean {
  // https://url.spec.whatwg.org/#port-state
  if (value === "") true;
  const port = Number(value);
  return Number.isInteger(port) && port >= 0 && port <= MAX_PORT;
}

/** @internal */
export const parts = new WeakMap<URL, URLParts>();

export class URLImpl implements URL {
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
      parts.get(this)!.hash = encodeHash(value);
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
    try {
      parts.get(this)!.hostname = encodeHostname(value);
    } catch {}
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
    parts.get(this)!.password = encodeUserinfo(value);
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
    parts.get(this)!.path = encodePathname(value);
  }

  get port(): string {
    const port = parts.get(this)!.port;
    if (schemePorts[parts.get(this)!.protocol] === port) {
      return "";
    }

    return port;
  }

  set port(value: string) {
    if (!isValidPort(value)) {
      return;
    }
    parts.get(this)!.port = value.toString();
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
    return parts.get(this)!.query;
  }

  set search(value: string) {
    value = String(value);
    const query = value == "" || value.charAt(0) == "?" ? value : `?${value}`;
    parts.get(this)!.query = encodeSearch(query);
    this.#updateSearchParams();
  }

  get username(): string {
    return parts.get(this)!.username;
  }

  set username(value: string) {
    value = String(value);
    parts.get(this)!.username = encodeUserinfo(value);
  }

  get searchParams(): URLSearchParams {
    return this.#searchParams;
  }

  constructor(url: string | URL, base?: string | URL) {
    let baseParts: URLParts | undefined;
    if (base) {
      baseParts = typeof base === "string" ? parse(base) : parts.get(base);
      if (baseParts == undefined) {
        throw new TypeError("Invalid base URL.");
      }
    }

    const urlParts =
      typeof url === "string" ? parse(url, !baseParts) : parts.get(url);
    if (urlParts == undefined) {
      throw new TypeError("Invalid URL.");
    }

    if (urlParts.protocol) {
      urlParts.path = normalizePath(urlParts.path, urlParts.protocol == "file");
      parts.set(this, urlParts);
    } else if (baseParts) {
      parts.set(this, {
        protocol: baseParts.protocol,
        username: baseParts.username,
        password: baseParts.password,
        hostname: baseParts.hostname,
        port: baseParts.port,
        path: resolvePathFromBase(
          urlParts.path,
          baseParts.path || "/",
          baseParts.protocol == "file"
        ),
        query: urlParts.query,
        hash: urlParts.hash,
      });
    } else {
      throw new TypeError("Invalid URL.");
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
  static createObjectURL(b: Blob): string {
    const origin = "http://deno-opaque-origin";
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

function charInC0ControlSet(c: string): boolean {
  return c >= "\u0000" && c <= "\u001F";
}

function charInSearchSet(c: string): boolean {
  // prettier-ignore
  return charInC0ControlSet(c) || ["\u0020", "\u0022", "\u0023", "\u0027", "\u003C", "\u003E"].includes(c) || c > "\u007E";
}

function charInFragmentSet(c: string): boolean {
  // prettier-ignore
  return charInC0ControlSet(c) || ["\u0020", "\u0022", "\u003C", "\u003E", "\u0060"].includes(c);
}

function charInPathSet(c: string): boolean {
  // prettier-ignore
  return charInFragmentSet(c) || ["\u0023", "\u003F", "\u007B", "\u007D"].includes(c);
}

function charInUserinfoSet(c: string): boolean {
  // "\u0027" ("'") seemingly isn't in the spec, but matches Chrome and Firefox.
  // prettier-ignore
  return charInPathSet(c) || ["\u0027", "\u002F", "\u003A", "\u003B", "\u003D", "\u0040", "\u005B", "\u005C", "\u005D", "\u005E", "\u007C"].includes(c);
}

function encodeChar(c: string): string {
  return `%${c.charCodeAt(0).toString(16)}`.toUpperCase();
}

function encodeUserinfo(s: string): string {
  return [...s].map((c) => (charInUserinfoSet(c) ? encodeChar(c) : c)).join("");
}

function encodeHostname(s: string): string {
  // FIXME: https://url.spec.whatwg.org/#idna
  if (s.includes(":")) {
    throw new TypeError("Invalid hostname.");
  }
  return encodeURIComponent(s);
}

function encodePathname(s: string): string {
  return [...s].map((c) => (charInPathSet(c) ? encodeChar(c) : c)).join("");
}

function encodeSearch(s: string): string {
  return [...s].map((c) => (charInSearchSet(c) ? encodeChar(c) : c)).join("");
}

function encodeHash(s: string): string {
  return [...s].map((c) => (charInFragmentSet(c) ? encodeChar(c) : c)).join("");
}
