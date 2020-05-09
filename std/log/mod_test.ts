const { test } = Deno;
import { assertEquals } from "../testing/asserts.ts";
import { debug, info, warning, error, critical } from "./mod.ts";

test("default loggers work as expected", function (): void {
  const debugData = debug("foo");
  const debugResolver = debug(() => "foo");
  const infoData = info("foo", 1, 2, 3);
  const infoResolver = info(() => "bar");
  const warningData = warning("foo");
  const warningResolver = warning(() => "bar");
  const errorData = error("foo", 1, 2, 3);
  const errorResolver = error(() => "bar");
  const criticalData = critical("foo");
  const criticalResolver = critical(() => "bar");
  assertEquals(debugData, "foo");
  assertEquals(debugResolver, undefined);
  assertEquals(infoData, { msg: "foo", args: [1, 2, 3] });
  assertEquals(infoResolver, "bar");
  assertEquals(warningData, "foo");
  assertEquals(warningResolver, "bar");
  assertEquals(errorData, { msg: "foo", args: [1, 2, 3] });
  assertEquals(errorResolver, "bar");
  assertEquals(criticalData, "foo");
  assertEquals(criticalResolver, "bar");
});
