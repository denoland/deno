import { openPlugin as openPluginOp } from "./ops/plugins.ts";
import { core } from "./core.ts";

export interface AsyncHandler {
  (msg: Uint8Array): void;
}

interface PluginOp {
  dispatch(
    control: Uint8Array,
    zeroCopy?: ArrayBufferView | null
  ): Uint8Array | null;
  setAsyncHandler(handler: AsyncHandler): void;
}

class PluginOpImpl implements PluginOp {
  readonly #opId: number;

  constructor(opId: number) {
    this.#opId = opId;
  }

  dispatch(
    control: Uint8Array,
    zeroCopy?: ArrayBufferView | null
  ): Uint8Array | null {
    return core.dispatch(this.#opId, control, zeroCopy);
  }

  setAsyncHandler(handler: AsyncHandler): void {
    core.setAsyncHandler(this.#opId, handler);
  }
}

// TODO(afinch7): add close method.

interface Plugin {
  ops: {
    [name: string]: PluginOp;
  };
}

class PluginImpl implements Plugin {
  #ops: { [name: string]: PluginOp } = {};

  constructor(_rid: number, ops: { [name: string]: number }) {
    for (const op in ops) {
      this.#ops[op] = new PluginOpImpl(ops[op]);
    }
  }

  get ops(): { [name: string]: PluginOp } {
    return Object.assign({}, this.#ops);
  }
}

export function openPlugin(filename: string): Plugin {
  const response = openPluginOp(filename);
  return new PluginImpl(response.rid, response.ops);
}
