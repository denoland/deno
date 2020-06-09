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

// In addition to being accessible through util.promisify.custom,
// this symbol is registered globally and can be accessed in any environment as Symbol.for('nodejs.util.promisify.custom')
const kCustomPromisifiedSymbol = Symbol.for("nodejs.util.promisify.custom");
// This is an internal Node symbol used by functions returning multiple arguments
// e.g. ['bytesRead', 'buffer'] for fs.read.
const kCustomPromisifyArgsSymbol = Symbol.for(
  "deno.nodejs.util.promisify.customArgs"
);

class NodeInvalidArgTypeError extends TypeError {
  public code = "ERR_INVALID_ARG_TYPE";
  constructor(argumentName: string, type: string, received: unknown) {
    super(
      `The "${argumentName}" argument must be of type ${type}. Received ${typeof received}`
    );
  }
}

export function promisify(original: Function): Function {
  if (typeof original !== "function")
    throw new NodeInvalidArgTypeError("original", "Function", original);

  // @ts-ignore TypeScript (as of 3.7) does not support indexing namespaces by symbol
  if (original[kCustomPromisifiedSymbol]) {
    // @ts-ignore TypeScript (as of 3.7) does not support indexing namespaces by symbol
    const fn = original[kCustomPromisifiedSymbol];
    if (typeof fn !== "function") {
      throw new NodeInvalidArgTypeError(
        "util.promisify.custom",
        "Function",
        fn
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
  // @ts-ignore TypeScript (as of 3.7) does not support indexing namespaces by symbol
  const argumentNames = original[kCustomPromisifyArgsSymbol];

  function fn(...args: unknown[]): Promise<unknown> {
    return new Promise((resolve, reject) => {
      // @ts-ignore
      original.call(this, ...args, (err: Error, ...values: unknown[]) => {
        if (err) {
          return reject(err);
        }
        if (argumentNames !== undefined && values.length > 1) {
          const obj = {};
          for (let i = 0; i < argumentNames.length; i++) {
            // @ts-ignore TypeScript
            obj[argumentNames[i]] = values[i];
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
    Object.getOwnPropertyDescriptors(original)
  );
}

promisify.custom = kCustomPromisifiedSymbol;
