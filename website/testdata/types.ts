import BigNumber from "http://site.com/bignum/index.ts";

/**
 * comment 1
 */
export type Point = {
  /**
   * comment 2
   */
  x: number;
  /**
   * comment 3
   */
  y?: BigNumber.number;
}

/**
 * comment 4
 */
export type T01 = ReturnType<() => string>;

/**
 * comment 5
 */
export type T02 = "Name" | 2;

/**
 * comment 6
 */
export type T03<X> = X;

/**
 * comment 7
 */
export type T04<X, Y extends X> = {
  [key in keyof X]: Y[key]
};

/**
 * comment 8
 */
export type T05<A, B> = keyof (A & B);

/**
 * comment 9
 */
export type T06<X, Y> = X extends Y ? number : string;

/**
 * comment 10
 */
export type T07 = { [P in keyof Person]?: Person[P] };

/**
 * comment 11
 */
export type T08 = { [P in keyof Person]: Person[P] | null };
