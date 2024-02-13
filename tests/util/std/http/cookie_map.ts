// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// This module is browser compatible.

/**
 * @deprecated (will be removed after 0.210.0) Import from {@link https://deno.land/std/crypto/unstable_cookie_map.ts} instead.
 *
 * Provides a iterable map interfaces for managing cookies server side.
 *
 * @example
 * To access the keys in a request and have any set keys available for creating
 * a response:
 *
 * ```ts
 * import {
 *   CookieMap,
 *   mergeHeaders
 * } from "https://deno.land/std@$STD_VERSION/http/unstable_cookie_map.ts";
 *
 * const request = new Request("https://localhost/", {
 *   headers: { "cookie": "foo=bar; bar=baz;"}
 * });
 *
 * const cookies = new CookieMap(request, { secure: true });
 * console.log(cookies.get("foo")); // logs "bar"
 * cookies.set("session", "1234567", { secure: true });
 *
 * const response = new Response("hello", {
 *   headers: mergeHeaders({
 *     "content-type": "text/plain",
 *   }, cookies),
 * });
 * ```
 *
 * @example
 * To have automatic management of cryptographically signed cookies, you can use
 * the {@linkcode SecureCookieMap} instead of {@linkcode CookieMap}. The biggest
 * difference is that the methods operate async in order to be able to support
 * async signing and validation of cookies:
 *
 * ```ts
 * import {
 *   SecureCookieMap,
 *   mergeHeaders,
 *   type KeyRing,
 * } from "https://deno.land/std@$STD_VERSION/http/unstable_cookie_map.ts";
 *
 * const request = new Request("https://localhost/", {
 *   headers: { "cookie": "foo=bar; bar=baz;"}
 * });
 *
 * // The keys must implement the `KeyRing` interface.
 * declare const keys: KeyRing;
 *
 * const cookies = new SecureCookieMap(request, { keys, secure: true });
 * console.log(await cookies.get("foo")); // logs "bar"
 * // the cookie will be automatically signed using the supplied key ring.
 * await cookies.set("session", "1234567");
 *
 * const response = new Response("hello", {
 *   headers: mergeHeaders({
 *     "content-type": "text/plain",
 *   }, cookies),
 * });
 * ```
 *
 * In addition, if you have a {@linkcode Response} or {@linkcode Headers} for a
 * response at construction of the cookies object, they can be passed and any
 * set cookies will be added directly to those headers:
 *
 * ```ts
 * import { CookieMap } from "https://deno.land/std@$STD_VERSION/http/unstable_cookie_map.ts";
 *
 * const request = new Request("https://localhost/", {
 *   headers: { "cookie": "foo=bar; bar=baz;"}
 * });
 *
 * const response = new Response("hello", {
 *   headers: { "content-type": "text/plain" },
 * });
 *
 * const cookies = new CookieMap(request, { response });
 * console.log(cookies.get("foo")); // logs "bar"
 * cookies.set("session", "1234567");
 * ```
 *
 * @module
 */

import {
  CookieMap as CookieMap_,
  cookieMapHeadersInitSymbol as cookieMapHeadersInitSymbol_,
  type CookieMapOptions as CookieMapOptions_,
  type CookieMapSetDeleteOptions as CookieMapSetDeleteOptions_,
  type Data as Data_,
  type Headered as Headered_,
  type KeyRing as KeyRing_,
  type Mergeable as Mergeable_,
  mergeHeaders as mergeHeaders_,
  SecureCookieMap as SecureCookieMap_,
  type SecureCookieMapGetOptions as SecureCookieMapGetOptions_,
  type SecureCookieMapOptions as SecureCookieMapOptions_,
  type SecureCookieMapSetDeleteOptions as SecureCookieMapSetDeleteOptions_,
} from "./unstable_cookie_map.ts";

/**
 * @deprecated (will be removed after 0.210.0) Import from {@link https://deno.land/std/crypto/unstable_cookie_map.ts} instead.
 */
export type CookieMapOptions = CookieMapOptions_;
/**
 * @deprecated (will be removed after 0.210.0) Import from {@link https://deno.land/std/crypto/unstable_cookie_map.ts} instead.
 */
export type CookieMapSetDeleteOptions = CookieMapSetDeleteOptions_;
/**
 * @deprecated (will be removed after 0.210.0) Import from {@link https://deno.land/std/crypto/unstable_cookie_map.ts} instead.
 */
export type Headered = Headered_;
/**
 * @deprecated (will be removed after 0.210.0) Import from {@link https://deno.land/std/crypto/unstable_cookie_map.ts} instead.
 */
export type Mergeable = Mergeable_;
/**
 * @deprecated (will be removed after 0.210.0) Import from {@link https://deno.land/std/crypto/unstable_cookie_map.ts} instead.
 */
export type SecureCookieMapGetOptions = SecureCookieMapGetOptions_;
/**
 * @deprecated (will be removed after 0.210.0) Import from {@link https://deno.land/std/crypto/unstable_cookie_map.ts} instead.
 */
export type SecureCookieMapOptions = SecureCookieMapOptions_;
/**
 * @deprecated (will be removed after 0.210.0) Import from {@link https://deno.land/std/crypto/unstable_cookie_map.ts} instead.
 */
export type SecureCookieMapSetDeleteOptions = SecureCookieMapSetDeleteOptions_;

/**
 * @deprecated (will be removed after 0.210.0) Import from {@link https://deno.land/std/crypto/unstable_cookie_map.ts} instead.
 *
 * Symbol which is used in {@link mergeHeaders} to extract a
 * `[string | string][]` from an instance to generate the final set of
 * headers.
 */
export const cookieMapHeadersInitSymbol = cookieMapHeadersInitSymbol_;

/**
 * @deprecated (will be removed after 0.210.0) Import from {@link https://deno.land/std/crypto/unstable_cookie_map.ts} instead.
 *
 * Allows merging of various sources of headers into a final set of headers
 * which can be used in a {@linkcode Response}.
 *
 * Note, that unlike when passing a `Response` or {@linkcode Headers} used in a
 * response to {@linkcode CookieMap} or {@linkcode SecureCookieMap}, merging
 * will not ensure that there are no other `Set-Cookie` headers from other
 * sources, it will simply append the various headers together.
 */
export const mergeHeaders = mergeHeaders_;

/**
 * @deprecated (will be removed after 0.210.0) Import from {@link https://deno.land/std/crypto/unstable_cookie_map.ts} instead.
 *
 * Provides a way to manage cookies in a request and response on the server
 * as a single iterable collection.
 *
 * The methods and properties align to {@linkcode Map}. When constructing a
 * {@linkcode Request} or {@linkcode Headers} from the request need to be
 * provided, as well as optionally the {@linkcode Response} or `Headers` for the
 * response can be provided. Alternatively the {@linkcode mergeHeaders}
 * function can be used to generate a final set of headers for sending in the
 * response.
 */
export const CookieMap = CookieMap_;

/**
 * @deprecated (will be removed after 0.210.0) Import from {@link https://deno.land/std/crypto/unstable_cookie_map.ts} instead.
 * Types of data that can be signed cryptographically.
 */
export type Data = Data_;

/**
 * @deprecated (will be removed after 0.210.0) Import from {@link https://deno.land/std/crypto/unstable_cookie_map.ts} instead.
 *
 * An interface which describes the methods that {@linkcode SecureCookieMap} uses to sign and verify cookies.
 */
export type KeyRing = KeyRing_;

/**
 * @deprecated (will be removed after 0.210.0) Import from {@link https://deno.land/std/crypto/unstable_cookie_map.ts} instead.
 *
 * Provides an way to manage cookies in a request and response on the server
 * as a single iterable collection, as well as the ability to sign and verify
 * cookies to prevent tampering.
 *
 * The methods and properties align to {@linkcode Map}, but due to the need to
 * support asynchronous cryptographic keys, all the APIs operate async. When
 * constructing a {@linkcode Request} or {@linkcode Headers} from the request
 * need to be provided, as well as optionally the {@linkcode Response} or
 * `Headers` for the response can be provided. Alternatively the
 * {@linkcode mergeHeaders} function can be used to generate a final set
 * of headers for sending in the response.
 *
 * On construction, the optional set of keys implementing the
 * {@linkcode KeyRing} interface. While it is optional, if you don't plan to use
 * keys, you might want to consider using just the {@linkcode CookieMap}.
 *
 * @example
 */
export const SecureCookieMap = SecureCookieMap_;
