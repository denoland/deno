import { test, assertEqual } from "https://deno.land/x/testing/testing.ts";
import { parse } from "../index.ts";

test(function whitespaceShouldBeWhitespace() {
    assertEqual(parse([ '-x', '\t' ]).x, '\t');
});
