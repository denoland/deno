// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { build } from "../build.ts";
import { getRandomValues } from "../ops/get_random_values.ts";
import { domainToAscii } from "../ops/idna.ts";
import { customInspect } from "./console.ts";
import { TextEncoder } from "./text_encoding.ts";
import { urls } from "./url_search_params.ts";

interface URLParts {
  protocol: string;
  slashes: string;
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
const schemePorts: Record<string, string> = {
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
  const isSpecial = specialSchemes.includes(parts.protocol);
  if (parts.protocol == "file") {
    parts.slashes = "//";
    parts.username = "";
    parts.password = "";
    [parts.hostname, restUrl] = takePattern(restUrl, /^[/\\]{2}([^/\\?#]*)/);
    parts.port = "";
    if (build.os == "windows" && parts.hostname == "") {
      // UNC paths. e.g. "\\\\localhost\\foo\\bar" on Windows should be
      // representable as `new URL("file:////localhost/foo/bar")` which is
      // equivalent to: `new URL("file://localhost/foo/bar")`.
      [parts.hostname, restUrl] = takePattern(restUrl, /^[/\\]{2,}([^/\\?#]*)/);
    }
  } else {
    let restAuthority;
    if (isSpecial) {
      parts.slashes = "//";
      [restAuthority, restUrl] = takePattern(restUrl, /^[/\\]{2,}([^/\\?#]*)/);
    } else {
      parts.slashes = restUrl.match(/^[/\\]{2}/) ? "//" : "";
      [restAuthority, restUrl] = takePattern(restUrl, /^[/\\]{2}([^/\\?#]*)/);
    }
    let restAuthentication;
    [restAuthentication, restAuthority] = takePattern(restAuthority, /^(.*)@/);
    [parts.username, restAuthentication] = takePattern(
      restAuthentication,
      /^([^:]*)/,
    );
    parts.username = encodeUserinfo(parts.username);
    [parts.password] = takePattern(restAuthentication, /^:(.*)/);
    parts.password = encodeUserinfo(parts.password);
    [parts.hostname, restAuthority] = takePattern(
      restAuthority,
      /^(\[[0-9a-fA-F.:]{2,}\]|[^:]+)/,
    );
    [parts.port] = takePattern(restAuthority, /^:(.*)/);
    if (!isValidPort(parts.port)) {
      return undefined;
    }
    if (parts.hostname == "" && isSpecial && isBase) {
      return undefined;
    }
  }
  try {
    parts.hostname = encodeHostname(parts.hostname, isSpecial);
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
    (getRandomValues(new Uint8Array(1))[0] % 16).toString(16));
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
  isFilePath = false,
): string {
  let normalizedPath = normalizePath(path, isFilePath);
  let normalizedBasePath = normalizePath(basePath, isFilePath);

  let driveLetterPrefix = "";
  if (build.os == "windows" && isFilePath) {
    let driveLetter: string;
    let baseDriveLetter: string;
    [driveLetter, normalizedPath] = takePattern(
      normalizedPath,
      /^(\/[A-Za-z]:)(?=\/)/,
    );
    [baseDriveLetter, normalizedBasePath] = takePattern(
      normalizedBasePath,
      /^(\/[A-Za-z]:)(?=\/)/,
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
  if (value === "") return true;

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
      const isSpecial = specialSchemes.includes(parts.get(this)!.protocol);
      parts.get(this)!.hostname = encodeHostname(value, isSpecial);
    } catch {}
  }

  get href(): string {
    const authentication = this.username || this.password
      ? `${this.username}${this.password ? ":" + this.password : ""}@`
      : "";
    const host = this.host;
    const slashes = host ? "//" : parts.get(this)!.slashes;
    let pathname = this.pathname;
    if (pathname.charAt(0) != "/" && pathname != "" && host != "") {
      pathname = `/${pathname}`;
    }
    return `${this.protocol}${slashes}${authentication}${host}${pathname}${this.search}${this.hash}`;
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
    let path = parts.get(this)!.path;
    if (specialSchemes.includes(parts.get(this)!.protocol)) {
      if (path.charAt(0) != "/") {
        path = `/${path}`;
      }
    }
    return path;
  }

  set pathname(value: string) {
    parts.get(this)!.path = encodePathname(String(value));
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
      if (baseParts === undefined) {
        throw new TypeError("Invalid base URL.");
      }
    }

    const urlParts = typeof url === "string"
      ? parse(url, !baseParts)
      : parts.get(url);
    if (urlParts == undefined) {
      throw new TypeError("Invalid URL.");
    }

    if (urlParts.protocol) {
      urlParts.path = normalizePath(urlParts.path, urlParts.protocol == "file");
      parts.set(this, urlParts);
    } else if (baseParts) {
      parts.set(this, {
        protocol: baseParts.protocol,
        slashes: baseParts.slashes,
        username: baseParts.username,
        password: baseParts.password,
        hostname: baseParts.hostname,
        port: baseParts.port,
        path: resolvePathFromBase(
          urlParts.path,
          baseParts.path || "/",
          baseParts.protocol == "file",
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

function parseIpv4Number(s: string): number {
  if (s.match(/^(0[Xx])[0-9A-Za-z]+$/)) {
    return Number(s);
  }
  if (s.match(/^[0-9]+$/)) {
    return Number(s.startsWith("0") ? `0o${s}` : s);
  }
  return NaN;
}

function parseIpv4(s: string): string {
  const parts = s.split(".");
  if (parts[parts.length - 1] == "" && parts.length > 1) {
    parts.pop();
  }
  if (parts.includes("") || parts.length > 4) {
    return s;
  }
  const numbers = parts.map(parseIpv4Number);
  if (numbers.includes(NaN)) {
    return s;
  }
  const last = numbers.pop()!;
  if (last >= 256 ** (4 - numbers.length) || numbers.find((n) => n >= 256)) {
    throw new TypeError("Invalid hostname.");
  }
  const ipv4 = numbers.reduce((sum, n, i) => sum + n * 256 ** (3 - i), last);
  const ipv4Hex = ipv4.toString(16).padStart(8, "0");
  const ipv4HexParts = ipv4Hex.match(/(..)(..)(..)(..)$/)!.slice(1);
  return ipv4HexParts.map((s) => String(Number(`0x${s}`))).join(".");
}

function charInC0ControlSet(c: string): boolean {
  return (c >= "\u0000" && c <= "\u001F") || c > "\u007E";
}

function charInSearchSet(c: string): boolean {
  // deno-fmt-ignore
  return charInC0ControlSet(c) || ["\u0020", "\u0022", "\u0023", "\u0027", "\u003C", "\u003E"].includes(c) || c > "\u007E";
}

function charInFragmentSet(c: string): boolean {
  // deno-fmt-ignore
  return charInC0ControlSet(c) || ["\u0020", "\u0022", "\u003C", "\u003E", "\u0060"].includes(c);
}

function charInPathSet(c: string): boolean {
  // deno-fmt-ignore
  return charInFragmentSet(c) || ["\u0023", "\u003F", "\u007B", "\u007D"].includes(c);
}

function charInUserinfoSet(c: string): boolean {
  // "\u0027" ("'") seemingly isn't in the spec, but matches Chrome and Firefox.
  // deno-fmt-ignore
  return charInPathSet(c) || ["\u0027", "\u002F", "\u003A", "\u003B", "\u003D", "\u0040", "\u005B", "\u005C", "\u005D", "\u005E", "\u007C"].includes(c);
}

function charIsForbiddenInHost(c: string): boolean {
  // deno-fmt-ignore
  return ["\u0000", "\u0009", "\u000A", "\u000D", "\u0020", "\u0023", "\u0025", "\u002F", "\u003A", "\u003C", "\u003E", "\u003F", "\u0040", "\u005B", "\u005C", "\u005D", "\u005E"].includes(c);
}

const encoder = new TextEncoder();

function encodeChar(c: string): string {
  return [...encoder.encode(c)]
    .map((n) => `%${n.toString(16)}`)
    .join("")
    .toUpperCase();
}

function encodeUserinfo(s: string): string {
  return [...s].map((c) => (charInUserinfoSet(c) ? encodeChar(c) : c)).join("");
}

function encodeHostname(s: string, isSpecial = true): string {
  // IPv6 parsing.
  if (s.startsWith("[") && s.endsWith("]")) {
    if (!s.match(/^\[[0-9A-Fa-f.:]{2,}\]$/)) {
      throw new TypeError("Invalid hostname.");
    }
    // IPv6 address compress
    return s.toLowerCase().replace(/\b:?(?:0+:?){2,}/, "::");
  }

  let result = s;

  if (!isSpecial) {
    // Check against forbidden host code points except for "%".
    for (const c of result) {
      if (charIsForbiddenInHost(c) && c != "\u0025") {
        throw new TypeError("Invalid hostname.");
      }
    }

    // Percent-encode C0 control set.
    result = [...result]
      .map((c) => (charInC0ControlSet(c) ? encodeChar(c) : c))
      .join("");

    return result;
  }

  // Percent-decode.
  if (result.match(/%(?![0-9A-Fa-f]{2})/) != null) {
    throw new TypeError("Invalid hostname.");
  }
  result = result.replace(
    /%(.{2})/g,
    (_, hex) => String.fromCodePoint(Number(`0x${hex}`)),
  );

  // IDNA domain to ASCII.
  result = domainToAscii(result);

  // Check against forbidden host code points.
  for (const c of result) {
    if (charIsForbiddenInHost(c)) {
      throw new TypeError("Invalid hostname.");
    }
  }

  // IPv4 parsing.
  if (isSpecial) {
    result = parseIpv4(result);
  }

  return result;
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
