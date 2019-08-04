// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { window } from "./window";

// This allows us to access core in API even if we
// dispose window.Deno
export const core = window.Deno.core as DenoCore;
