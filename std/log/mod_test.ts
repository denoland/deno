const { test } = Deno;
import { assertEquals } from "../testing/asserts.ts";
import { debug, info, warning, error, critical } from "./mod.ts";

test("default loggers work as expected", function (): void {
  const sym = Symbol("a");
  const debugData: string = debug("foo");
  const debugResolver: string | undefined = debug(() => "foo");
  const infoData: number = info(456, 1, 2, 3);
  const infoResolver: boolean | undefined = info(() => true);
  const warningData: symbol = warning(sym);
  const warningResolver: null | undefined = warning(() => null);
  const errorData: undefined = error(undefined, 1, 2, 3);
  const errorResolver: bigint | undefined = error(() => 5n);
  const criticalData: string = critical("foo");
  const criticalResolver: string | undefined = critical(() => "bar");
  assertEquals(debugData, "foo");
  assertEquals(debugResolver, undefined);
  assertEquals(infoData, 456);
  assertEquals(infoResolver, true);
  assertEquals(warningData, sym);
  assertEquals(warningResolver, null);
  assertEquals(errorData, undefined);
  assertEquals(errorResolver, 5n);
  assertEquals(criticalData, "foo");
  assertEquals(criticalResolver, "bar");
});
