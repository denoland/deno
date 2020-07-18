// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

import { sendSync, sendAsync } from "../dispatch_json.ts";
import { pathFromURL } from "../../util.ts";

export function chmodSync(path: string | URL, mode: number): void {
  sendSync("op_chmod", { path: pathFromURL(path), mode });
}

export async function chmod(path: string | URL, mode: number): Promise<void> {
  await sendAsync("op_chmod", { path: pathFromURL(path), mode });
}
