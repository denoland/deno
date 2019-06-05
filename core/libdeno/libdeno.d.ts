// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.

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

declare type MessageCallbackInternal = (msg: Uint8Array) => void;

declare interface DenoCore {
  recv(cb: MessageCallbackInternal): void;

  send(
    cmdId: number,
    control: null | ArrayBufferView,
    data?: ArrayBufferView
  ): null | Uint8Array;

  print(x: string, isErr?: boolean): void;

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
