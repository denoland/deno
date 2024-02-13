// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// This module is browser compatible.

/**
 * {@linkcode parse} and {@linkcode stringify} for handling
 * [TOML](https://toml.io/en/latest) encoded data. Be sure to read the supported
 * types as not every spec is supported at the moment and the handling in
 * TypeScript side is a bit different.
 *
 * ## Supported types and handling
 *
 * - :heavy_check_mark: [Keys](https://toml.io/en/latest#keys)
 * - :exclamation: [String](https://toml.io/en/latest#string)
 * - :heavy_check_mark: [Multiline String](https://toml.io/en/latest#string)
 * - :heavy_check_mark: [Literal String](https://toml.io/en/latest#string)
 * - :exclamation: [Integer](https://toml.io/en/latest#integer)
 * - :heavy_check_mark: [Float](https://toml.io/en/latest#float)
 * - :heavy_check_mark: [Boolean](https://toml.io/en/latest#boolean)
 * - :heavy_check_mark:
 *   [Offset Date-time](https://toml.io/en/latest#offset-date-time)
 * - :heavy_check_mark:
 *   [Local Date-time](https://toml.io/en/latest#local-date-time)
 * - :heavy_check_mark: [Local Date](https://toml.io/en/latest#local-date)
 * - :exclamation: [Local Time](https://toml.io/en/latest#local-time)
 * - :heavy_check_mark: [Table](https://toml.io/en/latest#table)
 * - :heavy_check_mark: [Inline Table](https://toml.io/en/latest#inline-table)
 * - :exclamation: [Array of Tables](https://toml.io/en/latest#array-of-tables)
 *
 * :exclamation: _Supported with warnings see [Warning](#Warning)._
 *
 * ### :warning: Warning
 *
 * #### String
 *
 * - Regex : Due to the spec, there is no flag to detect regex properly in a TOML
 *   declaration. So the regex is stored as string.
 *
 * #### Integer
 *
 * For **Binary** / **Octal** / **Hexadecimal** numbers, they are stored as string
 * to be not interpreted as Decimal.
 *
 * #### Local Time
 *
 * Because local time does not exist in JavaScript, the local time is stored as a
 * string.
 *
 * #### Inline Table
 *
 * Inline tables are supported. See below:
 *
 * ```toml
 * animal = { type = { name = "pug" } }
 * ## Output { animal: { type: { name: "pug" } } }
 * animal = { type.name = "pug" }
 * ## Output { animal: { type : { name : "pug" } }
 * animal.as.leaders = "tosin"
 * ## Output { animal: { as: { leaders: "tosin" } } }
 * "tosin.abasi" = "guitarist"
 * ## Output { tosin.abasi: "guitarist" }
 * ```
 *
 * #### Array of Tables
 *
 * At the moment only simple declarations like below are supported:
 *
 * ```toml
 * [[bin]]
 * name = "deno"
 * path = "cli/main.rs"
 *
 * [[bin]]
 * name = "deno_core"
 * path = "src/foo.rs"
 *
 * [[nib]]
 * name = "node"
 * path = "not_found"
 * ```
 *
 * will output:
 *
 * ```json
 * {
 *   "bin": [
 *     { "name": "deno", "path": "cli/main.rs" },
 *     { "name": "deno_core", "path": "src/foo.rs" }
 *   ],
 *   "nib": [{ "name": "node", "path": "not_found" }]
 * }
 * ```
 *
 * This module is browser compatible.
 *
 * @example
 * ```ts
 * import {
 *   parse,
 *   stringify,
 * } from "https://deno.land/std@$STD_VERSION/toml/mod.ts";
 * const obj = {
 *   bin: [
 *     { name: "deno", path: "cli/main.rs" },
 *     { name: "deno_core", path: "src/foo.rs" },
 *   ],
 *   nib: [{ name: "node", path: "not_found" }],
 * };
 * const tomlString = stringify(obj);
 * console.log(tomlString);
 *
 * // =>
 * // [[bin]]
 * // name = "deno"
 * // path = "cli/main.rs"
 *
 * // [[bin]]
 * // name = "deno_core"
 * // path = "src/foo.rs"
 *
 * // [[nib]]
 * // name = "node"
 * // path = "not_found"
 *
 * const tomlObject = parse(tomlString);
 * console.log(tomlObject);
 *
 * // =>
 * // {
 * //   bin: [
 * //     { name: "deno", path: "cli/main.rs" },
 * //     { name: "deno_core", path: "src/foo.rs" }
 * //   ],
 * //   nib: [ { name: "node", path: "not_found" } ]
 * // }
 * ```
 *
 * @module
 */

export * from "./stringify.ts";
export * from "./parse.ts";
