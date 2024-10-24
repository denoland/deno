// deno-lint-ignore-file no-explicit-any
export function jsx(
  _type: any,
  _props: any,
  _key: any,
  _source: any,
  _self: any,
) {}
// deno-lint-ignore-file no-explicit-any
export const jsxAttr = (name: string, value: any) => `${name}="${value}"`;
// deno-lint-ignore-file no-explicit-any
export const jsxTemplate = (_template: string[], ..._exprs: any[]) => "";
// deno-lint-ignore-file no-explicit-any
export const jsxEscape = (_value: any) => "";
console.log("imported", import.meta.url);

declare global {
  namespace JSX {
    interface IntrinsicElements {
      [tagName: string]: Record<string, any>;
    }
  }
}
