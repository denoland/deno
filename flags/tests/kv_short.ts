import { test, assertEqual } from "https://deno.land/x/testing/testing.ts";
import parseArgs from "../index.ts";

test(function short() {
    const argv = parseArgs([ '-b=123' ]);
    assertEqual(argv, { b: 123, _: [] });
});

test(function multiShort() {
    const argv = parseArgs([ '-a=whatever', '-b=robots' ]);
    assertEqual(argv, { a: 'whatever', b: 'robots', _: [] });
});
