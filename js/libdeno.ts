import { RawSourceMap } from "./types";
import { globalEval } from "./global_eval";

// The libdeno functions are moved so that users can't access them.
type MessageCallback = (msg: Uint8Array) => void;
export type PromiseRejectEvent =
  | "RejectWithNoHandler"
  | "HandlerAddedAfterReject"
  | "ResolveAfterResolved"
  | "RejectAfterResolved";

interface Libdeno {
  // TODO Remove recv(), it is unused.
  recv(cb: MessageCallback): void;

  send(control: ArrayBufferView, data?: ArrayBufferView): null | Uint8Array;

  print(x: string, isErr?: boolean): void;

  runMicrotasks(): void;
  exit(code: number): void;

  setGlobalErrorHandler: (
    handler: (
      message: string,
      source: string,
      line: number,
      col: number,
      error: Error
    ) => void
  ) => void;

  setPromiseRejectHandler: (
    handler: (
      error: Error | string,
      event: PromiseRejectEvent,
      /* tslint:disable-next-line:no-any */
      promise: Promise<any>
    ) => void
  ) => void;

  mainSource: string;
  mainSourceMap: RawSourceMap;
}

const window = globalEval("this");
export const libdeno = window.libdeno as Libdeno;
