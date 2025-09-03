// Copyright 2018-2025 the Deno authors. MIT license.
/// <reference path="../../cli/tsc/dts/lib.deno.unstable.d.ts" />
import { op_bundle } from "ext:core/ops";
import { primordials } from "ext:core/mod.js";
import { TextDecoder } from "ext:deno_web/08_text_encoding.js";

const { SafeArrayIterator, Uint8Array } = primordials;

const decoder = new TextDecoder();

export async function bundle(
  options: Deno.bundle.Options,
): Promise<Deno.bundle.Result> {
  const result = {
    success: false,
    ...await op_bundle(
      options,
    ),
  };
  result.success = result.errors.length === 0;

  for (
    const f of new SafeArrayIterator(
      // deno-lint-ignore no-explicit-any
      result.outputFiles as any ?? [],
    )
  ) {
    // deno-lint-ignore no-explicit-any
    const file = f as any;
    if (file.contents?.length === 0) {
      delete file.contents;
    } else {
      file.contents = decoder.decode(new Uint8Array(file.contents));
    }
  }
  if (result.outputFiles?.length === 0) {
    delete result.outputFiles;
  }
  return result;
}
