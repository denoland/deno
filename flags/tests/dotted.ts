import { test, assertEqual } from "https://deno.land/x/testing/testing.ts";
import parseArgs from "../index.ts";

test(function dottedAlias() {
    const argv = parseArgs(['--a.b', '22'], {default: {'a.b': 11}, alias: {'a.b': 'aa.bb'}});
    assertEqual(argv.a.b, 22);
    assertEqual(argv.aa.bb, 22);
});

test(function dottedDefault() {
    const argv = parseArgs('', {default: {'a.b': 11}, alias: {'a.b': 'aa.bb'}});
    assertEqual(argv.a.b, 11);
    assertEqual(argv.aa.bb, 11);
});

test(function dottedDefaultWithNoAlias() {
    const argv = parseArgs('', {default: {'a.b': 11}});
    assertEqual(argv.a.b, 11);
});
