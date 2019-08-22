// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { assert } from "./util";
import { sendSync, msg, flatbuffers } from "./dispatch_flatbuffers";

export interface ResourceMap {
  [rid: number]: string;
}

/** Returns a map of open _file like_ resource ids along with their string
 * representation.
 */
export function resources(): ResourceMap {
  const builder = flatbuffers.createBuilder();
  const inner = msg.Resource.createResource(builder, 0, 0);
  const baseRes = sendSync(builder, msg.Any.Resources, inner);
  assert(baseRes !== null);
  assert(msg.Any.ResourcesRes === baseRes!.innerType());
  const res = new msg.ResourcesRes();
  assert(baseRes!.inner(res) !== null);

  const resources: ResourceMap = {};

  for (let i = 0; i < res.resourcesLength(); i++) {
    const item = res.resources(i)!;
    resources[item.rid()!] = item.repr()!;
  }

  return resources;
}
