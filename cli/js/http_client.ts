// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

import {
  createHttpClient,
  doHttpRequest,
  HttpClientOptions,
} from "./ops/http_client.ts";

export class HttpClient {
  readonly rid: number;

  constructor(options: HttpClientOptions) {
    this.rid = createHttpClient(options) as number;
  }

  do(): any {
    return doHttpRequest(this.rid);
  }
}
