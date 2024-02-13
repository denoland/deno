// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { stringify } from "./stringify.ts";

export interface CsvStringifyStreamOptions {
  /**
   * Delimiter used to separate values.
   *
   * @default {","}
   */
  readonly separator?: string;

  /**
   * A list of columns to be included in the output.
   *
   * If you want to stream objects, this option is required.
   */
  readonly columns?: Array<string>;
}

/**
 * Convert each chunk to a CSV record.
 *
 * @example
 * ```ts
 * import { CsvStringifyStream } from "https://deno.land/std@$STD_VERSION/csv/csv_stringify_stream.ts";
 *
 * const file = await Deno.open("data.csv", { create: true, write: true });
 * const readable = ReadableStream.from([
 *   { id: 1, name: "one" },
 *   { id: 2, name: "two" },
 *   { id: 3, name: "three" },
 * ]);
 *
 * await readable
 *   .pipeThrough(new CsvStringifyStream({ columns: ["id", "name"] }))
 *   .pipeThrough(new TextEncoderStream())
 *   .pipeTo(file.writable);
 * ````
 */
export class CsvStringifyStream<TOptions extends CsvStringifyStreamOptions>
  extends TransformStream<
    TOptions["columns"] extends Array<string> ? Record<string, unknown>
      : Array<unknown>,
    string
  > {
  constructor(options?: TOptions) {
    const {
      separator,
      columns = [],
    } = options ?? {};

    super(
      {
        start(controller) {
          if (columns && columns.length > 0) {
            try {
              controller.enqueue(
                stringify([columns], { separator, headers: false }),
              );
            } catch (error) {
              controller.error(error);
            }
          }
        },
        transform(chunk, controller) {
          try {
            controller.enqueue(
              stringify([chunk], { separator, headers: false, columns }),
            );
          } catch (error) {
            controller.error(error);
          }
        },
      },
    );
  }
}
