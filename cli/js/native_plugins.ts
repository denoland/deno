import { sendSync } from "./dispatch_json.ts";
import { OP_OPEN_PLUGIN, setPluginAsyncHandler } from "./dispatch.ts";
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
  constructor(private readonly opId: number) {}

  dispatch(
    control: Uint8Array,
    zeroCopy?: ArrayBufferView | null
  ): Uint8Array | null {
    return core.dispatch(this.opId, control, zeroCopy);
  }

  setAsyncHandler(handler: AsyncHandler): void {
    setPluginAsyncHandler(this.opId, handler);
  }
}

// TODO(afinch7): add close method.

interface Plugin {
  ops: {
    [name: string]: PluginOp;
  };
}

class PluginImpl implements Plugin {
  private _ops: { [name: string]: PluginOp } = {};

  constructor(private readonly rid: number, ops: { [name: string]: number }) {
    for (const op in ops) {
      this._ops[op] = new PluginOpImpl(ops[op]);
    }
  }

  get ops(): { [name: string]: PluginOp } {
    return Object.assign({}, this._ops);
  }
}

interface OpenPluginResponse {
  rid: number;
  ops: {
    [name: string]: number;
  };
}

export function openPlugin(filename: string): Plugin {
  const response: OpenPluginResponse = sendSync(OP_OPEN_PLUGIN, {
    filename
  });
  return new PluginImpl(response.rid, response.ops);
}
