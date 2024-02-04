// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

/// <reference no-default-lib="true" />
/// <reference lib="esnext" />

/** @category Geometry Interfaces Module API */
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

/** @category Geometry Interfaces Module API */
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
interface DOMMatrix extends DOMMatrixReadOnly {
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
  invertSelf(): DOMMatrix;
  multiplySelf(other?: DOMMatrixInit): DOMMatrix;
  preMultiplySelf(other?: DOMMatrixInit): DOMMatrix;
  rotateAxisAngleSelf(
    x?: number,
    y?: number,
    z?: number,
    angle?: number,
  ): DOMMatrix;
  rotateFromVectorSelf(x?: number, y?: number): DOMMatrix;
  rotateSelf(rotX?: number, rotY?: number, rotZ?: number): DOMMatrix;
  scale3dSelf(
    scale?: number,
    originX?: number,
    originY?: number,
    originZ?: number,
  ): DOMMatrix;
  scaleSelf(
    scaleX?: number,
    scaleY?: number,
    scaleZ?: number,
    originX?: number,
    originY?: number,
    originZ?: number,
  ): DOMMatrix;
  /** Not available in Worker */
  setMatrixValue(transformList: string): DOMMatrix;
  skewXSelf(sx?: number): DOMMatrix;
  skewYSelf(sy?: number): DOMMatrix;
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
  /** Not available in Worker */
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
interface DOMMatrixReadOnly {
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
  flipX(): DOMMatrix;
  flipY(): DOMMatrix;
  inverse(): DOMMatrix;
  multiply(other?: DOMMatrixInit): DOMMatrix;
  rotate(rotX?: number, rotY?: number, rotZ?: number): DOMMatrix;
  rotateAxisAngle(
    x?: number,
    y?: number,
    z?: number,
    angle?: number,
  ): DOMMatrix;
  rotateFromVector(x?: number, y?: number): DOMMatrix;
  scale(
    scaleX?: number,
    scaleY?: number,
    scaleZ?: number,
    originX?: number,
    originY?: number,
    originZ?: number,
  ): DOMMatrix;
  scale3d(
    scale?: number,
    originX?: number,
    originY?: number,
    originZ?: number,
  ): DOMMatrix;
  /** @deprecated Supported for legacy reasons to be compatible with `SVGMatrix` as defined in SVG 1.1. Use `scale()` instead. */
  scaleNonUniform(scaleX?: number, scaleY?: number): DOMMatrix;
  skewX(sx?: number): DOMMatrix;
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
  /** Not available in Worker */
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
  /** Not available in Worker */
  new (init: string): DOMMatrixReadOnly;
  fromFloat32Array(array32: Float32Array): DOMMatrixReadOnly;
  fromFloat64Array(array64: Float64Array): DOMMatrixReadOnly;
  fromMatrix(other?: DOMMatrixInit): DOMMatrixReadOnly;
};

/** @category Geometry Interfaces Module API */
interface DOMPointInit {
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
interface DOMPoint extends DOMPointReadOnly {
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
interface DOMPointReadOnly {
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
interface DOMQuadInit {
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
interface DOMQuad {
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
interface DOMRectInit {
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
interface DOMRect extends DOMRectReadOnly {
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
interface DOMRectReadOnly {
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
