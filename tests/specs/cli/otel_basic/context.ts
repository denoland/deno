import { assertEquals } from "@std/assert";

const { ContextManager } = Deno.telemetry;

const cm = new ContextManager();

const a = cm.active();
const b = a.setValue("b", 1);
const c = b.setValue("c", 2);

const subB = c.deleteValue("b");
const subC = subB.deleteValue("c");

assertEquals(a.getValue("b"), undefined);
assertEquals(b.getValue("b"), 1);
assertEquals(c.getValue("b"), 1);

assertEquals(a.getValue("c"), undefined);
assertEquals(b.getValue("c"), undefined);
assertEquals(c.getValue("c"), 2);

assertEquals(subB.getValue("b"), undefined);
assertEquals(subB.getValue("c"), 2);

assertEquals(subC.getValue("b"), undefined);
assertEquals(subC.getValue("c"), undefined);
