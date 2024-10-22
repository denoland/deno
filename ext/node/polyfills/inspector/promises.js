// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
// Copyright Joyent and Node contributors. All rights reserved. MIT license.

import inspector from 'ext:deno_node/inspector.js';
import { promisify } from 'ext:deno_node/internal/util/types.ts';

class Session extends inspector.Session {
  constructor() { super(); } // eslint-disable-line no-useless-constructor
}
Session.prototype.post = promisify(inspector.Session.prototype.post);

module.exports = {
  ...inspector,
  Session,
};

export * from "ext:deno_node/inspector.js";
export { Session };

export default {
  ...inspector,
  Session,
};

