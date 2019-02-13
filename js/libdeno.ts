// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { globalEval } from "./global_eval";
import {
  FillDirection,
  MsgRingReceiver,
  MsgRingSender,
  WaitResult
} from "./msg_ring";

// The libdeno functions are moved so that users can't access them.
type MessageCallback = (msg: Uint8Array) => void;

const enum FutexOp {
  Wait = 0,
  NotifyOne = 1,
  NotifyAll = 2
}

interface EvalErrorInfo {
  // Is the object thrown a native Error?
  isNativeError: boolean;
  // Was the error happened during compilation?
  isCompileError: boolean;
  // The actual thrown entity
  // (might be an Error or anything else thrown by the user)
  // If isNativeError is true, this is an Error
  thrown: any; // tslint:disable-line:no-any
}

interface Libdeno {
  recv(cb: MessageCallback): void;

  send(control: ArrayBufferView, data?: ArrayBufferView): null | Uint8Array;

  futex(
    offset: number,
    op: FutexOp.Wait,
    val: number,
    timeout: number
  ): 0 | 1 | 2;
  futex(offset: number, op: FutexOp.NotifyOne): void;
  futex(offset: number, op: FutexOp.NotifyAll): void;

  print(x: string, isErr?: boolean): void;

  shared: SharedArrayBuffer;

  rx: MsgRingSender;
  tx: MsgRingSender;

  // DEPRECATED
  builtinModules: { [s: string]: object };

  /** Evaluate provided code in the current context.
   * It differs from eval(...) in that it does not create a new context.
   * Returns an array: [output, errInfo].
   * If an error occurs, `output` becomes null and `errInfo` is non-null.
   */
  evalContext(
    code: string
  ): [any, EvalErrorInfo | null] /* tslint:disable-line:no-any */;

  // tslint:disable-next-line:no-any
  errorToJSON: (e: Error) => string;
}

const window = globalEval("this");
export const libdeno = window.libdeno as Libdeno;

function lazyInit() {
  if (libdeno.shared.byteLength === 0) {
    // The message ring should not be accessed before a snapshot has been created.
    // This is because the size of the shared buffer gets embedded in the snapshot,
    // which is zero when the snapshot is created.
    // TODO: move shared buffer creation to libdeno, make the size fixed
    // (non-configurable), replace c++ getter by fixed property.
    throw new Error("Shared buffer can't be used before snapshotting.");
  }

  const futex = libdeno.futex;

  // TODO IMPORTANT: review msg_ring.ts for behavior when a spurious wakeup
  // occurs. Atomics.wait/notify guarantees that these will not happen, but
  // the native implementation doesn't come with that guarantee.
  function wait(
    i32: Int32Array,
    offset: number,
    value: number,
    timeout: number
  ): WaitResult {
    const byteOffset = offset * i32.BYTES_PER_ELEMENT + i32.byteOffset;
    const timeout2 = timeout === Infinity ? -1 : Math.max(0, timeout);
    const r = futex(byteOffset, FutexOp.Wait, value, timeout2);
    switch (r) {
      case 0:
        return "ok";
      case 1:
        return "not-equal";
      case 2:
        return "timed-out";
    }
  }

  function notify(i32: Int32Array, offset: number, count: number): void {
    if (count !== 1) {
      throw new Error("Not supported.");
    }
    const byteOffset = offset * i32.BYTES_PER_ELEMENT + i32.byteOffset;
    futex(byteOffset, FutexOp.NotifyOne);
  }

  // The SharedArrayBuffer in split half. First half is for the sender, second
  // half is for the receiver.
  const half = libdeno.shared.byteLength / 2;
  const commonConfig = {
    byteLength: half,
    fillDirection: FillDirection.BottomUp,
    wait,
    notify
  };
  const rx = new MsgRingReceiver(libdeno.shared, {
    byteOffset: half,
    ...commonConfig
  });
  const tx = new MsgRingSender(libdeno.shared, {
    byteOffset: 0,
    ...commonConfig
  });

  // Replace getters by an immutable value properties.
  const flags = { configurable: false, enumerable: true, writable: true };
  Object.defineProperties(libdeno, {
    rx: { value: rx, ...flags },
    tx: { value: tx, ...flags }
  });
}

// Create getters in order to allow lazy initialization of the channel.
const flags = { configurable: true, enumerable: true };
Object.defineProperties(libdeno, {
  tx: {
    get() {
      lazyInit();
      return libdeno.tx;
    },
    ...flags
  },
  rx: {
    get() {
      lazyInit();
      return libdeno.rx;
    },
    ...flags
  }
});
