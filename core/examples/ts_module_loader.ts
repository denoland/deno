declare namespace Deno {
  namespace core {
    function print(str: string, isError?: boolean): void;
  }
}

Deno.core.print("Test\n");
