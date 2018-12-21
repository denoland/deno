import { test, assertEqual } from "https://deno.land/x/testing/testing.ts";
import { parse } from "../index.ts";

test(function longOpts() {
    assertEqual(
        parse([ '--bool' ]),
        { bool : true, _ : [] },
    );
    assertEqual(
        parse([ '--pow', 'xixxle' ]),
        { pow : 'xixxle', _ : [] },
    );
    assertEqual(
        parse([ '--pow=xixxle' ]),
        { pow : 'xixxle', _ : [] },
    );
    assertEqual(
        parse([ '--host', 'localhost', '--port', '555' ]),
        { host : 'localhost', port : 555, _ : [] },
    );
    assertEqual(
        parse([ '--host=localhost', '--port=555' ]),
        { host : 'localhost', port : 555, _ : [] },
    );
});
