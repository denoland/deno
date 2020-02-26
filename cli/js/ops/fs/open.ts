// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { sendSync, sendAsync } from "../dispatch_json.ts";

export interface OpenOptions {
  read?: boolean;
  write?: boolean;
  append?: boolean;
  truncate?: boolean;
  create?: boolean;
  createNew?: boolean;
  mode?: number;
}

export type OpenMode = "r" | "r+" | "w" | "w+" | "a" | "a+" | "x" | "x+";

export function openSync(
  path: string,
  openMode: OpenMode | undefined,
  options: OpenOptions | undefined
): number {
  const mode: number | undefined = options?.mode;
  return sendSync("op_open", { path, options, openMode, mode });
}

export function open(
  path: string,
  openMode: OpenMode | undefined,
  options: OpenOptions | undefined
): Promise<number> {
  const mode: number | undefined = options?.mode;
  return sendAsync("op_open", {
    path,
    options,
    openMode,
    mode,
  });
}
