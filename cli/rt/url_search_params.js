System.register(
  "$deno$/web/url_search_params.ts",
  ["$deno$/web/url.ts", "$deno$/web/util.ts"],
  function (exports_95, context_95) {
    "use strict";
    let url_ts_1, util_ts_19, urls, URLSearchParamsImpl;
    const __moduleName = context_95 && context_95.id;
    function handleStringInitialization(searchParams, init) {
      // Overload: USVString
      // If init is a string and starts with U+003F (?),
      // remove the first code point from init.
      if (init.charCodeAt(0) === 0x003f) {
        init = init.slice(1);
      }
      for (const pair of init.split("&")) {
        // Empty params are ignored
        if (pair.length === 0) {
          continue;
        }
        const position = pair.indexOf("=");
        const name = pair.slice(0, position === -1 ? pair.length : position);
        const value = pair.slice(name.length + 1);
        searchParams.append(
          decodeURIComponent(name),
          decodeURIComponent(value)
        );
      }
    }
    function handleArrayInitialization(searchParams, init) {
      // Overload: sequence<sequence<USVString>>
      for (const tuple of init) {
        // If pair does not contain exactly two items, then throw a TypeError.
        if (tuple.length !== 2) {
          throw new TypeError(
            "URLSearchParams.constructor tuple array argument must only contain pair elements"
          );
        }
        searchParams.append(tuple[0], tuple[1]);
      }
    }
    return {
      setters: [
        function (url_ts_1_1) {
          url_ts_1 = url_ts_1_1;
        },
        function (util_ts_19_1) {
          util_ts_19 = util_ts_19_1;
        },
      ],
      execute: function () {
        /** @internal */
        exports_95("urls", (urls = new WeakMap()));
        URLSearchParamsImpl = class URLSearchParamsImpl {
          constructor(init = "") {
            this.#params = [];
            this.#updateSteps = () => {
              const url = urls.get(this);
              if (url == null) {
                return;
              }
              let query = this.toString();
              if (query === "") {
                query = null;
              }
              url_ts_1.parts.get(url).query = query;
            };
            if (typeof init === "string") {
              handleStringInitialization(this, init);
              return;
            }
            if (Array.isArray(init) || util_ts_19.isIterable(init)) {
              handleArrayInitialization(this, init);
              return;
            }
            if (Object(init) !== init) {
              return;
            }
            if (init instanceof URLSearchParamsImpl) {
              this.#params = [...init.#params];
              return;
            }
            // Overload: record<USVString, USVString>
            for (const key of Object.keys(init)) {
              this.append(key, init[key]);
            }
            urls.set(this, null);
          }
          #params;
          #updateSteps;
          append(name, value) {
            util_ts_19.requiredArguments(
              "URLSearchParams.append",
              arguments.length,
              2
            );
            this.#params.push([String(name), String(value)]);
            this.#updateSteps();
          }
          delete(name) {
            util_ts_19.requiredArguments(
              "URLSearchParams.delete",
              arguments.length,
              1
            );
            name = String(name);
            let i = 0;
            while (i < this.#params.length) {
              if (this.#params[i][0] === name) {
                this.#params.splice(i, 1);
              } else {
                i++;
              }
            }
            this.#updateSteps();
          }
          getAll(name) {
            util_ts_19.requiredArguments(
              "URLSearchParams.getAll",
              arguments.length,
              1
            );
            name = String(name);
            const values = [];
            for (const entry of this.#params) {
              if (entry[0] === name) {
                values.push(entry[1]);
              }
            }
            return values;
          }
          get(name) {
            util_ts_19.requiredArguments(
              "URLSearchParams.get",
              arguments.length,
              1
            );
            name = String(name);
            for (const entry of this.#params) {
              if (entry[0] === name) {
                return entry[1];
              }
            }
            return null;
          }
          has(name) {
            util_ts_19.requiredArguments(
              "URLSearchParams.has",
              arguments.length,
              1
            );
            name = String(name);
            return this.#params.some((entry) => entry[0] === name);
          }
          set(name, value) {
            util_ts_19.requiredArguments(
              "URLSearchParams.set",
              arguments.length,
              2
            );
            // If there are any name-value pairs whose name is name, in list,
            // set the value of the first such name-value pair to value
            // and remove the others.
            name = String(name);
            value = String(value);
            let found = false;
            let i = 0;
            while (i < this.#params.length) {
              if (this.#params[i][0] === name) {
                if (!found) {
                  this.#params[i][1] = value;
                  found = true;
                  i++;
                } else {
                  this.#params.splice(i, 1);
                }
              } else {
                i++;
              }
            }
            // Otherwise, append a new name-value pair whose name is name
            // and value is value, to list.
            if (!found) {
              this.append(name, value);
            }
            this.#updateSteps();
          }
          sort() {
            this.#params.sort((a, b) =>
              a[0] === b[0] ? 0 : a[0] > b[0] ? 1 : -1
            );
            this.#updateSteps();
          }
          forEach(
            callbackfn,
            // eslint-disable-next-line @typescript-eslint/no-explicit-any
            thisArg
          ) {
            util_ts_19.requiredArguments(
              "URLSearchParams.forEach",
              arguments.length,
              1
            );
            if (typeof thisArg !== "undefined") {
              callbackfn = callbackfn.bind(thisArg);
            }
            for (const [key, value] of this.entries()) {
              callbackfn(value, key, this);
            }
          }
          *keys() {
            for (const [key] of this.#params) {
              yield key;
            }
          }
          *values() {
            for (const [, value] of this.#params) {
              yield value;
            }
          }
          *entries() {
            yield* this.#params;
          }
          *[Symbol.iterator]() {
            yield* this.#params;
          }
          toString() {
            return this.#params
              .map(
                (tuple) =>
                  `${encodeURIComponent(tuple[0])}=${encodeURIComponent(
                    tuple[1]
                  )}`
              )
              .join("&");
          }
        };
        exports_95("URLSearchParamsImpl", URLSearchParamsImpl);
      },
    };
  }
);
