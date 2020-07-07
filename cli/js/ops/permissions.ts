// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

import { sendSync } from "./dispatch_json.ts";
import { PermissionState } from "../permissions.ts";

interface PermissionRequest {
  name: string;
  url?: string;
  path?: string;
}

export function query(desc: PermissionRequest): PermissionState {
  return sendSync("op_query_permission", desc).state;
}

export function revoke(desc: PermissionRequest): PermissionState {
  return sendSync("op_revoke_permission", desc).state;
}

export function request(desc: PermissionRequest): PermissionState {
  return sendSync("op_request_permission", desc).state;
}
