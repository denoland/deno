/**
 * Following import uses two distinct ways to provide types:
 * - X-TypeScript-Types headers
 * - @deno-types directive
 * 
 * Because "@deno-types" directive must be placed by user explicitly it
 * should have higher precedence than type header.
 * 
 * This is verified by providing conflicting type declaration
 * depending on a way. There should be no TS error, otherwise
 * it means that wrong type declarations are used (from X-TypeScript-Types)
 * header.
 */

// @deno-types="http://127.0.0.1:4545/type_headers_deno_types.foo.d.ts"
import { foo } from "http://127.0.0.1:4545/type_headers_deno_types.foo.js";

foo("hello");
