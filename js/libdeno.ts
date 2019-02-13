// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { globalEval } from "./global_eval";

// The libdeno functions are moved so that users can't access them.
type MessageCallback = (msg: Uint8Array) => void;

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

  print(x: string, isErr?: boolean): void;

  shared: ArrayBuffer;

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
