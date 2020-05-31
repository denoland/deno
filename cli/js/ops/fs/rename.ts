// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { sendSync, sendAsync } from "../dispatch_json.ts";
import { pathFromURL } from "../../util.ts";

type RenameArgs = {
  oldpath: string;
  newpath: string;
};

function renameArgs(oldpath: string | URL, newpath: string | URL): RenameArgs {
  if (oldpath instanceof URL) {
    oldpath = pathFromURL(oldpath);
  }
  if (newpath instanceof URL) {
    newpath = pathFromURL(newpath);
  }
  return {
    oldpath,
    newpath,
  };
}

export function renameSync(oldpath: string | URL, newpath: string | URL): void {
  sendSync("op_rename", renameArgs(oldpath, newpath));
}

export async function rename(
  oldpath: string | URL,
  newpath: string | URL
): Promise<void> {
  await sendAsync("op_rename", renameArgs(oldpath, newpath));
}
