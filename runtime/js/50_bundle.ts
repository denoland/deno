// Copyright 2018-2025 the Deno authors. MIT license.
/// <reference path="../../cli/tsc/dts/lib.deno.unstable.d.ts" />
import { op_bundle } from "ext:core/ops";

export async function bundle(
  options: Deno.bundle.Options,
): Promise<Deno.bundle.Result> {
  const result = {
    success: false,
    ...await op_bundle(options),
  };
  result.success = result.errors.length === 0;

  for (const file of result.outputFiles ?? []) {
    if (file.contents?.length === 0) {
      delete file.contents;
    }
  }
  if (result.outputFiles?.length === 0) {
    delete result.outputFiles;
  }
  return result;
}
