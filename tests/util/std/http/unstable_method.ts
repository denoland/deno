// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// This module is browser compatible.

/**
 * @deprecated (will be removed in 0.210.0)
 *
 * Contains the constant {@linkcode HTTP_METHODS} and the type
 * {@linkcode HttpMethod} and the type guard {@linkcode isHttpMethod} for
 * working with HTTP methods with type safety.
 *
 * @module
 */

/**
 * @deprecated (will be removed in 0.210.0)
 *
 * A constant array of common HTTP methods.
 *
 * This list is compatible with Node.js `http` module.
 */
export const HTTP_METHODS = [
  "ACL",
  "BIND",
  "CHECKOUT",
  "CONNECT",
  "COPY",
  "DELETE",
  "GET",
  "HEAD",
  "LINK",
  "LOCK",
  "M-SEARCH",
  "MERGE",
  "MKACTIVITY",
  "MKCALENDAR",
  "MKCOL",
  "MOVE",
  "NOTIFY",
  "OPTIONS",
  "PATCH",
  "POST",
  "PROPFIND",
  "PROPPATCH",
  "PURGE",
  "PUT",
  "REBIND",
  "REPORT",
  "SEARCH",
  "SOURCE",
  "SUBSCRIBE",
  "TRACE",
  "UNBIND",
  "UNLINK",
  "UNLOCK",
  "UNSUBSCRIBE",
] as const;

/**
 * @deprecated (will be removed in 0.210.0)
 *
 * A type representing string literals of each of the common HTTP method.
 */
export type HttpMethod = typeof HTTP_METHODS[number];

/**
 * @deprecated (will be removed in 0.210.0)
 *
 * A type guard that determines if a value is a valid HTTP method.
 */
export function isHttpMethod(value: unknown): value is HttpMethod {
  return HTTP_METHODS.includes(value as HttpMethod);
}
