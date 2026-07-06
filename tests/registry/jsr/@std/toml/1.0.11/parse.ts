// Copyright 2018-2025 the Deno authors. MIT license.
// This module is browser compatible.

import { parserFactory, toml } from "./_parser.ts";

/**
 * Parses a {@link https://toml.io | TOML} string into an object.
 *
 * @example Usage
 * ```ts
 * import { parse } from "@std/toml/parse";
 * import { assertEquals } from "@std/assert";
 *
 * const tomlString = `title = "TOML Example"
 * [owner]
 * name = "Alice"
 * bio = "Alice is a programmer."`;
 *
 * const obj = parse(tomlString);
 * assertEquals(obj, { title: "TOML Example", owner: { name: "Alice", bio: "Alice is a programmer." } });
 * ```
 * @param tomlString TOML string to be parsed.
 * @returns The parsed JS object.
 */
export function parse(tomlString: string): Record<string, unknown> {
  return parserFactory(toml)(tomlString);
}
