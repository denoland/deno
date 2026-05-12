// Copyright 2018-2026 the Deno authors. MIT license.
// Copyright Joyent and Node contributors. All rights reserved. MIT license.

(function () {
const { core } = globalThis.__bootstrap;
const lazyInspector = core.createLazyLoader("node:inspector");
const { promisify } = core.loadExtScript("ext:deno_node/internal/util.mjs");

const inspector = lazyInspector().default;

class Session extends inspector.Session {
  constructor() {
    super();
  }
}
Session.prototype.post = promisify(inspector.Session.prototype.post);

return {
  close: inspector.close,
  console: inspector.console,
  Network: inspector.Network,
  open: inspector.open,
  Session,
  url: inspector.url,
  waitForDebugger: inspector.waitForDebugger,
};
})();
