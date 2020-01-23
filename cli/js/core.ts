// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

// This allows us to access core in API even if we
// dispose window.Deno
export const core = globalThis.Deno.core as DenoCore;
