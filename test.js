const TESTS = [];

async function runTests() {
    for (const {name, fn} of TESTS) {
        console.log("test ", name);
        await fn();
    }
    console.log("done testing");
}

function test(def) {
    TESTS.push(def);
}

test({
    name: "a",
    fn: function () {
        console.log("hello a");
        return new Promise((_resolve, _reject) => {
            console.log("asdf");
        });
    }
});

test({
    name: "b",
    fn: function () {
        console.log("hello b");
    }
});

function foo() {
    return new Promise((_r, _rj) => {
        console.log("hello from clunky promise");
    });
}

await foo();
await runTests();