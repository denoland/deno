// Copyright 2018-2025 the Deno authors. MIT license.

import { core, primordials } from "ext:core/mod.js";

const {
  internalRidSymbol,
} = core;
import {
  op_pipe_connect,
  op_pipe_listen,
  op_pipe_read,
  op_pipe_write,
} from "ext:core/ops";
const {
  Error,
  ObjectDefineProperty,
} = primordials;

enum PipeMode {
  Message = "message",
  Byte = "byte",
}

type Kind = "unix" | "windows";

interface Options {
  path: string;
  kind: Kind;
}

interface WindowsListenOptions extends Options {
  kind: "windows";
  maxInstances?: number;
  pipeMode: PipeMode;
  inbound?: boolean;
  outbound?: boolean;
}

interface UnixListenOptions extends Options {
  kind: "unix";
  mode?: number;
  create?: boolean;
}

class Pipe {
  #rid = 0;

  constructor(rid: number) {
    ObjectDefineProperty(this, internalRidSymbol, {
      __proto__: null,
      enumerable: false,
      value: rid,
    });

    this.#rid = rid;
  }

  async write(buffer) {
    return await op_pipe_write(this.#rid, buffer);
  }

  async read(buffer) {
    return await op_pipe_read(this.#rid, buffer);
  }
}

function connect(opts: Options) {
  let rid: number;
  switch (opts.kind) {
    case "unix":
      rid = op_pipe_connect(opts.path, "Deno.pipe.connect");
      return new Pipe(rid);
    case "windows":
      rid = op_pipe_connect(opts.path, "Deno.pipe.connect");
      return new Pipe(rid);
    default:
      throw new Error(`Unsupported kind: ${opts.kind}`);
  }
}

function listen(opts: WindowsListenOptions | UnixListenOptions) {
  let rid: number;
  switch (opts.kind) {
    case "unix":
      rid = op_pipe_listen(opts, "Deno.pipe.connect");
      return new Pipe(rid);
    case "windows":
      rid = op_pipe_listen(opts, "Deno.pipe.connect");
      return new Pipe(rid);
    default:
      throw new Error(`Unsupported kind: ${opts.kind}`);
  }
}

export { connect, listen };
