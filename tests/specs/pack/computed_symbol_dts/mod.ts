import type { Options } from "./types.ts";
export { looseValue } from "./member/mod.ts";

export const customSymbol: unique symbol = Symbol("custom");
export const strictValue = null;
const rootIndex: number = 0 as number;
export const rootValue = ["root"][rootIndex];

export class Base {
  inherited(): string {
    return "base";
  }
}

export class Resource extends Base {
  regular(): number {
    return 1;
  }

  [customSymbol](): boolean {
    return true;
  }

  [Symbol.dispose](): void {}

  async [Symbol.asyncDispose](): Promise<void> {}

  configure(_options: Options): void {}

  overloaded(value: string): string;
  overloaded(value: number): number;
  overloaded(value: string | number): string | number {
    return value;
  }
}
