// Regression test for https://github.com/denoland/deno/issues/30285
//
// An overloaded generic function whose implementation signature declares type
// parameters used only in its (removed) body would trip TS6205 ("All type
// parameters are unused") during the public API type check that `deno publish`
// performs, because fast check strips the implementation body. `deno check`
// reported no error, so the two were inconsistent.

export type Handler<R> = (req: R) => R;
export type Middleware<R, P> = (req: R, params: P) => R;
export type Chain<R, P> = (req: R, params: P) => R;

export function chain<R>(handler: Handler<R>): Handler<R>;
export function chain<R, P>(middleware: Middleware<R, P>): Chain<R, P>;
export function chain<R, P>(
  middleware: Handler<R> | Middleware<R, P>,
): Handler<R> | Chain<R, P> {
  return middleware as Handler<R> | Chain<R, P>;
}
