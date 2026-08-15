// Copyright 2018-2026 the Deno authors. MIT license.

/*
 * This library contains DOM standards that are not currently included in the
 * distributed `lib.dom.d.ts` file with TypeScript.
 */

/// <reference no-default-lib="true"/>

interface ErrorConstructor {
  /** See https://v8.dev/docs/stack-trace-api#stack-trace-collection-for-custom-exceptions. */
  captureStackTrace(
    error: object,
    constructor?: (...args: never[]) => unknown,
  ): void;
}
