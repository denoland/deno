// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

/*
 * This library contains DOM standards that are not currently included in the
 * distributed `lib.dom.d.ts` file with TypeScript.
 */

/// <reference no-default-lib="true"/>

declare interface URLPatternInit {
  protocol?: string;
  username?: string;
  password?: string;
  hostname?: string;
  port?: string;
  pathname?: string;
  search?: string;
  hash?: string;
  baseURL?: string;
}

declare type URLPatternInput = string | URLPatternInit;

declare interface URLPatternComponentResult {
  input: string;
  groups: Record<string, string | undefined>;
}

/** `URLPatternResult` is the object returned from `URLPattern.exec`. */
declare interface URLPatternResult {
  /** The inputs provided when matching. */
  inputs: [URLPatternInit] | [URLPatternInit, string];

  /** The matched result for the `protocol` matcher. */
  protocol: URLPatternComponentResult;
  /** The matched result for the `username` matcher. */
  username: URLPatternComponentResult;
  /** The matched result for the `password` matcher. */
  password: URLPatternComponentResult;
  /** The matched result for the `hostname` matcher. */
  hostname: URLPatternComponentResult;
  /** The matched result for the `port` matcher. */
  port: URLPatternComponentResult;
  /** The matched result for the `pathname` matcher. */
  pathname: URLPatternComponentResult;
  /** The matched result for the `search` matcher. */
  search: URLPatternComponentResult;
  /** The matched result for the `hash` matcher. */
  hash: URLPatternComponentResult;
}

/**
 * The URLPattern API provides a web platform primitive for matching URLs based
 * on a convenient pattern syntax.
 *
 * The syntax is based on path-to-regexp. Wildcards, named capture groups,
 * regular groups, and group modifiers are all supported.
 *
 * ```ts
 * // Specify the pattern as structured data.
 * const pattern = new URLPattern({ pathname: "/users/:user" });
 * const match = pattern.exec("https://blog.example.com/users/joe");
 * console.log(match.pathname.groups.user); // joe
 * ```
 *
 * ```ts
 * // Specify a fully qualified string pattern.
 * const pattern = new URLPattern("https://example.com/books/:id");
 * console.log(pattern.test("https://example.com/books/123")); // true
 * console.log(pattern.test("https://deno.land/books/123")); // false
 * ```
 *
 * ```ts
 * // Specify a relative string pattern with a base URL.
 * const pattern = new URLPattern("/article/:id", "https://blog.example.com");
 * console.log(pattern.test("https://blog.example.com/article")); // false
 * console.log(pattern.test("https://blog.example.com/article/123")); // true
 * ```
 */
interface URLPattern {
  /**
   * Test if the given input matches the stored pattern.
   *
   * The input can either be provided as an absolute URL string with an optional base,
   * relative URL string with a required base, or as individual components
   * in the form of an `URLPatternInit` object.
   *
   * ```ts
   * const pattern = new URLPattern("https://example.com/books/:id");
   *
   * // Test an absolute url string.
   * console.log(pattern.test("https://example.com/books/123")); // true
   *
   * // Test a relative url with a base.
   * console.log(pattern.test("/books/123", "https://example.com")); // true
   *
   * // Test an object of url components.
   * console.log(pattern.test({ pathname: "/books/123" })); // true
   * ```
   */
  test(input: URLPatternInput, baseURL?: string): boolean;

  /**
   * Match the given input against the stored pattern.
   *
   * The input can either be provided as an absolute URL string with an optional base,
   * relative URL string with a required base, or as individual components
   * in the form of an `URLPatternInit` object.
   *
   * ```ts
   * const pattern = new URLPattern("https://example.com/books/:id");
   *
   * // Match an absolute url string.
   * let match = pattern.exec("https://example.com/books/123");
   * console.log(match.pathname.groups.id); // 123
   *
   * // Match a relative url with a base.
   * match = pattern.exec("/books/123", "https://example.com");
   * console.log(match.pathname.groups.id); // 123
   *
   * // Match an object of url components.
   * match = pattern.exec({ pathname: "/books/123" });
   * console.log(match.pathname.groups.id); // 123
   * ```
   */
  exec(input: URLPatternInput, baseURL?: string): URLPatternResult | null;

  /** The pattern string for the `protocol`. */
  readonly protocol: string;
  /** The pattern string for the `username`. */
  readonly username: string;
  /** The pattern string for the `password`. */
  readonly password: string;
  /** The pattern string for the `hostname`. */
  readonly hostname: string;
  /** The pattern string for the `port`. */
  readonly port: string;
  /** The pattern string for the `pathname`. */
  readonly pathname: string;
  /** The pattern string for the `search`. */
  readonly search: string;
  /** The pattern string for the `hash`. */
  readonly hash: string;
}

/**
 * The URLPattern API provides a web platform primitive for matching URLs based
 * on a convenient pattern syntax.
 *
 * The syntax is based on path-to-regexp. Wildcards, named capture groups,
 * regular groups, and group modifiers are all supported.
 *
 * ```ts
 * // Specify the pattern as structured data.
 * const pattern = new URLPattern({ pathname: "/users/:user" });
 * const match = pattern.exec("https://blog.example.com/users/joe");
 * console.log(match.pathname.groups.user); // joe
 * ```
 *
 * ```ts
 * // Specify a fully qualified string pattern.
 * const pattern = new URLPattern("https://example.com/books/:id");
 * console.log(pattern.test("https://example.com/books/123")); // true
 * console.log(pattern.test("https://deno.land/books/123")); // false
 * ```
 *
 * ```ts
 * // Specify a relative string pattern with a base URL.
 * const pattern = new URLPattern("/article/:id", "https://blog.example.com");
 * console.log(pattern.test("https://blog.example.com/article")); // false
 * console.log(pattern.test("https://blog.example.com/article/123")); // true
 * ```
 */
declare var URLPattern: {
  readonly prototype: URLPattern;
  new (input: URLPatternInput, baseURL?: string): URLPattern;
};

interface ErrorConstructor {
  /** See https://v8.dev/docs/stack-trace-api#stack-trace-collection-for-custom-exceptions. */
  captureStackTrace(error: Object, constructor?: Function): void;
}
