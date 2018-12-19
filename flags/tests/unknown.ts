import { test, assertEqual } from "https://deno.land/x/testing/testing.ts";
import parseArgs from "../index.ts";

test(function booleanAndAliasIsNotUnknown() {
    const unknown = [];
    function unknownFn(arg) {
        unknown.push(arg);
        return false;
    }
    const aliased = [ '-h', 'true', '--derp', 'true' ];
    const regular = [ '--herp',  'true', '-d', 'true' ];
    const opts = {
        alias: { h: 'herp' },
        boolean: 'h',
        unknown: unknownFn
    };
    const aliasedArgv = parseArgs(aliased, opts);
    const propertyArgv = parseArgs(regular, opts);

    assertEqual(unknown, ['--derp', '-d']);
});

test(function flagBooleanTrueAnyDoubleHyphenArgumentIsNotUnknown() {
    const unknown = [];
    function unknownFn(arg) {
        unknown.push(arg);
        return false;
    }
    const argv = parseArgs(['--honk', '--tacos=good', 'cow', '-p', '55'], {
        boolean: true,
        unknown: unknownFn
    });
    assertEqual(unknown, ['--tacos=good', 'cow', '-p']);
    assertEqual(argv, {
        honk: true,
        _: []
    });
});

test(function stringAndAliasIsNotUnkown() {
    const unknown = [];
    function unknownFn(arg) {
        unknown.push(arg);
        return false;
    }
    const aliased = [ '-h', 'hello', '--derp', 'goodbye' ];
    const regular = [ '--herp',  'hello', '-d', 'moon' ];
    const opts = {
        alias: { h: 'herp' },
        string: 'h',
        unknown: unknownFn
    };
    const aliasedArgv = parseArgs(aliased, opts);
    const propertyArgv = parseArgs(regular, opts);

    assertEqual(unknown, ['--derp', '-d']);
});

test(function defaultAndAliasIsNotUnknown() {
    const unknown = [];
    function unknownFn(arg) {
        unknown.push(arg);
        return false;
    }
    const aliased = [ '-h', 'hello' ];
    const regular = [ '--herp',  'hello' ];
    const opts = {
        default: { 'h': 'bar' },
        alias: { 'h': 'herp' },
        unknown: unknownFn
    };
    const aliasedArgv = parseArgs(aliased, opts);
    const propertyArgv = parseArgs(regular, opts);

    assertEqual(unknown, []);
});

test(function valueFollowingDoubleHyphenIsNotUnknown() {
    const unknown = [];
    function unknownFn(arg) {
        unknown.push(arg);
        return false;
    }
    const aliased = [ '--bad', '--', 'good', 'arg' ];
    const opts = {
        '--': true,
        unknown: unknownFn
    };
    const argv = parseArgs(aliased, opts);

    assertEqual(unknown, ['--bad']);
    assertEqual(argv, {
        '--': ['good', 'arg'],
        '_': []
    })
});
