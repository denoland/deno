// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { sendSync, sendAsync } from "../dispatch_json.ts";
import { pathFromURL } from "../../util.ts";

export function chownSync(path: string | URL, uid: number, gid: number): void {
  path = pathFromURL(path);
  sendSync("op_chown", { path, uid, gid });
}

export async function chown(
  path: string | URL,
  uid: number,
  gid: number
): Promise<void> {
  path = pathFromURL(path);
  await sendAsync("op_chown", { path, uid, gid });
}
