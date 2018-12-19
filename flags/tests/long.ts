import { test, assertEqual } from "https://deno.land/x/testing/testing.ts";
import parseArgs from "../index.ts";

test(function longOpts() {
    assertEqual(
        parseArgs([ '--bool' ]),
        { bool : true, _ : [] },
    );
    assertEqual(
        parseArgs([ '--pow', 'xixxle' ]),
        { pow : 'xixxle', _ : [] },
    );
    assertEqual(
        parseArgs([ '--pow=xixxle' ]),
        { pow : 'xixxle', _ : [] },
    );
    assertEqual(
        parseArgs([ '--host', 'localhost', '--port', '555' ]),
        { host : 'localhost', port : 555, _ : [] },
    );
    assertEqual(
        parseArgs([ '--host=localhost', '--port=555' ]),
        { host : 'localhost', port : 555, _ : [] },
    );
});
