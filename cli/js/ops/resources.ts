// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { core } from "../core.ts";

export interface ResourceMap {
  [rid: number]: string;
}

export function resources(): ResourceMap {
  const res = core.dispatchJson.sendSync("op_resources") as Array<
    [number, string]
  >;
  const resources: ResourceMap = {};
  for (const resourceTuple of res) {
    resources[resourceTuple[0]] = resourceTuple[1];
  }
  return resources;
}

export function close(rid: number): void {
  core.dispatchJson.sendSync("op_close", { rid });
}
