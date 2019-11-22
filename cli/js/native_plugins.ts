import { sendSync } from "./dispatch_json.ts";
import { OP_OPEN_NATIVE_PLUGIN, setPluginAsyncHandler } from "./dispatch.ts";
import { core } from "./core.ts";

export interface AsyncHandler {
  (msg: Uint8Array): void;
}

interface NativePluginOp {
  dispatch(
    control: Uint8Array,
    zeroCopy?: ArrayBufferView | null
  ): Uint8Array | null;
  setAsyncHandler(handler: AsyncHandler): void;
}

class NativePluginOpImpl implements NativePluginOp {
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

interface NativePlugin {
  ops: {
    [name: string]: NativePluginOp;
  };
}

class NativePluginImpl implements NativePlugin {
  private _ops: { [name: string]: NativePluginOp } = {};

  constructor(private readonly rid: number, ops: { [name: string]: number }) {
    for (const op in ops) {
      this._ops[op] = new NativePluginOpImpl(ops[op]);
    }
  }

  get ops(): { [name: string]: NativePluginOp } {
    return Object.assign({}, this._ops);
  }
}

interface OpenPluginResponse {
  rid: number;
  ops: {
    [name: string]: number;
  };
}

export function openPlugin(filename: string): NativePlugin {
  const response: OpenPluginResponse = sendSync(OP_OPEN_NATIVE_PLUGIN, {
    filename
  });
  return new NativePluginImpl(response.rid, response.ops);
}
