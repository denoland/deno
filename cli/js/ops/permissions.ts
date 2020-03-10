// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { sendSync } from "./dispatch_json.ts";

// TODO(bartlomieju): duplicated in `cli/js/permissions.ts` as
// `PermissionState
export type PermissionResponse = "granted" | "denied" | "prompt";

interface PermissionRequest {
  name: string;
  url?: string;
  path?: string;
}

export function query(desc: PermissionRequest): PermissionResponse {
  return sendSync("op_query_permission", desc).state;
}

export function revoke(desc: PermissionRequest): PermissionResponse {
  return sendSync("op_revoke_permission", desc).state;
}

export function request(desc: PermissionRequest): PermissionResponse {
  return sendSync("op_request_permission", desc).state;
}
