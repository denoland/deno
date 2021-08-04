// deno-lint-ignore-file
export namespace NS {
  export function test(name: string, fn: Function);
  export function test(options: object);
  export function test(name: string | object, fn?: Function) {}
}
