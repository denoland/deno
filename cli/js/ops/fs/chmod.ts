// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { sendSync, sendAsync } from "../dispatch_json.ts";
import { pathFromURL } from "../../util.ts";

export function chmodSync(path: string | URL, mode: number): void {
  path = pathFromURL(path);
  sendSync("op_chmod", { path, mode });
}

export async function chmod(path: string | URL, mode: number): Promise<void> {
  path = pathFromURL(path);
  await sendAsync("op_chmod", { path, mode });
}
