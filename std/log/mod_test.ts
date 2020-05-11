const { test } = Deno;
import { assertEquals } from "../testing/asserts.ts";
import { debug, info, warning, error, critical } from "./mod.ts";

test("default loggers work as expected", function (): void {
  const sym = Symbol("a");
  const debugData = debug("foo");
  const debugResolver = debug(() => "foo");
  const infoData = info(456, 1, 2, 3);
  const infoResolver = info(() => true);
  const warningData = warning(sym);
  const warningResolver = warning(() => null);
  const errorData = error(undefined, 1, 2, 3);
  const errorResolver = error(() => 5n);
  const criticalData = critical("foo");
  const criticalResolver = critical(() => "bar");
  assertEquals(debugData, "foo");
  assertEquals(debugResolver, undefined);
  assertEquals(infoData, { msg: 456, args: [1, 2, 3] });
  assertEquals(infoResolver, true);
  assertEquals(warningData, sym);
  assertEquals(warningResolver, null);
  assertEquals(errorData, { msg: undefined, args: [1, 2, 3] });
  assertEquals(errorResolver, 5n);
  assertEquals(criticalData, "foo");
  assertEquals(criticalResolver, "bar");
});
