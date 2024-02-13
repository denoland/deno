// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// This module is browser compatible.

import { ParserFactory, Toml } from "./_parser.ts";

/**
 * Parse parses TOML string into an object.
 * @param tomlString
 */
export const parse = ParserFactory(Toml);
