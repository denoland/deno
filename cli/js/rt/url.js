System.register(
  "$deno$/web/url.ts",
  [
    "$deno$/web/console.ts",
    "$deno$/web/url_search_params.ts",
    "$deno$/ops/get_random_values.ts",
  ],
  function (exports_96, context_96) {
    "use strict";
    let console_ts_5,
      url_search_params_ts_1,
      get_random_values_ts_1,
      patterns,
      urlRegExp,
      authorityRegExp,
      searchParamsMethods,
      blobURLMap,
      parts,
      URLImpl;
    const __moduleName = context_96 && context_96.id;
    function parse(url) {
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
    function generateUUID() {
      return "00000000-0000-4000-8000-000000000000".replace(/[0]/g, () =>
        // random integer from 0 to 15 as a hex digit.
        (
          get_random_values_ts_1.getRandomValues(new Uint8Array(1))[0] % 16
        ).toString(16)
      );
    }
    function isAbsolutePath(path) {
      return path.startsWith("/");
    }
    // Resolves `.`s and `..`s where possible.
    // Preserves repeating and trailing `/`s by design.
    function normalizePath(path) {
      const isAbsolute = isAbsolutePath(path);
      path = path.replace(/^\//, "");
      const pathSegments = path.split("/");
      const newPathSegments = [];
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
    function resolvePathFromBase(path, basePath) {
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
    return {
      setters: [
        function (console_ts_5_1) {
          console_ts_5 = console_ts_5_1;
        },
        function (url_search_params_ts_1_1) {
          url_search_params_ts_1 = url_search_params_ts_1_1;
        },
        function (get_random_values_ts_1_1) {
          get_random_values_ts_1 = get_random_values_ts_1_1;
        },
      ],
      execute: function () {
        patterns = {
          protocol: "(?:([a-z]+):)",
          authority: "(?://([^/?#]*))",
          path: "([^?#]*)",
          query: "(\\?[^#]*)",
          hash: "(#.*)",
          authentication: "(?:([^:]*)(?::([^@]*))?@)",
          hostname: "([^:]+)",
          port: "(?::(\\d+))",
        };
        urlRegExp = new RegExp(
          `^${patterns.protocol}?${patterns.authority}?${patterns.path}${patterns.query}?${patterns.hash}?`
        );
        authorityRegExp = new RegExp(
          `^${patterns.authentication}?${patterns.hostname}${patterns.port}?$`
        );
        searchParamsMethods = ["append", "delete", "set"];
        // Keep it outside of URL to avoid any attempts of access.
        exports_96("blobURLMap", (blobURLMap = new Map()));
        /** @internal */
        exports_96("parts", (parts = new WeakMap()));
        URLImpl = class URLImpl {
          constructor(url, base) {
            this.#updateSearchParams = () => {
              const searchParams = new URLSearchParams(this.search);
              for (const methodName of searchParamsMethods) {
                /* eslint-disable @typescript-eslint/no-explicit-any */
                const method = searchParams[methodName];
                searchParams[methodName] = (...args) => {
                  method.apply(searchParams, args);
                  this.search = searchParams.toString();
                };
                /* eslint-enable */
              }
              this.#searchParams = searchParams;
              url_search_params_ts_1.urls.set(searchParams, this);
            };
            let baseParts;
            if (base) {
              baseParts =
                typeof base === "string" ? parse(base) : parts.get(base);
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
          #searchParams;
          [console_ts_5.customInspect]() {
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
              .map((key) => `${key}: "${this[key] || ""}"`)
              .join(", ");
            return `URL { ${objectString} }`;
          }
          #updateSearchParams;
          get hash() {
            return parts.get(this).hash;
          }
          set hash(value) {
            value = unescape(String(value));
            if (!value) {
              parts.get(this).hash = "";
            } else {
              if (value.charAt(0) !== "#") {
                value = `#${value}`;
              }
              // hashes can contain % and # unescaped
              parts.get(this).hash = escape(value)
                .replace(/%25/g, "%")
                .replace(/%23/g, "#");
            }
          }
          get host() {
            return `${this.hostname}${this.port ? `:${this.port}` : ""}`;
          }
          set host(value) {
            value = String(value);
            const url = new URL(`http://${value}`);
            parts.get(this).hostname = url.hostname;
            parts.get(this).port = url.port;
          }
          get hostname() {
            return parts.get(this).hostname;
          }
          set hostname(value) {
            value = String(value);
            parts.get(this).hostname = encodeURIComponent(value);
          }
          get href() {
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
          set href(value) {
            value = String(value);
            if (value !== this.href) {
              const url = new URL(value);
              parts.set(this, { ...parts.get(url) });
              this.#updateSearchParams();
            }
          }
          get origin() {
            if (this.host) {
              return `${this.protocol}//${this.host}`;
            }
            return "null";
          }
          get password() {
            return parts.get(this).password;
          }
          set password(value) {
            value = String(value);
            parts.get(this).password = encodeURIComponent(value);
          }
          get pathname() {
            return parts.get(this)?.path || "/";
          }
          set pathname(value) {
            value = unescape(String(value));
            if (!value || value.charAt(0) !== "/") {
              value = `/${value}`;
            }
            // paths can contain % unescaped
            parts.get(this).path = escape(value).replace(/%25/g, "%");
          }
          get port() {
            return parts.get(this).port;
          }
          set port(value) {
            const port = parseInt(String(value), 10);
            parts.get(this).port = isNaN(port)
              ? ""
              : Math.max(0, port % 2 ** 16).toString();
          }
          get protocol() {
            return `${parts.get(this).protocol}:`;
          }
          set protocol(value) {
            value = String(value);
            if (value) {
              if (value.charAt(value.length - 1) === ":") {
                value = value.slice(0, -1);
              }
              parts.get(this).protocol = encodeURIComponent(value);
            }
          }
          get search() {
            const query = parts.get(this).query;
            if (query === null || query === "") {
              return "";
            }
            return query;
          }
          set search(value) {
            value = String(value);
            let query;
            if (value === "") {
              query = null;
            } else if (value.charAt(0) !== "?") {
              query = `?${value}`;
            } else {
              query = value;
            }
            parts.get(this).query = query;
            this.#updateSearchParams();
          }
          get username() {
            return parts.get(this).username;
          }
          set username(value) {
            value = String(value);
            parts.get(this).username = encodeURIComponent(value);
          }
          get searchParams() {
            return this.#searchParams;
          }
          toString() {
            return this.href;
          }
          toJSON() {
            return this.href;
          }
          // TODO(kevinkassimo): implement MediaSource version in the future.
          static createObjectURL(b) {
            const origin =
              globalThis.location.origin || "http://deno-opaque-origin";
            const key = `blob:${origin}/${generateUUID()}`;
            blobURLMap.set(key, b);
            return key;
          }
          static revokeObjectURL(url) {
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
        };
        exports_96("URLImpl", URLImpl);
      },
    };
  }
);
