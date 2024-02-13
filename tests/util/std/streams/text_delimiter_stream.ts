// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// This module is browser compatible.

import { createLPS } from "./_common.ts";

import type {
  DelimiterDisposition,
  DelimiterStreamOptions,
} from "./delimiter_stream.ts";

/** Transform a stream into a stream where each chunk is divided by a given delimiter.
 *
 * ```ts
 * import { TextDelimiterStream } from "https://deno.land/std@$STD_VERSION/streams/text_delimiter_stream.ts";
 * const res = await fetch("https://example.com");
 * const parts = res.body!
 *   .pipeThrough(new TextDecoderStream())
 *   .pipeThrough(new TextDelimiterStream("foo"));
 * ```
 */
export class TextDelimiterStream extends TransformStream<string, string> {
  #buf = "";
  #delimiter: string;
  #inspectIndex = 0;
  #matchIndex = 0;
  #delimLPS: Uint8Array;
  #disp: DelimiterDisposition;

  constructor(delimiter: string, options?: DelimiterStreamOptions) {
    super({
      transform: (chunk, controller) => {
        this.#handle(chunk, controller);
      },
      flush: (controller) => {
        controller.enqueue(this.#buf);
      },
    });

    this.#delimiter = delimiter;
    this.#delimLPS = createLPS(new TextEncoder().encode(delimiter));
    this.#disp = options?.disposition ?? "discard";
  }

  #handle(
    chunk: string,
    controller: TransformStreamDefaultController<string>,
  ) {
    this.#buf += chunk;
    let localIndex = 0;
    while (this.#inspectIndex < this.#buf.length) {
      if (chunk[localIndex] === this.#delimiter[this.#matchIndex]) {
        this.#inspectIndex++;
        localIndex++;
        this.#matchIndex++;
        if (this.#matchIndex === this.#delimiter.length) {
          // Full match
          const start = this.#inspectIndex - this.#delimiter.length;
          const end = this.#disp === "suffix" ? this.#inspectIndex : start;
          const copy = this.#buf.slice(0, end);
          controller.enqueue(copy);
          const shift = this.#disp === "prefix" ? start : this.#inspectIndex;
          this.#buf = this.#buf.slice(shift);
          this.#inspectIndex = this.#disp === "prefix"
            ? this.#delimiter.length
            : 0;
          this.#matchIndex = 0;
        }
      } else {
        if (this.#matchIndex === 0) {
          this.#inspectIndex++;
          localIndex++;
        } else {
          this.#matchIndex = this.#delimLPS[this.#matchIndex - 1];
        }
      }
    }
  }
}
