// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

// Adapted from Node.js. Copyright Joyent, Inc. and other Node contributors.

// Permission is hereby granted, free of charge, to any person obtaining a
// copy of this software and associated documentation files (the
// "Software"), to deal in the Software without restriction, including
// without limitation the rights to use, copy, modify, merge, publish,
// distribute, sublicense, and/or sell copies of the Software, and to permit
// persons to whom the Software is furnished to do so, subject to the
// following conditions:

// The above copyright notice and this permission notice shall be included
// in all copies or substantial portions of the Software.

// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS
// OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
// MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN
// NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM,
// DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR
// OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE
// USE OR OTHER DEALINGS IN THE SOFTWARE.

// It will do so until we'll have Node errors completely ported (#5944):
// Ref: https://github.com/nodejs/node/blob/50d28d4b3a616b04537feff014aa70437f064e30/lib/internal/errors.js#L251
// Ref: https://github.com/nodejs/node/blob/50d28d4b3a616b04537feff014aa70437f064e30/lib/internal/errors.js#L299
// Ref: https://github.com/nodejs/node/blob/50d28d4b3a616b04537feff014aa70437f064e30/lib/internal/errors.js#L325
// Ref: https://github.com/nodejs/node/blob/50d28d4b3a616b04537feff014aa70437f064e30/lib/internal/errors.js#L943
class ERR_INVALID_ARG_TYPE extends TypeError {
  code = "ERR_INVALID_ARG_TYPE";

  constructor(a1: string, a2: string, a3: unknown) {
    super(
      `The "${a1}" argument must be of type ${a2.toLocaleLowerCase()}. Received ${typeof a3} (${a3})`,
    );
    const { name } = this;
    // Add the error code to the name to include it in the stack trace.
    this.name = `${name} [${this.code}]`;
    // Access the stack to generate the error message including the error code from the name.
    this.stack;
    // Reset the name to the actual name.
    this.name = name;
  }
}

export const codes = {
  ERR_INVALID_ARG_TYPE,
};
