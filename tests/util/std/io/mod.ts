// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

/**
 * Utilities for working with Deno's readers, writers, and web streams.
 *
 * `Reader` and `Writer` interfaces are deprecated in Deno, and so many of these
 * utilities are also deprecated. Consider using web streams instead.
 *
 * @module
 * @deprecated (will be removed after 1.0.0) Use the [Web Streams API]{@link https://developer.mozilla.org/en-US/docs/Web/API/Streams_API} instead.
 */

export * from "./buf_reader.ts";
export * from "./buf_writer.ts";
export * from "./buffer.ts";
export * from "./copy_n.ts";
export * from "./limited_reader.ts";
export * from "./multi_reader.ts";
export * from "./read_delim.ts";
export * from "./read_int.ts";
export * from "./read_lines.ts";
export * from "./read_long.ts";
export * from "./read_range.ts";
export * from "./read_short.ts";
export * from "./read_string_delim.ts";
export * from "./slice_long_to_bytes.ts";
export * from "./string_reader.ts";
export * from "./string_writer.ts";
