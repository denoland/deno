// Copyright 2018-2026 the Deno authors. MIT license.
// Copyright Joyent and Node contributors. All rights reserved. MIT license.

import { core } from "ext:core/mod.js";
import inspector from "node:inspector";
const { promisify } = core.loadExtScript("ext:deno_node/internal/util.mjs");

class Session extends inspector.Session {
  constructor() {
    super();
  }
}
Session.prototype.post = promisify(inspector.Session.prototype.post);

export * from "node:inspector";
export { Session };

export default {
  ...inspector,
  Session,
};
