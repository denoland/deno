// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { sendSync, sendAsync } from "../dispatch_json.ts";

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
  sendSync("op_mkdir", mkdirArgs(path, options));
}

export async function mkdir(
  path: string,
  options?: MkdirOptions
): Promise<void> {
  await sendAsync("op_mkdir", mkdirArgs(path, options));
}
