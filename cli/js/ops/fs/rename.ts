// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { sendSync, sendAsync } from "../dispatch_json.ts";

export interface RenameOptions {
  createNew?: boolean;
}

interface RenameArgs {
  oldpath?: string;
  newpath?: string;
  createNew: boolean;
}

export function renameSync(
  oldpath: string,
  newpath: string,
  options: RenameOptions = {}
): void {
  const args = checkOptions(options);
  args.oldpath = oldpath;
  args.newpath = newpath;
  sendSync("op_rename", args);
}

export async function rename(
  oldpath: string,
  newpath: string,
  options: RenameOptions = {}
): Promise<void> {
  const args = checkOptions(options);
  args.oldpath = oldpath;
  args.newpath = newpath;
  await sendAsync("op_rename", args);
}

/** Check we have a valid combination of options.
 *  @internal
 */
function checkOptions(options: RenameOptions): RenameArgs {
  const createNew = options.createNew;
  return {
    createNew: !!createNew,
  };
}
