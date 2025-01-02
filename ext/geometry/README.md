# deno_geometry

This crate implements the Geometry Interfaces Module API.

Spec: https://drafts.fxtf.org/geometry/

## Usage Example

From javascript, include the extension's source:

```javascript
import { core } from "ext:core/mod.js";
import { createGeometryLoader } from "ext:deno_geometry/00_init.js";
```

For environments that do not have a CSS `<transform-list>` parser, such as Web
Worker, configure as follows:

```javascript
const loadGeometry = createGeometryLoader((_transformList, prefix) => {
  throw new TypeError(
    `${prefix}: Cannot parse CSS <transform-list> on Workers`,
  );
}, /* enableWindowFeatures */ false);
```

On the other hand, in environments with a CSS `<transform-list>` parser, you can
configure as follows:

```javascript
const loadGeometry = createGeometryLoader((transformList, prefix) => {
  try {
    // parse <transform-list> by yourself
    const { sequence, is2D } = parse(transformList);
    return {
      matrix: new Float64Array(sequence),
      is2D,
    };
  } catch {
    throw new TypeError(
      `${prefix}: Invalid <transform-list> string: ${transformList}`,
    );
  }
}, /* enableWindowFeatures */ true);
```

Then define to globalThis:

```javascript
Object.defineProperties(globalThis, {
  DOMMatrix: core.propNonEnumerableLazyLoaded(
    (geometry) => geometry.DOMMatrix,
    loadGeometry,
  ),
  DOMMatrixReadOnly: core.propNonEnumerableLazyLoaded(
    (geometry) => geometry.DOMMatrixReadOnly,
    loadGeometry,
  ),
  DOMPoint: core.propNonEnumerableLazyLoaded(
    (geometry) => geometry.DOMPoint,
    loadGeometry,
  ),
  DOMPointReadOnly: core.propNonEnumerableLazyLoaded(
    (geometry) => geometry.DOMPointReadOnly,
    loadGeometry,
  ),
  DOMQuad: core.propNonEnumerableLazyLoaded(
    (geometry) => geometry.DOMQuad,
    loadGeometry,
  ),
  DOMRect: core.propNonEnumerableLazyLoaded(
    (geometry) => geometry.DOMRect,
    loadGeometry,
  ),
  DOMRectReadOnly: core.propNonEnumerableLazyLoaded(
    (geometry) => geometry.DOMRectReadOnly,
    loadGeometry,
  ),
});
```

Then from rust, provide: `deno_geometry::deno_geometry::init_ops_and_esm()` in
the `extensions` field of your `RuntimeOptions`

## Dependencies

- **deno_webidl**: Provided by the `deno_webidl` crate
- **deno_web**: Provided by the `deno_web` crate
- **deno_console**: Provided by the `deno_console` crate

## Provided ops

Following ops are provided, which can be accessed through `Deno.ops`:

- op_create_matrix_identity
- Matrix
- Point
- Rect
