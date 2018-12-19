import { test, assertEqual } from "https://deno.land/x/testing/testing.ts";
import parseArgs from "../index.ts";

// stops parsing on the first non-option when stopEarly is set
test(function stopParsing() {
    const argv = parseArgs(['--aaa', 'bbb', 'ccc', '--ddd'], {
        stopEarly: true
    });

    assertEqual(argv, {
        aaa: 'bbb',
        _: ['ccc', '--ddd']
    });
});
