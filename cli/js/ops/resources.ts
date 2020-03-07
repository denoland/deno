// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { sendSync } from "./dispatch_json.ts";

export interface ResourceMap {
  [rid: number]: string;
}

/** Returns a map of open _file like_ resource ids along with their string
 * representation.
 */
export function resources(): ResourceMap {
  const res = sendSync("op_resources") as Array<[number, string]>;
  const resources: ResourceMap = {};
  for (const resourceTuple of res) {
    resources[resourceTuple[0]] = resourceTuple[1];
  }
  return resources;
}

/** Close the given resource ID. */
export function close(rid: number): void {
  sendSync("op_close", { rid });
}
