import { test, assertEqual } from "https://deno.land/x/testing/testing.ts";
import parseArgs from "../index.ts";

test(function booleanDefaultTrue() {
    const argv = parseArgs([], {
        boolean: 'sometrue',
        default: { sometrue: true }
    });
    assertEqual(argv.sometrue, true);
});

test(function booleanDefaultFalse() {
    const argv = parseArgs([], {
        boolean: 'somefalse',
        default: { somefalse: false }
    });
    assertEqual(argv.somefalse, false);
});

test(function booleanDefaultNull() {
    const argv = parseArgs([], {
        boolean: 'maybe',
        default: { maybe: null }
    });
    assertEqual(argv.maybe, null);
    const argv2 = parseArgs(['--maybe'], {
        boolean: 'maybe',
        default: { maybe: null }
    });
    assertEqual(argv2.maybe, true);

})
