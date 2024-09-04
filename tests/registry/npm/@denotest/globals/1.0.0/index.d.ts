declare const tempGlobalThis: typeof globalThis;
declare const tempGlobal: typeof global;
declare const tempProcess: typeof process;
export {
  tempGlobalThis as globalThis,
  tempGlobal as global,
  tempProcess as process,
};

type AssertTrue<T extends true> = never;
type _TestHasProcessGlobal = AssertTrue<
  typeof globalThis extends { process: any } ? true : false
>;

export function deleteSetTimeout(): void;
export function getSetTimeout(): typeof setTimeout;

export function checkWindowGlobal(): void;
export function checkSelfGlobal(): void;

export function getFoo(): string;
