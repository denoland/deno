import { RawSourceMap } from "./types";
import { globalEval } from "./global-eval";

// The libdeno functions are moved so that users can't access them.
type MessageCallback = (msg: Uint8Array) => void;
interface Libdeno {
  recv(cb: MessageCallback): void;

  send(control: ArrayBufferView, data?: ArrayBufferView): null | Uint8Array;

  print(x: string, isErr?: boolean): void;

  setGlobalErrorHandler: (
    handler: (
      message: string,
      source: string,
      line: number,
      col: number,
      error: Error
    ) => void
  ) => void;

  mainSource: string;
  mainSourceMap: RawSourceMap;
}

const window = globalEval("this");
export const libdeno = window.libdeno as Libdeno;
