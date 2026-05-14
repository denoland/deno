// Copyright 2018-2026 the Deno authors. MIT license.
// Copyright Joyent and Node contributors. All rights reserved. MIT license.
(function () {
const { core } = globalThis.__bootstrap;
const { SocketAddress } = core.loadExtScript(
  "ext:deno_node/internal/blocklist.mjs",
);
const { AF_INET } = core.loadExtScript(
  "ext:deno_node/internal_binding/block_list.ts",
).default;

class InternalSocketAddress extends SocketAddress {
  constructor(handle) {
    super({
      address: handle.address(),
      port: handle.port(),
      family: handle.family() === AF_INET ? "ipv4" : "ipv6",
      flowlabel: handle.flowlabel(),
    });
  }
}

return {
  SocketAddress,
  InternalSocketAddress,
};
})();
