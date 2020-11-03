// deno-lint-ignore-file

import { B } from "./subdir/more_decorators.ts";

function Decorator() {
  return function (
    target: Record<string, any>,
    propertyKey: string,
    descriptor: TypedPropertyDescriptor<any>,
  ) {
    const originalFn: Function = descriptor.value as Function;
    descriptor.value = async function (...args: any[]) {
      return await originalFn.apply(this, args);
    };
    return descriptor;
  };
}

class SomeClass {
  @Decorator()
  async test(): Promise<void> {}
}

new SomeClass().test();
new B().method();
