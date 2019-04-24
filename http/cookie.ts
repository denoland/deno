// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { ServerRequest } from "./server.ts";

export interface Cookie {
  [key: string]: string;
}

/* Parse the cookie of the Server Request */
export function getCookie(rq: ServerRequest): Cookie {
  if (rq.headers.has("Cookie")) {
    const out: Cookie = {};
    const c = rq.headers.get("Cookie").split(";");
    for (const kv of c) {
      const cookieVal = kv.split("=");
      const key = cookieVal.shift().trim();
      out[key] = cookieVal.join("");
    }
    return out;
  }
  return {};
}
