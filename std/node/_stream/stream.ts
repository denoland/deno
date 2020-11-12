// Copyright Node.js contributors. All rights reserved.

// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to
// deal in the Software without restriction, including without limitation the
// rights to use, copy, modify, merge, publish, distribute, sublicense, and/or
// sell copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:

// The above copyright notice and this permission notice shall be included in
// all copies or substantial portions of the Software.

// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
// FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS
// IN THE SOFTWARE.
import { Buffer } from "../buffer.ts";
import EventEmitter from "../events.ts";
import type Writable from "./writable.ts";
import { types } from "../util.ts";

class Stream extends EventEmitter {
  constructor() {
    super();
  }

  static _isUint8Array = types.isUint8Array;
  static _uint8ArrayToBuffer = (chunk: Uint8Array) => Buffer.from(chunk);

  pipe(dest: Writable, options: { end: boolean }) {
    // deno-lint-ignore no-this-alias
    const source = this;

    //TODO(Soremwar)
    //isStdio exist on stdin || stdout only, which extend from Duplex
    //if (!dest._isStdio && (options?.end ?? true)) {
    //Find an alternative to be able to pipe streams to stdin & stdout
    //Port them as well?
    if (options?.end ?? true) {
      source.on("end", onend);
      source.on("close", onclose);
    }

    let didOnEnd = false;
    function onend() {
      if (didOnEnd) return;
      didOnEnd = true;

      dest.end();
    }

    function onclose() {
      if (didOnEnd) return;
      didOnEnd = true;

      if (typeof dest.destroy === "function") dest.destroy();
    }

    // Don't leave dangling pipes when there are errors.
    function onerror(this: Stream, er: Error) {
      cleanup();
      if (this.listenerCount("error") === 0) {
        throw er; // Unhandled stream error in pipe.
      }
    }

    source.on("error", onerror);
    dest.on("error", onerror);

    // Remove all the event listeners that were added.
    function cleanup() {
      source.removeListener("end", onend);
      source.removeListener("close", onclose);

      source.removeListener("error", onerror);
      dest.removeListener("error", onerror);

      source.removeListener("end", cleanup);
      source.removeListener("close", cleanup);

      dest.removeListener("close", cleanup);
    }

    source.on("end", cleanup);
    source.on("close", cleanup);

    dest.on("close", cleanup);
    dest.emit("pipe", source);

    return dest;
  }
}

export default Stream;
