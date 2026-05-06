// Copyright 2018-2026 the Deno authors. MIT license.
import { core } from "ext:core/mod.js";
const { isatty, ReadStream, WriteStream } = core.loadExtScript(
  "ext:deno_node/tty.js",
);
export { isatty, ReadStream, WriteStream };
export default { isatty, ReadStream, WriteStream };
