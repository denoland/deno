// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { JsonOp } from "./dispatch_json.ts";

export interface ResourceMap {
  [rid: number]: string;
}

const OP_RESOURCES = new JsonOp("resources");

/** Returns a map of open _file like_ resource ids along with their string
 * representation.
 */
export function resources(): ResourceMap {
  const res = OP_RESOURCES.sendSync() as Array<[number, string]>;
  const resources: ResourceMap = {};
  for (const resourceTuple of res) {
    resources[resourceTuple[0]] = resourceTuple[1];
  }
  return resources;
}
