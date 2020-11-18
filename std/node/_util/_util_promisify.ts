// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
//
// Adapted from Node.js. Copyright Joyent, Inc. and other Node contributors.
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

// Hack: work around the following TypeScript error:
//   error: TS2345 [ERROR]: Argument of type 'typeof kCustomPromisifiedSymbol'
//   is not assignable to parameter of type 'typeof kCustomPromisifiedSymbol'.
//        assertStrictEquals(kCustomPromisifiedSymbol, promisify.custom);
//                                                     ~~~~~~~~~~~~~~~~
declare const _CustomPromisifiedSymbol: unique symbol;
declare const _CustomPromisifyArgsSymbol: unique symbol;
declare let Symbol: SymbolConstructor;
interface SymbolConstructor {
  for(key: "nodejs.util.promisify.custom"): typeof _CustomPromisifiedSymbol;
  for(
    key: "nodejs.util.promisify.customArgs",
  ): typeof _CustomPromisifyArgsSymbol;
}
// End hack.

// In addition to being accessible through util.promisify.custom,
// this symbol is registered globally and can be accessed in any environment as
// Symbol.for('nodejs.util.promisify.custom').
const kCustomPromisifiedSymbol = Symbol.for("nodejs.util.promisify.custom");
// This is an internal Node symbol used by functions returning multiple
// arguments, e.g. ['bytesRead', 'buffer'] for fs.read().
const kCustomPromisifyArgsSymbol = Symbol.for(
  "nodejs.util.promisify.customArgs",
);

class NodeInvalidArgTypeError extends TypeError {
  public code = "ERR_INVALID_ARG_TYPE";
  constructor(argumentName: string, type: string, received: unknown) {
    super(
      `The "${argumentName}" argument must be of type ${type}. Received ${typeof received}`,
    );
  }
}

export function promisify(
  // deno-lint-ignore no-explicit-any
  original: (...args: any[]) => void,
  // deno-lint-ignore no-explicit-any
): (...args: any[]) => Promise<any> {
  if (typeof original !== "function") {
    throw new NodeInvalidArgTypeError("original", "Function", original);
  }
  // deno-lint-ignore no-explicit-any
  if ((original as any)[kCustomPromisifiedSymbol]) {
    // deno-lint-ignore no-explicit-any
    const fn = (original as any)[kCustomPromisifiedSymbol];
    if (typeof fn !== "function") {
      throw new NodeInvalidArgTypeError(
        "util.promisify.custom",
        "Function",
        fn,
      );
    }
    return Object.defineProperty(fn, kCustomPromisifiedSymbol, {
      value: fn,
      enumerable: false,
      writable: false,
      configurable: true,
    });
  }

  // Names to create an object from in case the callback receives multiple
  // arguments, e.g. ['bytesRead', 'buffer'] for fs.read.
  // deno-lint-ignore no-explicit-any
  const argumentNames = (original as any)[kCustomPromisifyArgsSymbol];
  // deno-lint-ignore no-explicit-any
  function fn(this: any, ...args: unknown[]): Promise<unknown> {
    return new Promise((resolve, reject) => {
      original.call(this, ...args, (err: Error, ...values: unknown[]) => {
        if (err) {
          return reject(err);
        }
        if (argumentNames !== undefined && values.length > 1) {
          const obj = {};
          for (let i = 0; i < argumentNames.length; i++) {
            // deno-lint-ignore no-explicit-any
            (obj as any)[argumentNames[i]] = values[i];
          }
          resolve(obj);
        } else {
          resolve(values[0]);
        }
      });
    });
  }

  Object.setPrototypeOf(fn, Object.getPrototypeOf(original));

  Object.defineProperty(fn, kCustomPromisifiedSymbol, {
    value: fn,
    enumerable: false,
    writable: false,
    configurable: true,
  });
  return Object.defineProperties(
    fn,
    Object.getOwnPropertyDescriptors(original),
  );
}

promisify.custom = kCustomPromisifiedSymbol;
