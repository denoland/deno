import type { Options } from "./types.ts";

export const customSymbol: unique symbol = Symbol("custom");

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
