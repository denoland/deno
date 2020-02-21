// Copyright Joyent, Inc. and other Node contributors.
//
// Permission is hereby granted, free of charge, to any person obtaining a
// copy of this software and associated documentation files (the
// "Software"), to deal in the Software without restriction, including
// without limitation the rights to use, copy, modify, merge, publish,
// distribute, sublicense, and/or sell copies of the Software, and to permit
// persons to whom the Software is furnished to do so, subject to the
// following conditions:
//
// The above copyright notice and this permission notice shall be included
// in all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS
// OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
// MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN
// NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM,
// DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR
// OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE
// USE OR OTHER DEALINGS IN THE SOFTWARE.

export interface StringifyOptions {
  encodeURIComponent?: (str: string) => string;
}

export interface ParseOptions {
  maxKeys?: number;
  decodeURIComponent?: (str: string) => string;
}

export interface ParsedUrlQuery {
  [key: string]: string | string[];
}

export interface ParsedUrlQueryInput {
  [key: string]:
    | string
    | number
    | boolean
    | string[]
    | number[]
    | boolean[]
    | undefined
    | null;
}

export function stringify(
  obj?: ParsedUrlQueryInput,
  sep?: string,
  eq?: string,
  options?: StringifyOptions
): string {
  sep = sep || "&";
  eq = eq || "=";
  if (obj === null) {
    obj = undefined;
  }

  if (typeof obj === "object") {
    return map(objectKeys(obj), function(k: any) {
      const ks = encodeURIComponent(stringifyPrimitive(k)) + eq;
      if (Array.isArray(obj[k])) {
        return map(obj[k], function(v: any) {
          return ks + encodeURIComponent(stringifyPrimitive(v));
        }).join(sep);
      } else {
        return ks + encodeURIComponent(stringifyPrimitive(obj[k]));
      }
    }).join(sep);
  }

  if (!name) return "";
  return (
    encodeURIComponent(stringifyPrimitive(name)) +
    eq +
    encodeURIComponent(stringifyPrimitive(obj))
  );
}

var stringifyPrimitive = function(v: any) {
  switch (typeof v) {
    case "string":
      return v;

    case "boolean":
      return v ? "true" : "false";

    case "number":
      return isFinite(v) ? v : "";

    default:
      return "";
  }
};

function map(xs: any, f: Function) {
  if (xs.map) return xs.map(f);
  const res = [];
  for (var i = 0; i < xs.length; i++) {
    res.push(f(xs[i], i));
  }
  return res;
}

var objectKeys =
  Object.keys ||
  function(obj: object) {
    var res = [];
    for (var key in obj) {
      if (Object.prototype.hasOwnProperty.call(obj, key)) res.push(key);
    }
    return res;
  };

export function parse(
  str: string,
  sep?: string,
  eq?: string,
  options?: ParseOptions
): ParsedUrlQuery {
  sep = sep || "&";
  eq = eq || "=";
  const obj = {};

  if (typeof str !== "string" || str.length === 0) {
    return obj;
  }

  const regexp = /\+/g;
  const qs = str.split(sep);

  let maxKeys =
    options && typeof options.maxKeys === "number" ? options.maxKeys : 1000;

  let len = qs.length;
  // maxKeys <= 0 means that we should not limit keys count
  if (maxKeys > 0 && len > maxKeys) {
    len = maxKeys;
  }

  for (var i = 0; i < len; ++i) {
    var x = qs[i].replace(regexp, "%20"),
      idx = x.indexOf(eq),
      kstr: string,
      vstr: string,
      k: string,
      v: string;

    if (idx >= 0) {
      kstr = x.substr(0, idx);
      vstr = x.substr(idx + 1);
    } else {
      kstr = x;
      vstr = "";
    }

    k = decodeURIComponent(kstr);
    v = decodeURIComponent(vstr);

    if (!obj.hasOwnProperty(k)) {
      obj[k] = v;
    } else if (Array.isArray(obj[k])) {
      obj[k].push(v);
    } else {
      obj[k] = [obj[k], v];
    }
  }

  return obj;
}

/**
 * The querystring.encode() function is an alias for querystring.stringify().
 */
export const encode = stringify;

/**
 * The querystring.decode() function is an alias for querystring.parse().
 */
export const decode = parse;
