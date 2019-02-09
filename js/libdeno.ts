// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { globalEval } from "./global_eval";

// The libdeno functions are moved so that users can't access them.
type MessageCallback = (msg: Uint8Array) => void;
export type PromiseRejectEvent =
  | "RejectWithNoHandler"
  | "HandlerAddedAfterReject"
  | "ResolveAfterResolved"
  | "RejectAfterResolved";

interface EvalErrorInfo {
  // Is the object thrown a native Error?
  isNativeError: boolean;
  // Was the error happened during compilation?
  isCompileError: boolean;
  // The actual thrown entity
  // (might be an Error or anything else thrown by the user)
  thrown: any; // tslint:disable-line:no-any
}

interface Libdeno {
  recv(cb: MessageCallback): void;

  send(control: ArrayBufferView, data?: ArrayBufferView): null | Uint8Array;

  print(x: string, isErr?: boolean): void;

  shared: ArrayBuffer;

  builtinModules: { [s: string]: object };

  setGlobalErrorHandler: (
    handler: (
      message: string,
      source: string,
      line: number,
      col: number,
      error: Error
    ) => void
  ) => void;

  setPromiseErrorExaminer: (handler: () => boolean) => void;

  /** Evaluate provided code in the current context.
   * It differs from eval(...) in that it does not create a new context.
   * Returns an array: [output, errInfo].
   * If an error occurs, `output` becomes null and `errInfo` is non-null.
   */
  evalContext(
    code: string
  ): [any, EvalErrorInfo | null] /* tslint:disable-line:no-any */;
}

const window = globalEval("this");
export const libdeno = window.libdeno as Libdeno;
