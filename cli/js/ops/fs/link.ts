// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { sendSync, sendAsync } from "../dispatch_json.ts";

export function linkSync(oldname: string, newname: string): void {
  sendSync("op_link", { oldname, newname });
}

export async function link(oldname: string, newname: string): Promise<void> {
  await sendAsync("op_link", { oldname, newname });
}
