import * as types from "./point";

/**
 * Returns a new Point
 */
export class Point implements types.Point {
  constructor(
    public x: number,
    public y: number,
    public z: number) { }
  
  /**
   * Computes distance between this point and a second point passed by p.
   */
  distance(p: types.Point): number {
    return Math.sqrt(
      this.square(this.x - p.x) +
      this.square(this.y - p.y) +
      (this.z - p.z)
    );
  }

  /**
   * Returns an square of the given number
   */
  private square(n: number) {
    return n ** 2;
  }

  /**
   * Checks if a point is under X axis.
   */
  static isUnderXAxis(p: types.Point) {
    return p.x < 0;
  }
}
