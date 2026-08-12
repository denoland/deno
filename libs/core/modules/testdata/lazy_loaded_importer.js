// Copyright 2018-2026 the Deno authors. MIT license.
import { value } from "custom:aliased";
if (value !== "aliased") throw new Error("expected 'aliased', got " + value);
Deno.core.print("lazy_loaded_esm import works\n");
