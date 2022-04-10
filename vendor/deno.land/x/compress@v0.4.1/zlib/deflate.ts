// from https://github.com/nodeca/pako
import * as zlibDeflate from "./zlib/deflate.ts";
import { concatUint8Array } from "../utils/uint8.ts";
import { CODE, message as msg } from "./zlib/messages.ts";
import ZStream from "./zlib/zstream.ts";
import STATUS from "./zlib/status.ts";

export interface DeflateOptions {
  level?: number;
  method?: number;
  chunkSize?: number;
  windowBits?: number;
  memLevel?: number;
  strategy?: number;
  to?: string;
  raw?: boolean;
  gzip?: boolean;
  dictionary?: Uint8Array;
  header?: zlibDeflate.Header;
}

export class Deflate {
  err: STATUS = 0; // error code, if happens (0 = Z_OK)
  msg = ""; // error message
  ended = false; // used to avoid multiple onEnd() calls
  strm: ZStream;
  _dict_set = false;
  options: any;

  constructor(options: DeflateOptions = {}) {
    this.options = Object.assign({
      level: STATUS.Z_DEFAULT_COMPRESSION,
      method: STATUS.Z_DEFLATED,
      chunkSize: 16384,
      windowBits: 15,
      memLevel: 8,
      strategy: STATUS.Z_DEFAULT_STRATEGY,
      to: "",
    }, options);

    const opt = this.options;

    if (opt.raw && (opt.windowBits > 0)) {
      opt.windowBits = -opt.windowBits;
    } else if (opt.gzip && (opt.windowBits > 0) && (opt.windowBits < 16)) {
      opt.windowBits += 16;
    }

    this.strm = new ZStream();
    this.strm.avail_out = 0;

    let status = zlibDeflate.deflateInit2(
      this.strm,
      opt.level,
      opt.method,
      opt.windowBits,
      opt.memLevel,
      opt.strategy,
    );

    if (status !== STATUS.Z_OK) {
      throw new Error(msg[status]);
    }

    if (opt.header) {
      zlibDeflate.deflateSetHeader(this.strm, opt.header);
    }

    if (opt.dictionary) {
      status = zlibDeflate.deflateSetDictionary(this.strm, opt.dictionary);

      if (status !== STATUS.Z_OK) {
        throw new Error(msg[status]);
      }

      this._dict_set = true;
    }
  }

  push(data: Uint8Array, mode: boolean | number): Uint8Array {
    const strm = this.strm;
    const chunkSize = this.options.chunkSize;
    const chunks: Uint8Array[] = [];
    let status;

    if (this.ended) {
      throw new Error("can not call after ended");
    }

    const _mode = mode === ~~mode
      ? mode
      : (mode === true ? STATUS.Z_FINISH : STATUS.Z_NO_FLUSH);

    strm.input = data;
    strm.next_in = 0;
    strm.avail_in = strm.input.length;

    do {
      if (strm.avail_out === 0) {
        strm.output = new Uint8Array(chunkSize);
        strm.next_out = 0;
        strm.avail_out = chunkSize;
      }
      status = zlibDeflate.deflate(strm, _mode); /* no bad return value */

      if (status !== STATUS.Z_STREAM_END && status !== STATUS.Z_OK) {
        this.ended = true;
        throw new Error(this.strm.msg);
      }
      if (
        strm.avail_out === 0 ||
        (strm.avail_in === 0 &&
          (_mode === STATUS.Z_FINISH || _mode === STATUS.Z_SYNC_FLUSH))
      ) {
        chunks.push(strm.output!.subarray(0, strm.next_out));
      }
    } while (
      (strm.avail_in > 0 || strm.avail_out === 0) &&
      status !== STATUS.Z_STREAM_END
    );

    // Finalize on the last chunk.
    if (_mode === STATUS.Z_FINISH) {
      status = zlibDeflate.deflateEnd(this.strm);
      this.ended = true;
      if (status !== STATUS.Z_OK) throw new Error(this.strm.msg);
    }

    // callback interim results if Z_SYNC_FLUSH.
    if (_mode === STATUS.Z_SYNC_FLUSH) {
      strm.avail_out = 0;
    }

    return concatUint8Array(chunks);
  }
}

export function deflate(input: Uint8Array, options: DeflateOptions = {}) {
  const deflator = new Deflate(options);
  const result = deflator.push(input, true);
  // That will never happens, if you don't cheat with options :)
  if (deflator.err) throw deflator.msg || msg[deflator.err as CODE];
  return result;
}

export function deflateRaw(input: Uint8Array, options: DeflateOptions = {}) {
  options.raw = true;
  return deflate(input, options);
}

export function gzip(input: Uint8Array, options: DeflateOptions = {}) {
  options.gzip = true;
  return deflate(input, options);
}
