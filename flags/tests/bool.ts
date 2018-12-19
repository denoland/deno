import { test, assertEqual } from "https://deno.land/x/testing/testing.ts";
import parseArgs from "../index.ts";

test(function flagBooleanDefaultFalse() {
    const argv = parseArgs(['moo'], {
        boolean: ['t', 'verbose'],
        default: { verbose: false, t: false }
    });
    
    assertEqual(argv, {
        verbose: false,
        t: false,
        _: ['moo']
    });
    
    assertEqual(typeof argv.verbose, 'boolean');
    assertEqual(typeof argv.t, 'boolean');
});

test(function booleanGroups() {
    const argv = parseArgs([ '-x', '-z', 'one', 'two', 'three' ], {
        boolean: ['x','y','z']
    });
    
    assertEqual(argv, {
        x : true,
        y : false,
        z : true,
        _ : [ 'one', 'two', 'three' ]
    });
    
    assertEqual(typeof argv.x, 'boolean');
    assertEqual(typeof argv.y, 'boolean');
    assertEqual(typeof argv.z, 'boolean');
});

test(function booleanAndAliasWithChainableApi() {
    const aliased = [ '-h', 'derp' ];
    const regular = [ '--herp',  'derp' ];
    const opts = {
        herp: { alias: 'h', boolean: true }
    };
    const aliasedArgv = parseArgs(aliased, {
        boolean: 'herp',
        alias: { h: 'herp' }
    });
    const propertyArgv = parseArgs(regular, {
        boolean: 'herp',
        alias: { h: 'herp' }
    });
    const expected = {
        herp: true,
        h: true,
        '_': [ 'derp' ]
    };
    
    assertEqual(aliasedArgv, expected);
    assertEqual(propertyArgv, expected); 
});

test(function booleanAndAliasWithOptionsHash() {
    const aliased = [ '-h', 'derp' ];
    const regular = [ '--herp', 'derp' ];
    const opts = {
        alias: { 'h': 'herp' },
        boolean: 'herp'
    };
    const aliasedArgv = parseArgs(aliased, opts);
    const propertyArgv = parseArgs(regular, opts);
    const expected = {
        herp: true,
        h: true,
        '_': [ 'derp' ]
    };
    assertEqual(aliasedArgv, expected);
    assertEqual(propertyArgv, expected);
});

test(function booleanAndAliasArrayWithOptionsHash() {
    const aliased = [ '-h', 'derp' ];
    const regular = [ '--herp', 'derp' ];
    const alt = [ '--harp', 'derp' ];
    const opts = {
        alias: { 'h': ['herp', 'harp'] },
        boolean: 'h'
    };
    const aliasedArgv = parseArgs(aliased, opts);
    const propertyArgv = parseArgs(regular, opts);
    const altPropertyArgv = parseArgs(alt, opts);
    const expected = {
        harp: true,
        herp: true,
        h: true,
        '_': [ 'derp' ]
    };
    assertEqual(aliasedArgv, expected);
    assertEqual(propertyArgv, expected);
    assertEqual(altPropertyArgv, expected);
});

test(function booleanAndAliasUsingExplicitTrue() {
    const aliased = [ '-h', 'true' ];
    const regular = [ '--herp',  'true' ];
    const opts = {
        alias: { h: 'herp' },
        boolean: 'h'
    };
    const aliasedArgv = parseArgs(aliased, opts);
    const propertyArgv = parseArgs(regular, opts);
    const expected = {
        herp: true,
        h: true,
        '_': [ ]
    };

    assertEqual(aliasedArgv, expected);
    assertEqual(propertyArgv, expected); 
});

// regression, see https://github.com/substack/node-optimist/issues/71
// boolean and --x=true
test(function booleanAndNonBoolean() {
    const parsed = parseArgs(['--boool', '--other=true'], {
        boolean: 'boool'
    });

    assertEqual(parsed.boool, true);
    assertEqual(parsed.other, 'true');

    const parsed2 = parseArgs(['--boool', '--other=false'], {
        boolean: 'boool'
    });
    
    assertEqual(parsed2.boool, true);
    assertEqual(parsed2.other, 'false');
});

test(function booleanParsingTrue() {
    const parsed = parseArgs(['--boool=true'], {
        default: {
            boool: false
        },
        boolean: ['boool']
    });

    assertEqual(parsed.boool, true);
});

test(function booleanParsingFalse() {
    const parsed = parseArgs(['--boool=false'], {
        default: {
          boool: true
        },
        boolean: ['boool']
    });

    assertEqual(parsed.boool, false);
});
