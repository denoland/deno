// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

export type Message = string | ArrayBuffer;
export type OutputFormat = "hex" | "base64";

export interface Hasher {
  update(data: Message): this;
  digest(): ArrayBuffer;
  toString(format?: OutputFormat): string;
}
