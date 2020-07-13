// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

import { sendAsync, sendSync } from "./dispatch_json.ts";

interface DoResponse {
  body: any;
}

export interface HttpClientOptions {}

export function createHttpClient(options: HttpClientOptions) {
  return sendSync("op_create_http_client", options);
}

export function doHttpRequest(rid: number): Promise<DoResponse> {
  return sendAsync("op_do_http_request", { rid });
}
