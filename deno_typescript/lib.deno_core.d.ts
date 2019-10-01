// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.

// This file contains APIs that are introduced into the global namespace by
// Deno core.  These are not intended to be used directly by runtime users of
// Deno and therefore do not flow through to the runtime type library.

declare interface MessageCallback {
  (opId: number, msg: Uint8Array): void;
}

interface EvalErrorInfo {
  // Is the object thrown a native Error?
  isNativeError: boolean;
  // Was the error happened during compilation?
  isCompileError: boolean;
  // The actual thrown entity
  // (might be an Error or anything else thrown by the user)
  // If isNativeError is true, this is an Error
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  thrown: any;
}

declare interface DenoCore {
  print(s: string, isErr?: boolean);
  dispatch(
    opId: number,
    control: Uint8Array,
    zeroCopy?: ArrayBufferView | null
  ): Uint8Array | null;
  setAsyncHandler(cb: MessageCallback): void;
  sharedQueue: {
    head(): number;
    numRecords(): number;
    size(): number;
    push(buf: Uint8Array): boolean;
    reset(): void;
    shift(): Uint8Array | null;
  };

  ops(): Record<string, number>;

  recv(cb: MessageCallback): void;

  send(
    opId: number,
    control: null | ArrayBufferView,
    data?: ArrayBufferView
  ): null | Uint8Array;

  shared: SharedArrayBuffer;

  /** Evaluate provided code in the current context.
   * It differs from eval(...) in that it does not create a new context.
   * Returns an array: [output, errInfo].
   * If an error occurs, `output` becomes null and `errInfo` is non-null.
   */
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  evalContext(code: string): [any, EvalErrorInfo | null];

  errorToJSON: (e: Error) => string;
}

declare interface DenoInterface {
  core: DenoCore;
}
declare let Deno: DenoInterface;
