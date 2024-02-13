// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// This module is browser compatible.

import { toText } from "./to_text.ts";

export function toJson(
  readableStream: ReadableStream,
): Promise<unknown> {
  return toText(readableStream).then(JSON.parse);
}
