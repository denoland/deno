/**
 * Represents a point in 3D space.
 */
export interface Point {
  /**
   * Represents x value of a point.
   */
  x: number;

  /**
   * Represents x value of a point.
   */
  y: number;

  /**
   * Represents x value of a point.
   */
  z: number;
}

export interface Vec4 extends Point {
  t: number;
}

export interface V<T extends Vec4> {
  points: T[];
  f: (p: T) => number;
}
