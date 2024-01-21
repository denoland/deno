// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

// deno-lint-ignore-file no-var

/// <reference no-default-lib="true" />
/// <reference lib="esnext" />

/** @category Geometry Interfaces Module API */
declare interface DOMMatrix2DInit {
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

/** @category Geometry Interfaces Module API */
declare interface DOMMatrixInit extends DOMMatrix2DInit {
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
 */
declare interface DOMMatrix extends DOMMatrixReadOnly {
  a: number;
  b: number;
  c: number;
  d: number;
  e: number;
  f: number;
  m11: number;
  m12: number;
  m13: number;
  m14: number;
  m21: number;
  m22: number;
  m23: number;
  m24: number;
  m31: number;
  m32: number;
  m33: number;
  m34: number;
  m41: number;
  m42: number;
  m43: number;
  m44: number;
  /**
   * Modifies the matrix by inverting it.
   * If the matrix can't be inverted, its components are all set to `NaN`, and is2D property is set to `false`.
   */
  invertSelf(): DOMMatrix;
  /**
   * Modifies the matrix by post-multiplying it with the specified DOMMatrix.
   * This is equivalent to the dot product `A⋅B`, where matrix `A` is the source matrix and `B` is the matrix given as an input to the method.
   *
   * @param other
   */
  multiplySelf(other?: DOMMatrixInit): DOMMatrix;
  /**
   * Modifies the matrix by pre-multiplying it with the specified DOMMatrix.
   * This is equivalent to the dot product B⋅A, where matrix `A` is the source matrix and `B` is the matrix given as an input to the method.
   *
   * @param other
   */
  preMultiplySelf(other?: DOMMatrixInit): DOMMatrix;
  /**
   * Modifies the matrix by rotating it by the specified angle around the given vector.
   *
   * @param x
   * @param y
   * @param z
   * @param angle in degrees
   */
  rotateAxisAngleSelf(
    x?: number,
    y?: number,
    z?: number,
    angle?: number,
  ): DOMMatrix;
  /**
   * Modifies the matrix by rotating it by the angle between the specified vector and `(1, 0)`.
   *
   * @param x
   * @param y
   */
  rotateFromVectorSelf(x?: number, y?: number): DOMMatrix;
  /**
   * Modifies the matrix by rotating itself around each axis by the specified number of degrees.
   *
   * @param rotZ yaw angle in degrees
   */
  rotateSelf(rotZ?: number): DOMMatrix;
  /**
   * Modifies the matrix by rotating itself around each axis by the specified number of degrees.
   *
   * @param rotX roll angle in degrees
   * @param rotY pitch angle in degrees
   * @param rotZ yaw angle in degrees
   */
  rotateSelf(rotX?: number, rotY?: number, rotZ?: number): DOMMatrix;
  /**
   * Modifies the matrix by applying the specified scaling factor to all three axes, centered on the given origin.
   *
   * @param scale
   * @param originX
   * @param originY
   * @param originZ
   */
  scale3dSelf(
    scale?: number,
    originX?: number,
    originY?: number,
    originZ?: number,
  ): DOMMatrix;
  /**
   * Modifies the matrix by applying the specified scaling factors, with the center located at the specified origin. Also returns itself.
   * By default, the X and Z axes are scaled by `1` and the Y axis is given the same scaling value as the X axis.
   * The default origin is `(0, 0, 0)`.
   *
   * @param scaleX
   * @param scaleY
   * @param scaleZ
   * @param originX
   * @param originY
   * @param originZ
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
   * NOTE: Not available in Worker
   *
   * Replaces the contents of the matrix with the matrix described by the specified transform or transforms.
   *
   * @param transformList
   */
  setMatrixValue(transformList: string): DOMMatrix;
  /**
   * Modifies the matrix by applying the specified skew transformation along the X-axis.
   *
   * @param sx in degrees
   */
  skewXSelf(sx?: number): DOMMatrix;
  /**
   * Modifies the matrix by applying the specified skew transformation along the Y-axis.
   *
   * @param sy in degrees
   */
  skewYSelf(sy?: number): DOMMatrix;
  /**
   * Modifies the matrix by applying the specified vector. The default vector is `(0, 0, 0)`.
   *
   * @param tx
   * @param ty
   * @param tz
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
 */
declare var DOMMatrix: {
  prototype: DOMMatrix;
  new (init?: number[]): DOMMatrix;
  new (init: DOMMatrix | DOMMatrixReadOnly): DOMMatrix;
  /** NOTE: Not available in Worker */
  new (init: string): DOMMatrix;
  fromFloat32Array(array32: Float32Array): DOMMatrix;
  fromFloat64Array(array64: Float64Array): DOMMatrix;
  fromMatrix(other?: DOMMatrixInit): DOMMatrix;
};

/**
 * A read-only 4×4 matrix (column-major order), suitable for 2D and 3D operations including rotation and translation.
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
 */
declare interface DOMMatrixReadOnly {
  readonly a: number;
  readonly b: number;
  readonly c: number;
  readonly d: number;
  readonly e: number;
  readonly f: number;
  readonly is2D: boolean;
  readonly isIdentity: boolean;
  readonly m11: number;
  readonly m12: number;
  readonly m13: number;
  readonly m14: number;
  readonly m21: number;
  readonly m22: number;
  readonly m23: number;
  readonly m24: number;
  readonly m31: number;
  readonly m32: number;
  readonly m33: number;
  readonly m34: number;
  readonly m41: number;
  readonly m42: number;
  readonly m43: number;
  readonly m44: number;
  /** Returns a new `DOMMatrix` created by flipping the source matrix around its X-axis. */
  flipX(): DOMMatrix;
  /** Returns a new `DOMMatrix` created by flipping the source matrix around its Y-axis. */
  flipY(): DOMMatrix;
  /**
   * Returns a new `DOMMatrix` created by inverting the source matrix.
   * If the matrix cannot be inverted, the new matrix's components are all set to `NaN` and its is2D property is set to `false`.
   */
  inverse(): DOMMatrix;
  /**
   * Returns a new `DOMMatrix` created by computing the dot product of the source matrix and the specified matrix: `A⋅B`.
   *
   * @param other
   */
  multiply(other?: DOMMatrixInit): DOMMatrix;
  /**
   * Returns a new `DOMMatrix` created by rotating the source matrix around each of its axes by the specified number of degrees.
   *
   * @param rotZ yaw angle in degrees
   */
  rotate(rotZ?: number): DOMMatrix;
  /**
   * Returns a new `DOMMatrix` created by rotating the source matrix around each of its axes by the specified number of degrees.
   *
   * @param rotX roll angle in degrees
   * @param rotY pitch angle in degrees
   * @param rotZ yaw angle in degrees
   */
  rotate(rotX?: number, rotY?: number, rotZ?: number): DOMMatrix;
  /**
   * Returns a new DOMMatrix created by rotating the source matrix by the given angle around the specified vector.
   *
   * @param x
   * @param y
   * @param z
   * @param angle in degrees
   */
  rotateAxisAngle(
    x?: number,
    y?: number,
    z?: number,
    angle?: number,
  ): DOMMatrix;
  /**
   * Returns a new `DOMMatrix` created by rotating the source matrix by the angle between the specified vector and `(1, 0)`.
   *
   * @param x
   * @param y
   */
  rotateFromVector(x?: number, y?: number): DOMMatrix;
  /**
   * Returns a new `DOMMatrix` created by scaling the source matrix by the amount specified for each axis, centered on the given origin.
   * By default, the X and Z axes are scaled by `1` and the Y axis is given the same scaling value as the X axis.
   * The default origin is `(0, 0, 0)`.
   *
   * @param scaleX
   * @param scaleY
   * @param scaleZ
   * @param originX
   * @param originY
   * @param originZ
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
   * Returns a new `DOMMatrix` created by scaling the source 3D matrix by the given factor along all its axes, centered on the specified origin point.
   * The default origin is `(0, 0, 0)`.
   *
   * @param scale
   * @param originX
   * @param originY
   * @param originZ
   */
  scale3d(
    scale?: number,
    originX?: number,
    originY?: number,
    originZ?: number,
  ): DOMMatrix;
  /**
   * Returns a new `DOMMatrix` created by applying the specified scaling on the X, Y, and Z axes, centered at the given origin.
   * By default, the X and Y axes' scaling factors are both `1`.
   *
   * @deprecated Supported for legacy reasons to be compatible with `SVGMatrix` as defined in SVG 1.1. Use `scale()` instead.
   *
   * @param scaleX
   * @param scaleY
   */
  scaleNonUniform(scaleX?: number, scaleY?: number): DOMMatrix;
  /**
   * Returns a new DOMMatrix created by applying the specified skew transformation to the source matrix along its X-axis.
   *
   * @param sx in degrees
   */
  skewX(sx?: number): DOMMatrix;
  /**
   * Returns a new DOMMatrix created by applying the specified skew transformation to the source matrix along its Y-axis.
   *
   * @param sy in degrees
   */
  skewY(sy?: number): DOMMatrix;
  toFloat32Array(): Float32Array;
  toFloat64Array(): Float64Array;
  toJSON(): {
    a: number;
    b: number;
    c: number;
    d: number;
    e: number;
    f: number;
    is2D: boolean;
    isIdentity: boolean;
    m11: number;
    m12: number;
    m13: number;
    m14: number;
    m21: number;
    m22: number;
    m23: number;
    m24: number;
    m31: number;
    m32: number;
    m33: number;
    m34: number;
    m41: number;
    m42: number;
    m43: number;
    m44: number;
  };
  transformPoint(point?: DOMPointInit): DOMPoint;
  translate(tx?: number, ty?: number, tz?: number): DOMMatrix;
  /** NOTE: Not available in Worker */
  toString(): string;
}

/**
 * A read-only 4×4 matrix (column-major order), suitable for 2D and 3D operations including rotation and translation.
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
 */
declare var DOMMatrixReadOnly: {
  prototype: DOMMatrixReadOnly;
  new (init?: number[]): DOMMatrixReadOnly;
  new (init: DOMMatrix | DOMMatrixReadOnly): DOMMatrixReadOnly;
  /** Not available in Worker */
  new (init: string): DOMMatrixReadOnly;
  fromFloat32Array(array32: Float32Array): DOMMatrixReadOnly;
  fromFloat64Array(array64: Float64Array): DOMMatrixReadOnly;
  fromMatrix(other?: DOMMatrixInit): DOMMatrixReadOnly;
};

/** @category Geometry Interfaces Module API */
declare interface DOMPointInit {
  w?: number;
  x?: number;
  y?: number;
  z?: number;
}

/**
 * A object represents a 2D or 3D point in a coordinate system; it includes values for the coordinates in up to three dimensions, as well as an optional perspective value.
 * [MDN](https://developer.mozilla.org/docs/Web/API/DOMPoint)
 *
 * @category Geometry Interfaces Module API
 */
declare interface DOMPoint extends DOMPointReadOnly {
  w: number;
  x: number;
  y: number;
  z: number;
}

/**
 * A object represents a 2D or 3D point in a coordinate system; it includes values for the coordinates in up to three dimensions, as well as an optional perspective value.
 * [MDN](https://developer.mozilla.org/docs/Web/API/DOMPoint)
 *
 * @category Geometry Interfaces Module API
 */
declare var DOMPoint: {
  prototype: DOMPoint;
  new (x?: number, y?: number, z?: number, w?: number): DOMPoint;
  fromPoint(other?: DOMPointInit): DOMPoint;
};

/**
 * A read-only object represents a 2D or 3D point in a coordinate system; it includes values for the coordinates in up to three dimensions, as well as an optional perspective value.
 * [MDN](https://developer.mozilla.org/docs/Web/API/DOMPointReadOnly)
 *
 * @category Geometry Interfaces Module API
 */
declare interface DOMPointReadOnly {
  readonly w: number;
  readonly x: number;
  readonly y: number;
  readonly z: number;
  matrixTransform(matrix?: DOMMatrixInit): DOMPoint;
  toJSON(): {
    w: number;
    x: number;
    y: number;
    z: number;
  };
}

/**
 * A read-only object represents a 2D or 3D point in a coordinate system; it includes values for the coordinates in up to three dimensions, as well as an optional perspective value.
 * [MDN](https://developer.mozilla.org/docs/Web/API/DOMPointReadOnly)
 *
 * @category Geometry Interfaces Module API
 */
declare var DOMPointReadOnly: {
  prototype: DOMPointReadOnly;
  new (x?: number, y?: number, z?: number, w?: number): DOMPointReadOnly;
  fromPoint(other?: DOMPointInit): DOMPointReadOnly;
};

/** @category Geometry Interfaces Module API */
declare interface DOMQuadInit {
  p1?: DOMPointInit;
  p2?: DOMPointInit;
  p3?: DOMPointInit;
  p4?: DOMPointInit;
}

/**
 * A collection of four DOMPoints defining the corners of an arbitrary quadrilateral.
 * [MDN](https://developer.mozilla.org/docs/Web/API/DOMQuad)
 *
 * @category Geometry Interfaces Module API
 */
declare interface DOMQuad {
  readonly p1: DOMPoint;
  readonly p2: DOMPoint;
  readonly p3: DOMPoint;
  readonly p4: DOMPoint;
  getBounds(): DOMRect;
  toJSON(): {
    p1: DOMPoint;
    p2: DOMPoint;
    p3: DOMPoint;
    p4: DOMPoint;
  };
}

/**
 * A collection of four DOMPoints defining the corners of an arbitrary quadrilateral.
 * [MDN](https://developer.mozilla.org/docs/Web/API/DOMQuad)
 *
 * @category Geometry Interfaces Module API
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

/** @category Geometry Interfaces Module API */
declare interface DOMRectInit {
  height?: number;
  width?: number;
  x?: number;
  y?: number;
}

/**
 * [MDN](https://developer.mozilla.org/docs/Web/API/DOMRect)
 *
 * @category Geometry Interfaces Module API
 */
declare interface DOMRect extends DOMRectReadOnly {
  height: number;
  width: number;
  x: number;
  y: number;
}

/**
 * [MDN](https://developer.mozilla.org/docs/Web/API/DOMRect)
 *
 * @category Geometry Interfaces Module API
 */
declare var DOMRect: {
  prototype: DOMRect;
  new (x?: number, y?: number, width?: number, height?: number): DOMRect;
  fromRect(other?: DOMRectInit): DOMRect;
};

/**
 * [MDN](https://developer.mozilla.org/docs/Web/API/DOMRectReadOnly)
 *
 * @category Geometry Interfaces Module API
 */
declare interface DOMRectReadOnly {
  readonly bottom: number;
  readonly height: number;
  readonly left: number;
  readonly right: number;
  readonly top: number;
  readonly width: number;
  readonly x: number;
  readonly y: number;
  toJSON(): {
    bottom: number;
    height: number;
    left: number;
    right: number;
    top: number;
    width: number;
    x: number;
    y: number;
  };
}

/**
 * [MDN](https://developer.mozilla.org/docs/Web/API/DOMRectReadOnly)
 *
 * @category Geometry Interfaces Module API
 */
declare var DOMRectReadOnly: {
  prototype: DOMRectReadOnly;
  new (
    x?: number,
    y?: number,
    width?: number,
    height?: number,
  ): DOMRectReadOnly;
  fromRect(other?: DOMRectInit): DOMRectReadOnly;
};
