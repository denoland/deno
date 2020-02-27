// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { sendSync, sendAsync } from "../dispatch_json.ts";

export interface TruncateOptions {
  createNew?: boolean;
  create?: boolean;
}

function coerceLen(len?: number): number {
  if (!len) {
    return 0;
  }

  if (len < 0) {
    return 0;
  }

  return len;
}

interface TruncateArgs {
  createNew: boolean;
  create: boolean;
  path?: string;
  len?: number;
}

export function truncateSync(
  path: string,
  len?: number,
  options: TruncateOptions = {}
): void {
  const args = checkOptions(options);
  args.path = path;
  args.len = coerceLen(len);
  sendSync("op_truncate", args);
}

export async function truncate(
  path: string,
  len?: number,
  options: TruncateOptions = {}
): Promise<void> {
  const args = checkOptions(options);
  args.path = path;
  args.len = coerceLen(len);
  await sendAsync("op_truncate", args);
}

/** Check we have a valid combination of options.
 *  @internal
 */
function checkOptions(options: TruncateOptions): TruncateArgs {
  const createNew = options.createNew;
  const create = options.create;
  return {
    ...options,
    createNew: !!createNew,
    create: createNew || create !== false,
  };
}
