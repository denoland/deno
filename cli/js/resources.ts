// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import * as dispatch from "./dispatch.ts";
import { sendSync } from "./dispatch_json.ts";

export interface ResourceMap {
  [rid: number]: string;
}

/** Returns a map of open _file like_ resource ids along with their string
 * representation.
 */
export function resources(): ResourceMap {
  const res = sendSync(dispatch.OP_RESOURCES) as Array<[number, string]>;
  const resources: ResourceMap = {};
  for (const resourceTuple of res) {
    resources[resourceTuple[0]] = resourceTuple[1];
  }
  return resources;
}
