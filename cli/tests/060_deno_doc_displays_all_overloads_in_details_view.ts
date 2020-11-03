// deno-lint-ignore-file
export namespace NS {
  export function test(name: string, fn: Function): void;
  export function test(options: object): void;
  export function test(name: string | object, fn?: Function): void {}
}
