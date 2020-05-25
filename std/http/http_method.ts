// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

/** HTTP methods */
export enum Method {
  GET = "GET",
  HEAD = "HEAD",
  POST = "POST",
  PUT = "PUT",
  PATCH = "PATCH", // RFC 5789
  DELETE = "DELETE",
  CONNECT = "CONNECT",
  OPTIONS = "OPTIONS",
  TRACE = "TRACE",
}
