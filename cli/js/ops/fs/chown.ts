// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

import { sendSync, sendAsync } from "../dispatch_json.ts";
import { pathFromURL } from "../../util.ts";

export function chownSync(
  path: string | URL,
  uid: number | null,
  gid: number | null
): void {
  sendSync("op_chown", { path: pathFromURL(path), uid, gid });
}

export async function chown(
  path: string | URL,
  uid: number | null,
  gid: number | null
): Promise<void> {
  await sendAsync("op_chown", { path: pathFromURL(path), uid, gid });
}
