function logThing(thing: import("./subdir/mod3").Thing) {
    console.log(`a: ${thing.a}, b: ${thing.b}`);
}

logThing({
    a: "foo",
    b: 1
});
