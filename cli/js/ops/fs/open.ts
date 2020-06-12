// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { sendSync, sendAsync } from "../dispatch_json.ts";
import { pathFromURL } from "../../util.ts";

export interface OpenOptions {
  read?: boolean;
  write?: boolean;
  append?: boolean;
  truncate?: boolean;
  create?: boolean;
  createNew?: boolean;
  /** Permissions to use if creating the file (defaults to `0o666`, before
   * the process's umask).
   * It's an error to specify mode without also setting create or createNew to `true`.
   * Ignored on Windows. */
  mode?: number;
}

export function openSync(path: string | URL, options: OpenOptions): number {
  const mode: number | undefined = options?.mode;
  path = pathFromURL(path);
  return sendSync("op_open", { path, options, mode });
}

export function open(
  path: string | URL,
  options: OpenOptions
): Promise<number> {
  const mode: number | undefined = options?.mode;
  path = pathFromURL(path);
  return sendAsync("op_open", {
    path,
    options,
    mode,
  });
}
