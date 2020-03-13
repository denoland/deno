// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { sendSync } from "./dispatch_json.ts";

export interface ResourceMap {
  [rid: number]: string;
}

export function resources(): ResourceMap {
  const res = sendSync("op_resources") as Array<[number, string]>;
  const resources: ResourceMap = {};
  for (const resourceTuple of res) {
    resources[resourceTuple[0]] = resourceTuple[1];
  }
  return resources;
}

export function close(rid: number): void {
  sendSync("op_close", { rid });
}
