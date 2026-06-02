declare const tempGlobalThis: typeof globalThis;
declare const tempGlobal: typeof global;
declare const tempProcess: typeof process;
export {
  tempGlobal as global,
  tempGlobalThis as globalThis,
  tempProcess as process,
};

type AssertTrue<T extends true> = never;
type _TestHasProcessGlobal = AssertTrue<
  typeof globalThis extends { process: any } ? true : false
>;

// Regression test for https://github.com/denoland/deno/issues/32682.
// Inside an npm package, references to web-standard globals like
// `RequestInit`, `ResponseInit`, and `Response` must resolve to Deno's
// versions (with `body`/`status`/etc.) instead of `@types/node`'s
// conditional interfaces, which can degrade to `{}` when the @types/node
// declarations are processed alongside Deno's libs.
type _TestRequestInitHasBody = AssertTrue<
  "body" extends keyof RequestInit ? true : false
>;
type _TestRequestInitHasSignal = AssertTrue<
  "signal" extends keyof RequestInit ? true : false
>;
type _TestResponseInitHasStatus = AssertTrue<
  "status" extends keyof ResponseInit ? true : false
>;
type _TestRequestInitHasHeaders = AssertTrue<
  "headers" extends keyof RequestInit ? true : false
>;

export function deleteSetTimeout(): void;
export function getSetTimeout(): typeof setTimeout;

export function checkWindowGlobal(): void;
export function checkSelfGlobal(): void;

export function getFoo(): string;
