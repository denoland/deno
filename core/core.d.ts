// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.

// This file contains APIs that are introduced into the global namespace by
// Deno core.  These are not intended to be used directly by runtime users of
// Deno and therefore do not flow through to the runtime type library.

declare interface MessageCallback {
  (opId: number, msg: Uint8Array): void;
}

declare type DenoOpListener = (id: number | undefined) => void;

declare interface DenoOps {
  [namespace: string]: { [name: string]: number | DenoOpListener };
}

declare interface DenoCore {
  dispatch(
    opId: number,
    control: Uint8Array,
    zeroCopy?: ArrayBufferView | null
  ): Uint8Array | null;
  setAsyncHandler(opId: number, cb: MessageCallback): void;
  sharedQueue: {
    head(): number;
    numRecords(): number;
    size(): number;
    push(buf: Uint8Array): boolean;
    reset(): void;
    shift(): Uint8Array | null;
  };
}
