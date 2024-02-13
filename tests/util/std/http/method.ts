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

export {
  /**
   * @deprecated (will be removed in 0.210.0)
   *
   * A constant array of common HTTP methods.
   *
   * This list is compatible with Node.js `http` module.
   */
  HTTP_METHODS,
  /**
   * @deprecated (will be removed in 0.210.0)
   *
   * A type representing string literals of each of the common HTTP method.
   */
  type HttpMethod,
  /**
   * @deprecated (will be removed in 0.210.0)
   *
   * A type guard that determines if a value is a valid HTTP method.
   */
  isHttpMethod,
} from "./unstable_method.ts";
