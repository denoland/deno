// Copyright 2018-2026 the Deno authors. MIT license.

// During this module's evaluation, lazy-load the sibling module B. When
// `main.js` statically imports both A and B, V8 instantiates both during
// the link phase and evaluates them in DFS post-order, so B is registered
// in the module map (instantiated) but not yet evaluated when A evaluates.
const lazyB = Deno.core.createLazyLoader("custom:lazy_b");
export const { value } = lazyB();
