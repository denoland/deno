// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { sendSync, sendAsync } from "../dispatch_json.ts";

export interface OpenOptions {
  read?: boolean;
  write?: boolean;
  append?: boolean;
  truncate?: boolean;
  create?: boolean;
  createNew?: boolean;
}

export type OpenMode = "r" | "r+" | "w" | "w+" | "a" | "a+" | "x" | "x+";

export function openSync(
  path: string,
  mode: OpenMode | undefined,
  options: OpenOptions | undefined
): number {
  return sendSync("op_open", { path, options, mode });
}

export async function open(
  path: string,
  mode: OpenMode | undefined,
  options: OpenOptions | undefined
): Promise<number> {
  return await sendAsync("op_open", {
    path,
    options,
    mode
  });
}
