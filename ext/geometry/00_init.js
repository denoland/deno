// Copyright 2018-2025 the Deno authors. MIT license.

import { core } from "ext:core/mod.js";

const loadGeometry = core.createLazyLoader("ext:deno_geometry/01_geometry.js");

export { loadGeometry };
