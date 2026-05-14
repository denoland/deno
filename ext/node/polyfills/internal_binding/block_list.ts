// Copyright 2018-2026 the Deno authors. MIT license.
// Copyright Joyent and Node contributors. All rights reserved. MIT license.

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
