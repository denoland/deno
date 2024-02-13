// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// This module is browser compatible.

/** Utility functions for media types (MIME types).
 *
 * This API is inspired by the GoLang [`mime`](https://pkg.go.dev/mime) package
 * and [jshttp/mime-types](https://github.com/jshttp/mime-types), and is
 * designed to integrate and improve the APIs from
 * [deno.land/x/media_types](https://deno.land/x/media_types).
 *
 * The `vendor` folder contains copy of the
 * [jshttp/mime-db](https://github.com/jshttp/mime-types) `db.json` file along
 * with its license.
 *
 * @module
 */

export * from "./content_type.ts";
export * from "./extension.ts";
export * from "./extensions_by_type.ts";
export * from "./format_media_type.ts";
export * from "./get_charset.ts";
export * from "./parse_media_type.ts";
export * from "./type_by_extension.ts";
