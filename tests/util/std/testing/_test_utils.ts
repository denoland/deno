// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
export class Point {
  constructor(public x: number, public y: number) {}
  // deno-lint-ignore no-explicit-any
  action(...args: any[]): any {
    return args[0];
  }
  toString(): string {
    return [this.x, this.y].join(", ");
  }
  explicitTypes(_x: number, _y: string) {
    return true;
  }
  *[Symbol.iterator](): IterableIterator<number> {
    yield this.x;
    yield this.y;
  }
}

export function stringifyPoint(point: Point) {
  return point.toString();
}

export type PointWithExtra = Point & {
  nonExistent: () => number;
};
