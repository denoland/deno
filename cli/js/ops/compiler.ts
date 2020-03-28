// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

import { sendAsync, sendSync } from "./dispatch_json.ts";
import { TextDecoder, TextEncoder } from "../web/text_encoding.ts";
import { core } from "../core.ts";

export function resolveModules(
  specifiers: string[],
  referrer?: string
): string[] {
  return sendSync("op_resolve_modules", { specifiers, referrer });
}

export function fetchSourceFiles(
  specifiers: string[],
  referrer?: string
): Promise<
  Array<{
    url: string;
    filename: string;
    mediaType: number;
    sourceCode: string;
  }>
> {
  return sendAsync("op_fetch_source_files", {
    specifiers,
    referrer,
  });
}

const encoder = new TextEncoder();
const decoder = new TextDecoder();

export function getAsset(name: string): string {
  const opId = core.ops()["op_fetch_asset"];
  // We really don't want to depend on JSON dispatch during snapshotting, so
  // this op exchanges strings with Rust as raw byte arrays.
  const sourceCodeBytes = core.dispatch(opId, encoder.encode(name));
  return decoder.decode(sourceCodeBytes!);
}

export function cache(
  extension: string,
  moduleId: string,
  contents: string
): void {
  sendSync("op_cache", {
    extension,
    moduleId,
    contents,
  });
}
