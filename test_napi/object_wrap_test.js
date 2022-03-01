import { assertEquals, loadTestLibrary } from "./common.js";

const objectWrap = loadTestLibrary();

Deno.test("napi object wrap new", function () {
  const obj = new objectWrap.NapiObject(0);
  assertEquals(obj.get_value(), 0);
  obj.set_value(10);
  assertEquals(obj.get_value(), 10);
});
