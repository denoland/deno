// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import * as urlSearchParams from "./url_search_params";

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

export class URL {
  private _parts: URLParts;
  private _searchParams!: urlSearchParams.URLSearchParams;

  private _updateSearchParams() {
    const searchParams = new urlSearchParams.URLSearchParams(this.search);

    for (const methodName of searchParamsMethods) {
      // tslint:disable:no-any
      const method: (...args: any[]) => any = searchParams[methodName];
      searchParams[methodName] = (...args: any[]) => {
        method.apply(searchParams, args);
        this.search = searchParams.toString();
      };
      // tslint:enable
    }
    this._searchParams = searchParams;
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
    return this._parts.query;
  }

  set search(value: string) {
    value = String(value);
    if (value.charAt(0) !== "?") {
      value = `?${value}`;
    }
    this._parts.query = value;
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
      if (!baseParts) {
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
        path: urlParts.path || baseParts.path,
        query: urlParts.query || baseParts.query,
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
}
