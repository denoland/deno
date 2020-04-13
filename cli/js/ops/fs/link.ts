// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { sendSync, sendAsync } from "../dispatch_json.ts";

export function linkSync(oldpath: string, newpath: string): void {
  sendSync("op_link", { oldpath, newpath });
}

export async function link(oldpath: string, newpath: string): Promise<void> {
  await sendAsync("op_link", { oldpath, newpath });
}
