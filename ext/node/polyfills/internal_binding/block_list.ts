// Copyright 2018-2026 the Deno authors. MIT license.
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

// Mirrors Node's `internalBinding('block_list')`. Exposes a low-level
// SocketAddress handle and the AF_INET/AF_INET6 family constants used by the
// internal SocketAddress implementation. The handle is used by
// `internal/socketaddress.InternalSocketAddress` to wrap an already-validated
// address tuple into a SocketAddress without re-running validation.

(function () {
// Match POSIX values for AF_INET / AF_INET6 on Linux. The exact values are
// not observable through the public API, only through this internal binding.
const AF_INET = 2;
const AF_INET6 = 10;

class SocketAddress {
  #address;
  #port;
  #family;
  #flowlabel;

  constructor(address, port, family, flowlabel) {
    this.#address = address;
    this.#port = port;
    this.#family = family;
    this.#flowlabel = flowlabel;
  }

  address() {
    return this.#address;
  }

  port() {
    return this.#port;
  }

  family() {
    return this.#family;
  }

  flowlabel() {
    return this.#flowlabel;
  }
}

const exports = {
  AF_INET,
  AF_INET6,
  SocketAddress,
};

return {
  ...exports,
  default: exports,
};
})();
