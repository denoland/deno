// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// This module is browser compatible.

import { Status, STATUS_TEXT } from "./status.ts";
import { deepMerge } from "../collections/deep_merge.ts";

/**
 * @deprecated (will be removed after 0.210.0)
 *
 * Internal utility for returning a standardized response, automatically defining the body, status code and status text, according to the response type.
 */
export function createCommonResponse(
  status: Status,
  body?: BodyInit | null,
  init?: ResponseInit,
): Response {
  if (body === undefined) {
    body = STATUS_TEXT[status];
  }
  init = deepMerge({
    status,
    statusText: STATUS_TEXT[status],
    // @ts-ignore Trust me
  }, init ?? {});
  return new Response(body, init);
}
