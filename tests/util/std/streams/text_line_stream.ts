// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// This module is browser compatible.

export interface TextLineStreamOptions {
  /**
   * Allow splitting by `\r`.
   *
   * @default {false}
   */
  allowCR?: boolean;
}

/**
 * Transform a stream into a stream where each chunk is divided by a newline,
 * be it `\n` or `\r\n`. `\r` can be enabled via the `allowCR` option.
 *
 * @example
 * ```ts
 * import { TextLineStream } from "https://deno.land/std@$STD_VERSION/streams/text_line_stream.ts";
 * const res = await fetch("https://example.com");
 * const lines = res.body!
 *   .pipeThrough(new TextDecoderStream())
 *   .pipeThrough(new TextLineStream());
 * ```
 */
export class TextLineStream extends TransformStream<string, string> {
  #currentLine = "";

  constructor(options: TextLineStreamOptions = { allowCR: false }) {
    super({
      transform: (chars, controller) => {
        chars = this.#currentLine + chars;

        while (true) {
          const lfIndex = chars.indexOf("\n");
          const crIndex = options.allowCR ? chars.indexOf("\r") : -1;

          if (
            crIndex !== -1 && crIndex !== (chars.length - 1) &&
            (lfIndex === -1 || (lfIndex - 1) > crIndex)
          ) {
            controller.enqueue(chars.slice(0, crIndex));
            chars = chars.slice(crIndex + 1);
            continue;
          }

          if (lfIndex === -1) break;

          const endIndex = chars[lfIndex - 1] === "\r" ? lfIndex - 1 : lfIndex;
          controller.enqueue(chars.slice(0, endIndex));
          chars = chars.slice(lfIndex + 1);
        }

        this.#currentLine = chars;
      },
      flush: (controller) => {
        if (this.#currentLine === "") return;
        const currentLine = options.allowCR && this.#currentLine.endsWith("\r")
          ? this.#currentLine.slice(0, -1)
          : this.#currentLine;
        controller.enqueue(currentLine);
      },
    });
  }
}
