# deno_geometry

This crate implements the Geometry Interfaces Module API.

Spec: https://drafts.fxtf.org/geometry/

## Usage Example

From javascript, include the extension's source:

```javascript
import { core } from "ext:core/mod.js";
import { loadGeometry } from "ext:deno_geometry/00_init.js";
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

Then from rust, provide: `deno_geometry::deno_geometry::init(bool)` in the
`extensions` field of your `RuntimeOptions`

Where `bool` indicates whether window features are enabled at initialization.

## Dependencies

- **deno_webidl**: Provided by the `deno_webidl` crate
- **deno_web**: Provided by the `deno_web` crate
- **deno_console**: Provided by the `deno_console` crate

## Provided ops

Following ops are provided, which can be accessed through `Deno.ops`:

- DOMPointReadOnly
- DOMPoint
- DOMRectReadOnly
- DOMRect
- DOMQuad
- DOMMatrixReadOnly
- DOMMatrix
- op_geometry_get_enable_window_features
- op_geometry_matrix_set_matrix_value
- op_geometry_matrix_to_buffer
- op_geometry_matrix_to_string
