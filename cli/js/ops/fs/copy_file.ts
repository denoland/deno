// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { sendSync, sendAsync } from "../dispatch_json.ts";

export interface CopyFileOptions {
  createNew?: boolean;
  create?: boolean;
}

interface CopyFileArgs {
  createNew: boolean;
  create: boolean;
  oldpath?: string;
  newpath?: string;
}

export function copyFileSync(
  fromPath: string,
  toPath: string,
  options: CopyFileOptions = {}
): void {
  const args = checkOptions(options);
  args.oldpath = fromPath;
  args.newpath = toPath;
  sendSync("op_copy_file", args);
}

export async function copyFile(
  fromPath: string,
  toPath: string,
  options: CopyFileOptions = {}
): Promise<void> {
  const args = checkOptions(options);
  args.oldpath = fromPath;
  args.newpath = toPath;
  await sendAsync("op_copy_file", args);
}

/** Check we have a valid combination of options.
 *  @internal
 */
function checkOptions(options: CopyFileOptions): CopyFileArgs {
  const createNew = options.createNew;
  const create = options.create;
  return {
    ...options,
    createNew: !!createNew,
    create: createNew || create !== false,
  };
}
