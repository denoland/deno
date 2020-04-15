System.register(
  "$deno$/web/headers.ts",
  ["$deno$/web/dom_iterable.ts", "$deno$/web/util.ts", "$deno$/web/console.ts"],
  function (exports_94, context_94) {
    "use strict";
    let dom_iterable_ts_2,
      util_ts_18,
      console_ts_4,
      invalidTokenRegex,
      invalidHeaderCharRegex,
      headerMap,
      HeadersBase,
      HeadersImpl;
    const __moduleName = context_94 && context_94.id;
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    function isHeaders(value) {
      // eslint-disable-next-line @typescript-eslint/no-use-before-define
      return value instanceof Headers;
    }
    // TODO: headerGuard? Investigate if it is needed
    // node-fetch did not implement this but it is in the spec
    function normalizeParams(name, value) {
      name = String(name).toLowerCase();
      value = String(value).trim();
      return [name, value];
    }
    // The following name/value validations are copied from
    // https://github.com/bitinn/node-fetch/blob/master/src/headers.js
    // Copyright (c) 2016 David Frank. MIT License.
    function validateName(name) {
      if (invalidTokenRegex.test(name) || name === "") {
        throw new TypeError(`${name} is not a legal HTTP header name`);
      }
    }
    function validateValue(value) {
      if (invalidHeaderCharRegex.test(value)) {
        throw new TypeError(`${value} is not a legal HTTP header value`);
      }
    }
    return {
      setters: [
        function (dom_iterable_ts_2_1) {
          dom_iterable_ts_2 = dom_iterable_ts_2_1;
        },
        function (util_ts_18_1) {
          util_ts_18 = util_ts_18_1;
        },
        function (console_ts_4_1) {
          console_ts_4 = console_ts_4_1;
        },
      ],
      execute: function () {
        // From node-fetch
        // Copyright (c) 2016 David Frank. MIT License.
        invalidTokenRegex = /[^\^_`a-zA-Z\-0-9!#$%&'*+.|~]/;
        invalidHeaderCharRegex = /[^\t\x20-\x7e\x80-\xff]/;
        headerMap = Symbol("header map");
        // ref: https://fetch.spec.whatwg.org/#dom-headers
        HeadersBase = class HeadersBase {
          constructor(init) {
            if (init === null) {
              throw new TypeError(
                "Failed to construct 'Headers'; The provided value was not valid"
              );
            } else if (isHeaders(init)) {
              this[headerMap] = new Map(init);
            } else {
              this[headerMap] = new Map();
              if (Array.isArray(init)) {
                for (const tuple of init) {
                  // If header does not contain exactly two items,
                  // then throw a TypeError.
                  // ref: https://fetch.spec.whatwg.org/#concept-headers-fill
                  util_ts_18.requiredArguments(
                    "Headers.constructor tuple array argument",
                    tuple.length,
                    2
                  );
                  const [name, value] = normalizeParams(tuple[0], tuple[1]);
                  validateName(name);
                  validateValue(value);
                  const existingValue = this[headerMap].get(name);
                  this[headerMap].set(
                    name,
                    existingValue ? `${existingValue}, ${value}` : value
                  );
                }
              } else if (init) {
                const names = Object.keys(init);
                for (const rawName of names) {
                  const rawValue = init[rawName];
                  const [name, value] = normalizeParams(rawName, rawValue);
                  validateName(name);
                  validateValue(value);
                  this[headerMap].set(name, value);
                }
              }
            }
          }
          [console_ts_4.customInspect]() {
            let headerSize = this[headerMap].size;
            let output = "";
            this[headerMap].forEach((value, key) => {
              const prefix = headerSize === this[headerMap].size ? " " : "";
              const postfix = headerSize === 1 ? " " : ", ";
              output = output + `${prefix}${key}: ${value}${postfix}`;
              headerSize--;
            });
            return `Headers {${output}}`;
          }
          // ref: https://fetch.spec.whatwg.org/#concept-headers-append
          append(name, value) {
            util_ts_18.requiredArguments("Headers.append", arguments.length, 2);
            const [newname, newvalue] = normalizeParams(name, value);
            validateName(newname);
            validateValue(newvalue);
            const v = this[headerMap].get(newname);
            const str = v ? `${v}, ${newvalue}` : newvalue;
            this[headerMap].set(newname, str);
          }
          delete(name) {
            util_ts_18.requiredArguments("Headers.delete", arguments.length, 1);
            const [newname] = normalizeParams(name);
            validateName(newname);
            this[headerMap].delete(newname);
          }
          get(name) {
            util_ts_18.requiredArguments("Headers.get", arguments.length, 1);
            const [newname] = normalizeParams(name);
            validateName(newname);
            const value = this[headerMap].get(newname);
            return value || null;
          }
          has(name) {
            util_ts_18.requiredArguments("Headers.has", arguments.length, 1);
            const [newname] = normalizeParams(name);
            validateName(newname);
            return this[headerMap].has(newname);
          }
          set(name, value) {
            util_ts_18.requiredArguments("Headers.set", arguments.length, 2);
            const [newname, newvalue] = normalizeParams(name, value);
            validateName(newname);
            validateValue(newvalue);
            this[headerMap].set(newname, newvalue);
          }
          get [Symbol.toStringTag]() {
            return "Headers";
          }
        };
        // @internal
        HeadersImpl = class HeadersImpl extends dom_iterable_ts_2.DomIterableMixin(
          HeadersBase,
          headerMap
        ) {};
        exports_94("HeadersImpl", HeadersImpl);
      },
    };
  }
);
