// Copyright 2018-2025 the Deno authors. MIT license.

import { core } from "ext:core/mod.js";

const lazyLoad = core.createLazyLoader("ext:deno_geometry/01_geometry.js");

let geometry;

/**
 * @param {(transformList: string, prefix: string) => { matrix: Float64Array, is2D: boolean }} transformListParser
 * @param {boolean} enableWindowFeatures
 */
export function createGeometryLoader(
  transformListParser,
  enableWindowFeatures,
) {
  return () => {
    if (geometry !== undefined) {
      return geometry;
    }

    geometry = lazyLoad();
    geometry.init(transformListParser, enableWindowFeatures);

    return geometry;
  };
}
