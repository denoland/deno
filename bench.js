const lib = Deno.dlopen("./noop.so", {
    noop: {
        parameters: [],
        result: "void",
    },
}).symbols;

Deno.bench("ffi noop", () => {
    lib.noop();
});

const noop = () => {};

Deno.bench("js noop", () => {
    noop();
});
