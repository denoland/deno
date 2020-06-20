// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { core } from "../../core.ts";

type MkdirArgs = { path: string; recursive: boolean; mode?: number };

function mkdirArgs(path: string, options?: MkdirOptions): MkdirArgs {
  const args: MkdirArgs = { path, recursive: false };
  if (options) {
    if (typeof options.recursive == "boolean") {
      args.recursive = options.recursive;
    }
    if (options.mode) {
      args.mode = options.mode;
    }
  }
  return args;
}

export interface MkdirOptions {
  recursive?: boolean;
  mode?: number;
}

export function mkdirSync(path: string, options?: MkdirOptions): void {
  core.dispatchJson.sendSync("op_mkdir", mkdirArgs(path, options));
}

export async function mkdir(
  path: string,
  options?: MkdirOptions
): Promise<void> {
  await core.dispatchJson.sendAsync("op_mkdir", mkdirArgs(path, options));
}
