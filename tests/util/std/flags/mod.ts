// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// This module is browser compatible.

/**
 * @deprecated (will be removed in 1.0.0) Import from `std/cli/parse_args.ts` instead
 *
 * Command line arguments parser based on
 * [minimist](https://github.com/minimistjs/minimist).
 *
 * This module is browser compatible.
 *
 * @example
 * ```ts
 * import { parse } from "https://deno.land/std@$STD_VERSION/flags/mod.ts";
 *
 * console.dir(parse(Deno.args));
 * ```
 *
 * @module
 */

export {
  /**
   * @deprecated (will be removed in 1.0.0) Import from `std/cli/parse_args.ts` instead
   * The value returned from `parse`.
   */
  type Args,
  /**
   * @deprecated (will be removed in 1.0.0) Import from `std/cli/parse_args.ts` instead
   *
   * Take a set of command line arguments, optionally with a set of options, and
   * return an object representing the flags found in the passed arguments.
   *
   * By default, any arguments starting with `-` or `--` are considered boolean
   * flags. If the argument name is followed by an equal sign (`=`) it is
   * considered a key-value pair. Any arguments which could not be parsed are
   * available in the `_` property of the returned object.
   *
   * By default, the flags module tries to determine the type of all arguments
   * automatically and the return type of the `parse` method will have an index
   * signature with `any` as value (`{ [x: string]: any }`).
   *
   * If the `string`, `boolean` or `collect` option is set, the return value of
   * the `parse` method will be fully typed and the index signature of the return
   * type will change to `{ [x: string]: unknown }`.
   *
   * Any arguments after `'--'` will not be parsed and will end up in `parsedArgs._`.
   *
   * Numeric-looking arguments will be returned as numbers unless `options.string`
   * or `options.boolean` is set for that argument name.
   *
   * @example
   * ```ts
   * import { parse } from "https://deno.land/std@$STD_VERSION/flags/mod.ts";
   * const parsedArgs = parse(Deno.args);
   * ```
   *
   * @example
   * ```ts
   * import { parse } from "https://deno.land/std@$STD_VERSION/flags/mod.ts";
   * const parsedArgs = parse(["--foo", "--bar=baz", "./quux.txt"]);
   * // parsedArgs: { foo: true, bar: "baz", _: ["./quux.txt"] }
   * ```
   */
  parseArgs as parse,
  /**
   * @deprecated (will be removed in 1.0.0) Import from `std/cli/parse_args.ts` instead
   *
   * The options for the `parse` call.
   */
  type ParseOptions,
} from "../cli/parse_args.ts";
