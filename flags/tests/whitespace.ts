import { test, assertEqual } from "https://deno.land/x/testing/testing.ts";
import parseArgs from "../index.ts";

test(function whitespaceShouldBeWhitespace() {
    assertEqual(parseArgs([ '-x', '\t' ]).x, '\t');
});
