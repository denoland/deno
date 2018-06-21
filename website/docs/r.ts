import { z } from "g";
import { t } from "lib";
import { g } from "h";

type y = g;
export type x = y | z & t;
export type r = x[] | z;
export type u = [x, y];
export type o = (p: x) => y;

/**
 * A function
 * @param x Something
 */
export function f<T, B extends C>(x: x, t: x | y, r: T): T {
  return r;
}

/**
 * Some other function
 * @param x another parameter
 */
export function p(p: 23, x?: z.g, h?: z.g.y): string {
  return "A";
}
