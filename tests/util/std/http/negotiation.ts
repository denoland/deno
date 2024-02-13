// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// This module is browser compatible.

/**
 * Contains the functions {@linkcode accepts}, {@linkcode acceptsEncodings}, and
 * {@linkcode acceptsLanguages} to provide content negotiation capabilities.
 *
 * @module
 */

import { preferredEncodings } from "./_negotiation/encoding.ts";
import { preferredLanguages } from "./_negotiation/language.ts";
import { preferredMediaTypes } from "./_negotiation/media_type.ts";

export type Request = {
  headers: {
    get(key: string): string | null;
  };
};

/**
 * Returns an array of media types accepted by the request, in order of
 * preference. If there are no media types supplied in the request, then any
 * media type selector will be returned.
 *
 * @example
 * ```ts
 * import { accepts } from "https://deno.land/std@$STD_VERSION/http/negotiation.ts";
 *
 * const req = new Request("https://example.com/", {
 *   headers: {
 *     "accept":
 *       "text/html, application/xhtml+xml, application/xml;q=0.9, image/webp, *\/*;q=0.8",
 *   },
 * });
 *
 * console.log(accepts(req));
 * // [
 * //   "text/html",
 * //   "application/xhtml+xml",
 * //   "image/webp",
 * //   "application/xml",
 * //   "*\/*",
 * // ]
 * ```
 */
export function accepts(request: Request): string[];
/**
 * For a given set of media types, return the best match accepted in the
 * request. If no media type matches, then the function returns `undefined`.
 *
 *  @example
 * ```ts
 * import { accepts } from "https://deno.land/std@$STD_VERSION/http/negotiation.ts";
 *
 * const req = new Request("https://example.com/", {
 *   headers: {
 *     "accept":
 *       "text/html, application/xhtml+xml, application/xml;q=0.9, image/webp, *\/*;q=0.8",
 *   },
 * });
 *
 * accepts(req, "text/html", "image/webp"); // "text/html";
 * ```
 */
export function accepts(
  request: Request,
  ...types: string[]
): string | undefined;
export function accepts(
  request: Request,
  ...types: string[]
): string | string[] | undefined {
  const accept = request.headers.get("accept");
  return types.length
    ? accept ? preferredMediaTypes(accept, types)[0] : types[0]
    : accept
    ? preferredMediaTypes(accept)
    : ["*/*"];
}

/**
 * Returns an array of content encodings accepted by the request, in order of
 * preference. If there are no encoding supplied in the request, then `["*"]`
 * is returned, implying any encoding is accepted.
 *
 * @example
 * ```ts
 * import { acceptsEncodings } from "https://deno.land/std@$STD_VERSION/http/negotiation.ts";
 *
 * const req = new Request("https://example.com/", {
 *   headers: { "accept-encoding": "deflate, gzip;q=1.0, *;q=0.5" },
 * });
 *
 * acceptsEncodings(req); // ["deflate", "gzip", "*"]
 * ```
 */
export function acceptsEncodings(request: Request): string[];
/**
 * For a given set of content encodings, return the best match accepted in the
 * request. If no content encodings match, then the function returns
 * `undefined`.
 *
 * **NOTE:** You should always supply `identity` as one of the encodings
 * to ensure that there is a match when the `Accept-Encoding` header is part
 * of the request.
 *
 * @example
 * ```ts
 * import { acceptsEncodings } from "https://deno.land/std@$STD_VERSION/http/negotiation.ts";
 *
 * const req = new Request("https://example.com/", {
 *   headers: { "accept-encoding": "deflate, gzip;q=1.0, *;q=0.5" },
 * });
 *
 * acceptsEncodings(req, "gzip", "identity"); // "gzip"
 * ```
 */
export function acceptsEncodings(
  request: Request,
  ...encodings: string[]
): string | undefined;
export function acceptsEncodings(
  request: Request,
  ...encodings: string[]
): string | string[] | undefined {
  const acceptEncoding = request.headers.get("accept-encoding");
  return encodings.length
    ? acceptEncoding
      ? preferredEncodings(acceptEncoding, encodings)[0]
      : encodings[0]
    : acceptEncoding
    ? preferredEncodings(acceptEncoding)
    : ["*"];
}

/**
 * Returns an array of languages accepted by the request, in order of
 * preference. If there are no languages supplied in the request, then `["*"]`
 * is returned, imply any language is accepted.
 *
 * @example
 * ```ts
 * import { acceptsLanguages } from "https://deno.land/std@$STD_VERSION/http/negotiation.ts";
 *
 * const req = new Request("https://example.com/", {
 *   headers: {
 *     "accept-language": "fr-CH, fr;q=0.9, en;q=0.8, de;q=0.7, *;q=0.5",
 *   },
 * });
 *
 * acceptsLanguages(req); // ["fr-CH", "fr", "en", "de", "*"]
 * ```
 */
export function acceptsLanguages(request: Request): string[];
/**
 * For a given set of languages, return the best match accepted in the request.
 * If no languages match, then the function returns `undefined`.
 *
 * @example
 * ```ts
 * import { acceptsLanguages } from "https://deno.land/std@$STD_VERSION/http/negotiation.ts";
 *
 * const req = new Request("https://example.com/", {
 *   headers: {
 *     "accept-language": "fr-CH, fr;q=0.9, en;q=0.8, de;q=0.7, *;q=0.5",
 *   },
 * });
 *
 * acceptsLanguages(req, "en-gb", "en-us", "en"); // "en"
 * ```
 */
export function acceptsLanguages(
  request: Request,
  ...langs: string[]
): string | undefined;
export function acceptsLanguages(
  request: Request,
  ...langs: string[]
): string | string[] | undefined {
  const acceptLanguage = request.headers.get("accept-language");
  return langs.length
    ? acceptLanguage ? preferredLanguages(acceptLanguage, langs)[0] : langs[0]
    : acceptLanguage
    ? preferredLanguages(acceptLanguage)
    : ["*"];
}
