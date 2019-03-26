// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { window } from "./window";

export const core = window.Deno.core as DenoCore;
