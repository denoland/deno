// Copyright 2018-2025 the Deno authors. MIT license.
import { core, primordials } from "ext:core/mod.js";
import { op_bundle } from "ext:core/ops";

export async function bundle(options) {
  const result = await op_bundle(options);
  result.success = result.errors.length === 0;
  
  for (const file of result.outputFiles ?? []) {
    if (file.contents.length === 0) {
      delete file.contents;
    }
  }
  if (result.outputFiles?.length === 0) {
    delete result.outputFiles;
  }
  return result;
}