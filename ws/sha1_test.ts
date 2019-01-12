import { assertEqual, test } from "../testing/mod.ts";
import { Sha1 } from "./sha1.ts";

test(function testSha1() {
  const sha1 = new Sha1();
  sha1.update("abcde");
  assertEqual(sha1.toString(), "03de6c570bfe24bfc328ccd7ca46b76eadaf4334");
});
