// Copyright 2018-2025 the Deno authors. MIT license.

// deno-lint-ignore-file no-var

/// <reference no-default-lib="true" />
/// <reference lib="esnext" />

/**
 * @category Geometry Interfaces Module API
 * @experimental
 */
interface DOMMatrix2DInit {
  a?: number;
  b?: number;
  c?: number;
  d?: number;
  e?: number;
  f?: number;
  m11?: number;
  m12?: number;
  m21?: number;
  m22?: number;
  m41?: number;
  m42?: number;
}

/**
 * @category Geometry Interfaces Module API
 * @experimental
 */
interface DOMMatrixInit extends DOMMatrix2DInit {
  is2D?: boolean;
  m13?: number;
  m14?: number;
  m23?: number;
  m24?: number;
  m31?: number;
  m32?: number;
  m33?: number;
  m34?: number;
  m43?: number;
  m44?: number;
}

/**
 * The **`DOMMatrix`** interface represents 4×4 matrices, suitable for 2D and 3D operations including rotation and translation.
 *
 * [MDN](https://developer.mozilla.org/docs/Web/API/DOMMatrix)
 *
 * ```
 * | m11 m21 m31 m41 |
 * | m12 m22 m32 m42 |
 * | m13 m23 m33 m43 |
 * | m14 m24 m34 m44 |
 * ```
 *
 * @category Geometry Interfaces Module API
 * @experimental
 */
interface DOMMatrix extends DOMMatrixReadOnly {
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/DOMMatrix#instance_properties) */
  a: number;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/DOMMatrix#instance_properties) */
  b: number;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/DOMMatrix#instance_properties) */
  c: number;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/DOMMatrix#instance_properties) */
  d: number;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/DOMMatrix#instance_properties) */
  e: number;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/DOMMatrix#instance_properties) */
  f: number;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/DOMMatrix#instance_properties) */
  m11: number;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/DOMMatrix#instance_properties) */
  m12: number;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/DOMMatrix#instance_properties) */
  m13: number;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/DOMMatrix#instance_properties) */
  m14: number;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/DOMMatrix#instance_properties) */
  m21: number;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/DOMMatrix#instance_properties) */
  m22: number;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/DOMMatrix#instance_properties) */
  m23: number;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/DOMMatrix#instance_properties) */
  m24: number;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/DOMMatrix#instance_properties) */
  m31: number;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/DOMMatrix#instance_properties) */
  m32: number;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/DOMMatrix#instance_properties) */
  m33: number;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/DOMMatrix#instance_properties) */
  m34: number;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/DOMMatrix#instance_properties) */
  m41: number;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/DOMMatrix#instance_properties) */
  m42: number;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/DOMMatrix#instance_properties) */
  m43: number;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/DOMMatrix#instance_properties) */
  m44: number;
  /**
   * The **`invertSelf()`** method of the DOMMatrix interface inverts the original matrix.
   *
   * [MDN Reference](https://developer.mozilla.org/docs/Web/API/DOMMatrix/invertSelf)
   */
  invertSelf(): DOMMatrix;
  /**
   * The **`multiplySelf()`** method of the DOMMatrix interface multiplies a matrix by the `otherMatrix` parameter, computing the dot product of the original matrix and the specified matrix: `A⋅B`.
   *
   * [MDN Reference](https://developer.mozilla.org/docs/Web/API/DOMMatrix/multiplySelf)
   */
  multiplySelf(other?: DOMMatrixInit): DOMMatrix;
  /**
   * The **`preMultiplySelf()`** method of the DOMMatrix interface modifies the matrix by pre-multiplying it with the specified `DOMMatrix`.
   *
   * [MDN Reference](https://developer.mozilla.org/docs/Web/API/DOMMatrix/preMultiplySelf)
   */
  preMultiplySelf(other?: DOMMatrixInit): DOMMatrix;
  /**
   * The `rotateAxisAngleSelf()` method of the DOMMatrix interface is a transformation method that rotates the source matrix by the given vector and angle, returning the altered matrix.
   *
   * [MDN Reference](https://developer.mozilla.org/docs/Web/API/DOMMatrix/rotateAxisAngleSelf)
   */
  rotateAxisAngleSelf(
    x?: number,
    y?: number,
    z?: number,
    angle?: number,
  ): DOMMatrix;
  /**
   * The `rotateFromVectorSelf()` method of the DOMMatrix interface is a mutable transformation method that modifies a matrix by rotating the matrix by the angle between the specified vector and `(1, 0)`.
   *
   * [MDN Reference](https://developer.mozilla.org/docs/Web/API/DOMMatrix/rotateFromVectorSelf)
   */
  rotateFromVectorSelf(x?: number, y?: number): DOMMatrix;
  /**
   * The `rotateSelf()` method of the DOMMatrix interface is a mutable transformation method that modifies a matrix.
   *
   * [MDN Reference](https://developer.mozilla.org/docs/Web/API/DOMMatrix/rotateSelf)
   */
  rotateSelf(rotX?: number, rotY?: number, rotZ?: number): DOMMatrix;
  /**
   * The **`scale3dSelf()`** method of the DOMMatrix interface is a mutable transformation method that modifies a matrix by applying a specified scaling factor to all three axes, centered on the given origin, with a default origin of `(0, 0, 0)`, returning the 3D-scaled matrix.
   *
   * [MDN Reference](https://developer.mozilla.org/docs/Web/API/DOMMatrix/scale3dSelf)
   */
  scale3dSelf(
    scale?: number,
    originX?: number,
    originY?: number,
    originZ?: number,
  ): DOMMatrix;
  /**
   * The **`scaleSelf()`** method of the DOMMatrix interface is a mutable transformation method that modifies a matrix by applying a specified scaling factor, centered on the given origin, with a default origin of `(0, 0)`, returning the scaled matrix.
   *
   * [MDN Reference](https://developer.mozilla.org/docs/Web/API/DOMMatrix/scaleSelf)
   */
  scaleSelf(
    scaleX?: number,
    scaleY?: number,
    scaleZ?: number,
    originX?: number,
    originY?: number,
    originZ?: number,
  ): DOMMatrix;
  /**
   * The **`setMatrixValue()`** method of the DOMMatrix interface replaces the contents of the matrix with the matrix described by the specified transform or transforms, returning itself.
   *
   * [MDN Reference](https://developer.mozilla.org/docs/Web/API/DOMMatrix/setMatrixValue)
   */
  setMatrixValue(transformList: string): DOMMatrix;
  /**
   * The `skewXSelf()` method of the DOMMatrix interface is a mutable transformation method that modifies a matrix.
   *
   * [MDN Reference](https://developer.mozilla.org/docs/Web/API/DOMMatrix/skewXSelf)
   */
  skewXSelf(sx?: number): DOMMatrix;
  /**
   * The `skewYSelf()` method of the DOMMatrix interface is a mutable transformation method that modifies a matrix.
   *
   * [MDN Reference](https://developer.mozilla.org/docs/Web/API/DOMMatrix/skewYSelf)
   */
  skewYSelf(sy?: number): DOMMatrix;
  /**
   * The `translateSelf()` method of the DOMMatrix interface is a mutable transformation method that modifies a matrix.
   *
   * [MDN Reference](https://developer.mozilla.org/docs/Web/API/DOMMatrix/translateSelf)
   */
  translateSelf(tx?: number, ty?: number, tz?: number): DOMMatrix;
}

/**
 * A 4×4 matrix (column-major order), suitable for 2D and 3D operations including rotation and translation.
 * [MDN](https://developer.mozilla.org/docs/Web/API/DOMMatrix)
 *
 * ```
 * | m11 m21 m31 m41 |
 * | m12 m22 m32 m42 |
 * | m13 m23 m33 m43 |
 * | m14 m24 m34 m44 |
 * ```
 *
 * @category Geometry Interfaces Module API
 * @experimental
 */
declare var DOMMatrix: {
  prototype: DOMMatrix;
  new (init?: string | number[]): DOMMatrix;
  fromFloat32Array(array32: Float32Array<ArrayBuffer>): DOMMatrix;
  fromFloat64Array(array64: Float64Array<ArrayBuffer>): DOMMatrix;
  fromMatrix(other?: DOMMatrixInit): DOMMatrix;
};

/**
 * The **`DOMMatrixReadOnly`** interface represents a read-only 4×4 matrix, suitable for 2D and 3D operations.
 *
 * [MDN](https://developer.mozilla.org/docs/Web/API/DOMMatrixReadOnly)
 *
 * ```
 * | m11 m21 m31 m41 |
 * | m12 m22 m32 m42 |
 * | m13 m23 m33 m43 |
 * | m14 m24 m34 m44 |
 * ```
 *
 * @category Geometry Interfaces Module API
 * @experimental
 */
interface DOMMatrixReadOnly {
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/DOMMatrixReadOnly#instance_properties) */
  readonly a: number;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/DOMMatrixReadOnly#instance_properties) */
  readonly b: number;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/DOMMatrixReadOnly#instance_properties) */
  readonly c: number;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/DOMMatrixReadOnly#instance_properties) */
  readonly d: number;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/DOMMatrixReadOnly#instance_properties) */
  readonly e: number;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/DOMMatrixReadOnly#instance_properties) */
  readonly f: number;
  /**
   * The readonly **`is2D`** property of the DOMMatrixReadOnly interface is a Boolean flag that is `true` when the matrix is 2D.
   *
   * [MDN Reference](https://developer.mozilla.org/docs/Web/API/DOMMatrixReadOnly/is2D)
   */
  readonly is2D: boolean;
  /**
   * The readonly **`isIdentity`** property of the DOMMatrixReadOnly interface is a Boolean whose value is `true` if the matrix is the identity matrix.
   *
   * [MDN Reference](https://developer.mozilla.org/docs/Web/API/DOMMatrixReadOnly/isIdentity)
   */
  readonly isIdentity: boolean;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/DOMMatrixReadOnly#instance_properties) */
  readonly m11: number;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/DOMMatrixReadOnly#instance_properties) */
  readonly m12: number;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/DOMMatrixReadOnly#instance_properties) */
  readonly m13: number;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/DOMMatrixReadOnly#instance_properties) */
  readonly m14: number;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/DOMMatrixReadOnly#instance_properties) */
  readonly m21: number;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/DOMMatrixReadOnly#instance_properties) */
  readonly m22: number;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/DOMMatrixReadOnly#instance_properties) */
  readonly m23: number;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/DOMMatrixReadOnly#instance_properties) */
  readonly m24: number;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/DOMMatrixReadOnly#instance_properties) */
  readonly m31: number;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/DOMMatrixReadOnly#instance_properties) */
  readonly m32: number;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/DOMMatrixReadOnly#instance_properties) */
  readonly m33: number;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/DOMMatrixReadOnly#instance_properties) */
  readonly m34: number;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/DOMMatrixReadOnly#instance_properties) */
  readonly m41: number;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/DOMMatrixReadOnly#instance_properties) */
  readonly m42: number;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/DOMMatrixReadOnly#instance_properties) */
  readonly m43: number;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/DOMMatrixReadOnly#instance_properties) */
  readonly m44: number;
  /**
   * The **`flipX()`** method of the DOMMatrixReadOnly interface creates a new matrix being the result of the original matrix flipped about the x-axis.
   *
   * [MDN Reference](https://developer.mozilla.org/docs/Web/API/DOMMatrixReadOnly/flipX)
   */
  flipX(): DOMMatrix;
  /**
   * The **`flipY()`** method of the DOMMatrixReadOnly interface creates a new matrix being the result of the original matrix flipped about the y-axis.
   *
   * [MDN Reference](https://developer.mozilla.org/docs/Web/API/DOMMatrixReadOnly/flipY)
   */
  flipY(): DOMMatrix;
  /**
   * The **`inverse()`** method of the DOMMatrixReadOnly interface creates a new matrix which is the inverse of the original matrix.
   *
   * [MDN Reference](https://developer.mozilla.org/docs/Web/API/DOMMatrixReadOnly/inverse)
   */
  inverse(): DOMMatrix;
  /**
   * The **`multiply()`** method of the DOMMatrixReadOnly interface creates and returns a new matrix which is the dot product of the matrix and the `otherMatrix` parameter.
   *
   * [MDN Reference](https://developer.mozilla.org/docs/Web/API/DOMMatrixReadOnly/multiply)
   */
  multiply(other?: DOMMatrixInit): DOMMatrix;
  /**
   * The `rotate()` method of the DOMMatrixReadOnly interface returns a new DOMMatrix created by rotating the source matrix around each of its axes by the specified number of degrees.
   *
   * [MDN Reference](https://developer.mozilla.org/docs/Web/API/DOMMatrixReadOnly/rotate)
   */
  rotate(rotX?: number, rotY?: number, rotZ?: number): DOMMatrix;
  /**
   * The `rotateAxisAngle()` method of the DOMMatrixReadOnly interface returns a new DOMMatrix created by rotating the source matrix by the given vector and angle.
   *
   * [MDN Reference](https://developer.mozilla.org/docs/Web/API/DOMMatrixReadOnly/rotateAxisAngle)
   */
  rotateAxisAngle(
    x?: number,
    y?: number,
    z?: number,
    angle?: number,
  ): DOMMatrix;
  /**
   * The `rotateFromVector()` method of the DOMMatrixReadOnly interface is returns a new DOMMatrix created by rotating the source matrix by the angle between the specified vector and `(1, 0)`.
   *
   * [MDN Reference](https://developer.mozilla.org/docs/Web/API/DOMMatrixReadOnly/rotateFromVector)
   */
  rotateFromVector(x?: number, y?: number): DOMMatrix;
  /**
   * The **`scale()`** method of the original matrix with a scale transform applied.
   *
   * [MDN Reference](https://developer.mozilla.org/docs/Web/API/DOMMatrixReadOnly/scale)
   */
  scale(
    scaleX?: number,
    scaleY?: number,
    scaleZ?: number,
    originX?: number,
    originY?: number,
    originZ?: number,
  ): DOMMatrix;
  /**
   * The **`scale3d()`** method of the DOMMatrixReadOnly interface creates a new matrix which is the result of a 3D scale transform being applied to the matrix.
   *
   * [MDN Reference](https://developer.mozilla.org/docs/Web/API/DOMMatrixReadOnly/scale3d)
   */
  scale3d(
    scale?: number,
    originX?: number,
    originY?: number,
    originZ?: number,
  ): DOMMatrix;
  /** @deprecated */
  scaleNonUniform(scaleX?: number, scaleY?: number): DOMMatrix;
  /**
   * The `skewX()` method of the DOMMatrixReadOnly interface returns a new DOMMatrix created by applying the specified skew transformation to the source matrix along its x-axis.
   *
   * [MDN Reference](https://developer.mozilla.org/docs/Web/API/DOMMatrixReadOnly/skewX)
   */
  skewX(sx?: number): DOMMatrix;
  /**
   * The `skewY()` method of the DOMMatrixReadOnly interface returns a new DOMMatrix created by applying the specified skew transformation to the source matrix along its y-axis.
   *
   * [MDN Reference](https://developer.mozilla.org/docs/Web/API/DOMMatrixReadOnly/skewY)
   */
  skewY(sy?: number): DOMMatrix;
  /**
   * The **`toFloat32Array()`** method of the DOMMatrixReadOnly interface returns a new Float32Array containing all 16 elements (`m11`, `m12`, `m13`, `m14`, `m21`, `m22`, `m23`, `m24`, `m31`, `m32`, `m33`, `m34`, `m41`, `m42`, `m43`, `m44`) which comprise the matrix.
   *
   * [MDN Reference](https://developer.mozilla.org/docs/Web/API/DOMMatrixReadOnly/toFloat32Array)
   */
  toFloat32Array(): Float32Array<ArrayBuffer>;
  /**
   * The **`toFloat64Array()`** method of the DOMMatrixReadOnly interface returns a new Float64Array containing all 16 elements (`m11`, `m12`, `m13`, `m14`, `m21`, `m22`, `m23`, `m24`, `m31`, `m32`, `m33`, `m34`, `m41`, `m42`, `m43`, `m44`) which comprise the matrix.
   *
   * [MDN Reference](https://developer.mozilla.org/docs/Web/API/DOMMatrixReadOnly/toFloat64Array)
   */
  toFloat64Array(): Float64Array<ArrayBuffer>;
  /**
   * The **`toJSON()`** method of the DOMMatrixReadOnly interface creates and returns a JSON object.
   *
   * [MDN Reference](https://developer.mozilla.org/docs/Web/API/DOMMatrixReadOnly/toJSON)
   */
  toJSON(): any;
  /**
   * The **`transformPoint`** method of the You can also create a new `DOMPoint` by applying a matrix to a point with the DOMPointReadOnly.matrixTransform() method.
   *
   * [MDN Reference](https://developer.mozilla.org/docs/Web/API/DOMMatrixReadOnly/transformPoint)
   */
  transformPoint(point?: DOMPointInit): DOMPoint;
  /**
   * The `translate()` method of the DOMMatrixReadOnly interface creates a new matrix being the result of the original matrix with a translation applied.
   *
   * [MDN Reference](https://developer.mozilla.org/docs/Web/API/DOMMatrixReadOnly/translate)
   */
  translate(tx?: number, ty?: number, tz?: number): DOMMatrix;
  toString(): string;
}

/**
 * The **`DOMMatrixReadOnly`** interface represents a read-only 4×4 matrix, suitable for 2D and 3D operations.
 *
 * [MDN](https://developer.mozilla.org/docs/Web/API/DOMMatrixReadOnly)
 *
 * ```
 * | m11 m21 m31 m41 |
 * | m12 m22 m32 m42 |
 * | m13 m23 m33 m43 |
 * | m14 m24 m34 m44 |
 * ```
 *
 * @category Geometry Interfaces Module API
 * @experimental
 */
declare var DOMMatrixReadOnly: {
  prototype: DOMMatrixReadOnly;
  new (init?: string | number[]): DOMMatrixReadOnly;
  fromFloat32Array(array32: Float32Array<ArrayBuffer>): DOMMatrixReadOnly;
  fromFloat64Array(array64: Float64Array<ArrayBuffer>): DOMMatrixReadOnly;
  fromMatrix(other?: DOMMatrixInit): DOMMatrixReadOnly;
};

/**
 * @category Geometry Interfaces Module API
 * @experimental
 */
interface DOMPointInit {
  w?: number;
  x?: number;
  y?: number;
  z?: number;
}

/**
 * A **`DOMPoint`** object represents a 2D or 3D point in a coordinate system; it includes values for the coordinates in up to three dimensions, as well as an optional perspective value.
 *
 * [MDN](https://developer.mozilla.org/docs/Web/API/DOMPoint)
 *
 * @category Geometry Interfaces Module API
 * @experimental
 */
interface DOMPoint extends DOMPointReadOnly {
  /**
   * The **`DOMPoint`** interface's **`w`** property holds the point's perspective value, w, for a point in space.
   *
   * [MDN Reference](https://developer.mozilla.org/docs/Web/API/DOMPoint/w)
   */
  w: number;
  /**
   * The **`DOMPoint`** interface's **`x`** property holds the horizontal coordinate, x, for a point in space.
   *
   * [MDN Reference](https://developer.mozilla.org/docs/Web/API/DOMPoint/x)
   */
  x: number;
  /**
   * The **`DOMPoint`** interface's **`y`** property holds the vertical coordinate, _y_, for a point in space.
   *
   * [MDN Reference](https://developer.mozilla.org/docs/Web/API/DOMPoint/y)
   */
  y: number;
  /**
   * The **`DOMPoint`** interface's **`z`** property specifies the depth coordinate of a point in space.
   *
   * [MDN Reference](https://developer.mozilla.org/docs/Web/API/DOMPoint/z)
   */
  z: number;
}

/**
 * A **`DOMPoint`** object represents a 2D or 3D point in a coordinate system; it includes values for the coordinates in up to three dimensions, as well as an optional perspective value.
 *
 * [MDN](https://developer.mozilla.org/docs/Web/API/DOMPoint)
 *
 * @category Geometry Interfaces Module API
 * @experimental
 */
declare var DOMPoint: {
  prototype: DOMPoint;
  new (x?: number, y?: number, z?: number, w?: number): DOMPoint;
  /**
   * The **`fromPoint()`** static method of the DOMPoint interface creates and returns a new mutable `DOMPoint` object given a source point.
   *
   * [MDN Reference](https://developer.mozilla.org/docs/Web/API/DOMPoint/fromPoint_static)
   */
  fromPoint(other?: DOMPointInit): DOMPoint;
};

/**
 * The **`DOMPointReadOnly`** interface specifies the coordinate and perspective fields used by DOMPoint to define a 2D or 3D point in a coordinate system.
 *
 * [MDN](https://developer.mozilla.org/docs/Web/API/DOMPointReadOnly)
 *
 * @category Geometry Interfaces Module API
 * @experimental
 */
interface DOMPointReadOnly {
  /**
   * The **`DOMPointReadOnly`** interface's **`w`** property holds the point's perspective value, `w`, for a read-only point in space.
   *
   * [MDN Reference](https://developer.mozilla.org/docs/Web/API/DOMPointReadOnly/w)
   */
  readonly w: number;
  /**
   * The **`DOMPointReadOnly`** interface's **`x`** property holds the horizontal coordinate, x, for a read-only point in space.
   *
   * [MDN Reference](https://developer.mozilla.org/docs/Web/API/DOMPointReadOnly/x)
   */
  readonly x: number;
  /**
   * The **`DOMPointReadOnly`** interface's **`y`** property holds the vertical coordinate, y, for a read-only point in space.
   *
   * [MDN Reference](https://developer.mozilla.org/docs/Web/API/DOMPointReadOnly/y)
   */
  readonly y: number;
  /**
   * The **`DOMPointReadOnly`** interface's **`z`** property holds the depth coordinate, z, for a read-only point in space.
   *
   * [MDN Reference](https://developer.mozilla.org/docs/Web/API/DOMPointReadOnly/z)
   */
  readonly z: number;
  /**
   * The **`matrixTransform()`** method of the DOMPointReadOnly interface applies a matrix transform specified as an object to the DOMPointReadOnly object, creating and returning a new `DOMPointReadOnly` object.
   *
   * [MDN Reference](https://developer.mozilla.org/docs/Web/API/DOMPointReadOnly/matrixTransform)
   */
  matrixTransform(matrix?: DOMMatrixInit): DOMPoint;
  /**
   * The DOMPointReadOnly method `toJSON()` returns an object giving the ```js-nolint toJSON() ``` None.
   *
   * [MDN Reference](https://developer.mozilla.org/docs/Web/API/DOMPointReadOnly/toJSON)
   */
  toJSON(): any;
}

/**
 * The **`DOMPointReadOnly`** interface specifies the coordinate and perspective fields used by DOMPoint to define a 2D or 3D point in a coordinate system.
 *
 * [MDN](https://developer.mozilla.org/docs/Web/API/DOMPointReadOnly)
 *
 * @category Geometry Interfaces Module API
 * @experimental
 */
declare var DOMPointReadOnly: {
  prototype: DOMPointReadOnly;
  new (x?: number, y?: number, z?: number, w?: number): DOMPointReadOnly;
  /**
   * The static **DOMPointReadOnly** method `fromPoint()` creates and returns a new `DOMPointReadOnly` object given a source point.
   *
   * [MDN Reference](https://developer.mozilla.org/docs/Web/API/DOMPointReadOnly/fromPoint_static)
   */
  fromPoint(other?: DOMPointInit): DOMPointReadOnly;
};

/**
 * @category Geometry Interfaces Module API
 * @experimental
 */
interface DOMQuadInit {
  p1?: DOMPointInit;
  p2?: DOMPointInit;
  p3?: DOMPointInit;
  p4?: DOMPointInit;
}

/**
 * A `DOMQuad` is a collection of four `DOMPoint`s defining the corners of an arbitrary quadrilateral.
 *
 * [MDN](https://developer.mozilla.org/docs/Web/API/DOMQuad)
 *
 * @category Geometry Interfaces Module API
 * @experimental
 */
interface DOMQuad {
  /**
   * The **`DOMQuad`** interface's **`p1`** property holds the DOMPoint object that represents one of the four corners of the `DOMQuad`.
   *
   * [MDN Reference](https://developer.mozilla.org/docs/Web/API/DOMQuad/p1)
   */
  readonly p1: DOMPoint;
  /**
   * The **`DOMQuad`** interface's **`p2`** property holds the DOMPoint object that represents one of the four corners of the `DOMQuad`.
   *
   * [MDN Reference](https://developer.mozilla.org/docs/Web/API/DOMQuad/p2)
   */
  readonly p2: DOMPoint;
  /**
   * The **`DOMQuad`** interface's **`p3`** property holds the DOMPoint object that represents one of the four corners of the `DOMQuad`.
   *
   * [MDN Reference](https://developer.mozilla.org/docs/Web/API/DOMQuad/p3)
   */
  readonly p3: DOMPoint;
  /**
   * The **`DOMQuad`** interface's **`p4`** property holds the DOMPoint object that represents one of the four corners of the `DOMQuad`.
   *
   * [MDN Reference](https://developer.mozilla.org/docs/Web/API/DOMQuad/p4)
   */
  readonly p4: DOMPoint;
  /**
   * The DOMQuad method `getBounds()` returns a DOMRect object representing the smallest rectangle that fully contains the `DOMQuad` object.
   *
   * [MDN Reference](https://developer.mozilla.org/docs/Web/API/DOMQuad/getBounds)
   */
  getBounds(): DOMRect;
  /**
   * The DOMQuad method `toJSON()` returns a ```js-nolint toJSON() ``` None.
   *
   * [MDN Reference](https://developer.mozilla.org/docs/Web/API/DOMQuad/toJSON)
   */
  toJSON(): any;
}

/**
 * A `DOMQuad` is a collection of four `DOMPoint`s defining the corners of an arbitrary quadrilateral.
 *
 * [MDN](https://developer.mozilla.org/docs/Web/API/DOMQuad)
 *
 * @category Geometry Interfaces Module API
 * @experimental
 */
declare var DOMQuad: {
  prototype: DOMQuad;
  new (
    p1?: DOMPointInit,
    p2?: DOMPointInit,
    p3?: DOMPointInit,
    p4?: DOMPointInit,
  ): DOMQuad;
  fromQuad(other?: DOMQuadInit): DOMQuad;
  fromRect(other?: DOMRectInit): DOMQuad;
};

/**
 * @category Geometry Interfaces Module API
 * @experimental
 */
interface DOMRectInit {
  height?: number;
  width?: number;
  x?: number;
  y?: number;
}

/**
 * A **`DOMRect`** describes the size and position of a rectangle.
 *
 * [MDN](https://developer.mozilla.org/docs/Web/API/DOMRect)
 *
 * @category Geometry Interfaces Module API
 * @experimental
 */
interface DOMRect extends DOMRectReadOnly {
  /**
   * The **`height`** property of the DOMRect interface represents the height of the rectangle.
   *
   * [MDN Reference](https://developer.mozilla.org/docs/Web/API/DOMRect/height)
   */
  height: number;
  /**
   * The **`width`** property of the DOMRect interface represents the width of the rectangle.
   *
   * [MDN Reference](https://developer.mozilla.org/docs/Web/API/DOMRect/width)
   */
  width: number;
  /**
   * The **`x`** property of the DOMRect interface represents the x-coordinate of the rectangle, which is the horizontal distance between the viewport's left edge and the rectangle's origin.
   *
   * [MDN Reference](https://developer.mozilla.org/docs/Web/API/DOMRect/x)
   */
  x: number;
  /**
   * The **`y`** property of the DOMRect interface represents the y-coordinate of the rectangle, which is the vertical distance between the viewport's top edge and the rectangle's origin.
   *
   * [MDN Reference](https://developer.mozilla.org/docs/Web/API/DOMRect/y)
   */
  y: number;
}

/**
 * A **`DOMRect`** describes the size and position of a rectangle.
 *
 * [MDN](https://developer.mozilla.org/docs/Web/API/DOMRect)
 *
 * @category Geometry Interfaces Module API
 * @experimental
 */
declare var DOMRect: {
  prototype: DOMRect;
  new (x?: number, y?: number, width?: number, height?: number): DOMRect;
  /**
   * The **`fromRect()`** static method of the object with a given location and dimensions.
   *
   * [MDN Reference](https://developer.mozilla.org/docs/Web/API/DOMRect/fromRect_static)
   */
  fromRect(other?: DOMRectInit): DOMRect;
};

/**
 * The **`DOMRectReadOnly`** interface specifies the standard properties (also used by DOMRect) to define a rectangle whose properties are immutable.
 *
 * [MDN](https://developer.mozilla.org/docs/Web/API/DOMRectReadOnly)
 *
 * @category Geometry Interfaces Module API
 * @experimental
 */
interface DOMRectReadOnly {
  /**
   * The **`bottom`** read-only property of the **`DOMRectReadOnly`** interface returns the bottom coordinate value of the `DOMRect`.
   *
   * [MDN Reference](https://developer.mozilla.org/docs/Web/API/DOMRectReadOnly/bottom)
   */
  readonly bottom: number;
  /**
   * The **`height`** read-only property of the **`DOMRectReadOnly`** interface represents the height of the `DOMRect`.
   *
   * [MDN Reference](https://developer.mozilla.org/docs/Web/API/DOMRectReadOnly/height)
   */
  readonly height: number;
  /**
   * The **`left`** read-only property of the **`DOMRectReadOnly`** interface returns the left coordinate value of the `DOMRect`.
   *
   * [MDN Reference](https://developer.mozilla.org/docs/Web/API/DOMRectReadOnly/left)
   */
  readonly left: number;
  /**
   * The **`right`** read-only property of the **`DOMRectReadOnly`** interface returns the right coordinate value of the `DOMRect`.
   *
   * [MDN Reference](https://developer.mozilla.org/docs/Web/API/DOMRectReadOnly/right)
   */
  readonly right: number;
  /**
   * The **`top`** read-only property of the **`DOMRectReadOnly`** interface returns the top coordinate value of the `DOMRect`.
   *
   * [MDN Reference](https://developer.mozilla.org/docs/Web/API/DOMRectReadOnly/top)
   */
  readonly top: number;
  /**
   * The **`width`** read-only property of the **`DOMRectReadOnly`** interface represents the width of the `DOMRect`.
   *
   * [MDN Reference](https://developer.mozilla.org/docs/Web/API/DOMRectReadOnly/width)
   */
  readonly width: number;
  /**
   * The **`x`** read-only property of the **`DOMRectReadOnly`** interface represents the x coordinate of the `DOMRect`'s origin.
   *
   * [MDN Reference](https://developer.mozilla.org/docs/Web/API/DOMRectReadOnly/x)
   */
  readonly x: number;
  /**
   * The **`y`** read-only property of the **`DOMRectReadOnly`** interface represents the y coordinate of the `DOMRect`'s origin.
   *
   * [MDN Reference](https://developer.mozilla.org/docs/Web/API/DOMRectReadOnly/y)
   */
  readonly y: number;
  /**
   * The DOMRectReadOnly method `toJSON()` returns a JSON representation of the `DOMRectReadOnly` object.
   *
   * [MDN Reference](https://developer.mozilla.org/docs/Web/API/DOMRectReadOnly/toJSON)
   */
  toJSON(): any;
}

/**
 * The **`DOMRectReadOnly`** interface specifies the standard properties (also used by DOMRect) to define a rectangle whose properties are immutable.
 *
 * [MDN](https://developer.mozilla.org/docs/Web/API/DOMRectReadOnly)
 *
 * @category Geometry Interfaces Module API
 * @experimental
 */
declare var DOMRectReadOnly: {
  prototype: DOMRectReadOnly;
  new (
    x?: number,
    y?: number,
    width?: number,
    height?: number,
  ): DOMRectReadOnly;
  /**
   * The **`fromRect()`** static method of the object with a given location and dimensions.
   *
   * [MDN Reference](https://developer.mozilla.org/docs/Web/API/DOMRectReadOnly/fromRect_static)
   */
  fromRect(other?: DOMRectInit): DOMRectReadOnly;
};
