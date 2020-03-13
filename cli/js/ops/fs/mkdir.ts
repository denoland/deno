// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { sendSync, sendAsync } from "../dispatch_json.ts";

type MkdirArgs = { path: string; recursive: boolean; mode?: number };

// TODO(ry) The complexity in argument parsing is to support deprecated forms of
// mkdir and mkdirSync.
function mkdirArgs(
  path: string,
  optionsOrRecursive?: MkdirOptions | boolean,
  mode?: number
): MkdirArgs {
  const args: MkdirArgs = { path, recursive: false };
  if (typeof optionsOrRecursive == "boolean") {
    args.recursive = optionsOrRecursive;
    if (mode) {
      args.mode = mode;
    }
  } else if (optionsOrRecursive) {
    if (typeof optionsOrRecursive.recursive == "boolean") {
      args.recursive = optionsOrRecursive.recursive;
    }
    if (optionsOrRecursive.mode) {
      args.mode = optionsOrRecursive.mode;
    }
  }
  return args;
}

export interface MkdirOptions {
  recursive?: boolean;
  mode?: number;
}

export function mkdirSync(
  path: string,
  optionsOrRecursive?: MkdirOptions | boolean,
  mode?: number
): void {
  sendSync("op_mkdir", mkdirArgs(path, optionsOrRecursive, mode));
}

export async function mkdir(
  path: string,
  optionsOrRecursive?: MkdirOptions | boolean,
  mode?: number
): Promise<void> {
  await sendAsync("op_mkdir", mkdirArgs(path, optionsOrRecursive, mode));
}
