// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import * as body from "./body.ts";
import * as domTypes from "./dom_types.d.ts";
import { ReadableStreamImpl } from "./streams/readable_stream.ts";

function byteUpperCase(s: string): string {
  return String(s).replace(/[a-z]/g, function byteUpperCaseReplace(c): string {
    return c.toUpperCase();
  });
}

function normalizeMethod(m: string): string {
  const u = byteUpperCase(m);
  if (
    u === "DELETE" ||
    u === "GET" ||
    u === "HEAD" ||
    u === "OPTIONS" ||
    u === "POST" ||
    u === "PUT"
  ) {
    return u;
  }
  return m;
}

export class Request extends body.Body implements domTypes.Request {
  public method: string;
  public url: string;
  public credentials?: "omit" | "same-origin" | "include";
  public headers: Headers;

  constructor(input: domTypes.RequestInfo, init?: domTypes.RequestInit) {
    if (arguments.length < 1) {
      throw TypeError("Not enough arguments");
    }

    if (!init) {
      init = {};
    }

    let b: body.BodySource;

    // prefer body from init
    if (init.body) {
      b = init.body;
    } else if (input instanceof Request && input._bodySource) {
      if (input.bodyUsed) {
        throw TypeError(body.BodyUsedError);
      }
      b = input._bodySource;
    } else if (typeof input === "object" && "body" in input && input.body) {
      if (input.bodyUsed) {
        throw TypeError(body.BodyUsedError);
      }
      b = input.body;
    } else {
      b = "";
    }

    let headers: Headers;

    // prefer headers from init
    if (init.headers) {
      headers = new Headers(init.headers);
    } else if (input instanceof Request) {
      headers = input.headers;
    } else {
      headers = new Headers();
    }

    const contentType = headers.get("content-type") || "";
    super(b, contentType);
    this.headers = headers;

    // readonly attribute ByteString method;
    this.method = "GET";

    // readonly attribute USVString url;
    this.url = "";

    // readonly attribute RequestCredentials credentials;
    this.credentials = "omit";

    if (input instanceof Request) {
      if (input.bodyUsed) {
        throw TypeError(body.BodyUsedError);
      }
      this.method = input.method;
      this.url = input.url;
      this.headers = new Headers(input.headers);
      this.credentials = input.credentials;
      this._stream = input._stream;
    } else if (typeof input === "string") {
      this.url = input;
    }

    if (init && "method" in init) {
      this.method = normalizeMethod(init.method as string);
    }

    if (
      init &&
      "credentials" in init &&
      init.credentials &&
      ["omit", "same-origin", "include"].indexOf(init.credentials) !== -1
    ) {
      this.credentials = init.credentials;
    }
  }

  public clone(): domTypes.Request {
    if (this.bodyUsed) {
      throw TypeError(body.BodyUsedError);
    }

    const iterators = this.headers.entries();
    const headersList: Array<[string, string]> = [];
    for (const header of iterators) {
      headersList.push(header);
    }

    let body2 = this._bodySource;

    if (this._bodySource instanceof ReadableStreamImpl) {
      const tees = this._bodySource.tee();
      this._stream = this._bodySource = tees[0];
      body2 = tees[1];
    }

    const cloned = new Request(this.url, {
      body: body2,
      method: this.method,
      headers: new Headers(headersList),
      credentials: this.credentials,
    });
    return cloned;
  }
}
