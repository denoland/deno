// Copyright 2018 the Deno authors. All rights reserved. MIT license.

/** If you use the eval function indirectly, by invoking it via a reference
 * other than eval, as of ECMAScript 5 it works in the global scope rather than
 * the local scope. This means, for instance, that function declarations create
 * global functions, and that the code being evaluated doesn't have access to
 * local variables within the scope where it's being called.
 *
 * @internal
 */
export const globalEval = eval;
