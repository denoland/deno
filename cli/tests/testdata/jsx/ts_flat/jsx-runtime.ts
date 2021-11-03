// deno-lint-ignore-file no-explicit-any
export function jsx(
  _type: any,
  _props: any,
  _key: string,
  _source?: string,
  _self?: string,
): any {}
export const jsxs = jsx;
export const jsxDEV = jsx;
export const Fragment = Symbol("Fragment");

console.log("imported", import.meta.url);
