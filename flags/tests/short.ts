import { test, assertEqual } from "https://deno.land/x/testing/testing.ts";
import parseArgs from "../index.ts";

test(function numbericShortArgs() {
    assertEqual(parseArgs([ '-n123' ]), { n: 123, _: [] });
    assertEqual(
        parseArgs([ '-123', '456' ]),
        { 1: true, 2: true, 3: 456, _: [] }
    );
});

test(function short() {
    assertEqual(
        parseArgs([ '-b' ]),
        { b : true, _ : [] },
    );
    assertEqual(
        parseArgs([ 'foo', 'bar', 'baz' ]),
        { _ : [ 'foo', 'bar', 'baz' ] },
    );
    assertEqual(
        parseArgs([ '-cats' ]),
        { c : true, a : true, t : true, s : true, _ : [] },
    );
    assertEqual(
        parseArgs([ '-cats', 'meow' ]),
        { c : true, a : true, t : true, s : 'meow', _ : [] },
    );
    assertEqual(
        parseArgs([ '-h', 'localhost' ]),
        { h : 'localhost', _ : [] },
    );
    assertEqual(
        parseArgs([ '-h', 'localhost', '-p', '555' ]),
        { h : 'localhost', p : 555, _ : [] },
    );
});
 
test(function mixedShortBoolAndCapture() {
    assertEqual(
        parseArgs([ '-h', 'localhost', '-fp', '555', 'script.js' ]),
        {
            f : true, p : 555, h : 'localhost',
            _ : [ 'script.js' ]
        }
    );
});
 
test(function shortAndLong() {
    assertEqual(
        parseArgs([ '-h', 'localhost', '-fp', '555', 'script.js' ]),
        {
            f : true, p : 555, h : 'localhost',
            _ : [ 'script.js' ]
        }
    );
});
