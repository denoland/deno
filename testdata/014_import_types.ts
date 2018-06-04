function logThing(thing: import("./subdir/mod3.d.ts").Thing) {
    console.log(`a: ${thing.a}, b: ${thing.b}`);
}

logThing({
    a: "foo",
    b: 1
});
