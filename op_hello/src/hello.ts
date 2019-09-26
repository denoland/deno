const OP_HELLO: number = Deno.ops.add("hello");

// https://www.typescriptlang.org/docs/handbook/namespaces.html#splitting-across-files
namespace Deno {
  /**
   * The typedoc here ideally would be presevered automatically in
   * lib.deno_runtime.d.ts
   */
  export function hello() {
    Deno.core.send(OP_HELLO);
  }
}
