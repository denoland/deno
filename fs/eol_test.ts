// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { test } from "../testing/mod.ts";
import { assertEquals } from "../testing/asserts.ts";
import { format, detect, EOL } from "./eol.ts";

const CRLFinput = "deno\r\nis not\r\nnode";
const Mixedinput = "deno\nis not\r\nnode";
const Mixedinput2 = "deno\r\nis not\nnode";
const LFinput = "deno\nis not\nnode";
const NoNLinput = "deno is not node";

test(function detectCRLF() {
  assertEquals(detect(CRLFinput), EOL.CRLF);
});
test(function detectLF() {
  assertEquals(detect(LFinput), EOL.LF);
});
test(function detectNoNewLine() {
  assertEquals(detect(NoNLinput), null);
});
test(function testFormat() {
  assertEquals(format(CRLFinput, EOL.LF), LFinput);
  assertEquals(format(LFinput, EOL.LF), LFinput);
  assertEquals(format(LFinput, EOL.CRLF), CRLFinput);
  assertEquals(format(CRLFinput, EOL.CRLF), CRLFinput);
  assertEquals(format(CRLFinput, EOL.CRLF), CRLFinput);
  assertEquals(format(NoNLinput, EOL.CRLF), NoNLinput);
  assertEquals(format(Mixedinput, EOL.CRLF), CRLFinput);
  assertEquals(format(Mixedinput, EOL.LF), LFinput);
  assertEquals(format(Mixedinput2, EOL.CRLF), CRLFinput);
  assertEquals(format(Mixedinput2, EOL.LF), LFinput);
});
