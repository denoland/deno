import { test, assertEqual } from "https://deno.land/x/testing/testing.ts";
import parseArgs from "../index.ts";

test(function hyphen() {
    assertEqual(parseArgs([ '-n', '-' ]), { n: '-', _: [] });
    assertEqual(parseArgs([ '-' ]), { _: [ '-' ] });
    assertEqual(parseArgs([ '-f-' ]), { f: '-', _: [] });
    assertEqual(
        parseArgs([ '-b', '-' ], { boolean: 'b' }),
        { b: true, _: [ '-' ] }
    );
    assertEqual(
        parseArgs([ '-s', '-' ], { string: 's' }),
        { s: '-', _: [] }
    );
});

test(function doubleDash() {
    assertEqual(parseArgs([ '-a', '--', 'b' ]), { a: true, _: [ 'b' ] });
    assertEqual(parseArgs([ '--a', '--', 'b' ]), { a: true, _: [ 'b' ] });
    assertEqual(parseArgs([ '--a', '--', 'b' ]), { a: true, _: [ 'b' ] });
});

test(function moveArgsAfterDoubleDashIntoOwnArray() {
    assertEqual(
        parseArgs([ '--name', 'John', 'before', '--', 'after' ], { '--': true }),
        { name: 'John', _: [ 'before' ], '--': [ 'after' ] });
});
