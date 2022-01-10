// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
// deno-lint-ignore-file
// Only for testing types. Invoke with `deno cache`

const remote = Deno.dlopen(
  "dummy_lib.so",
  {
    method1: { parameters: ["usize", "usize"], result: "void" },
    method2: { parameters: ["void"], result: "void" },
    method3: { parameters: ["usize"], result: "void" },
    method4: { parameters: ["isize"], result: "void" },
    method5: { parameters: ["u8"], result: "void" },
    method6: { parameters: ["u16"], result: "void" },
    method7: { parameters: ["u32"], result: "void" },
    method8: { parameters: ["u64"], result: "void" },
    method9: { parameters: ["i8"], result: "void" },
    method10: { parameters: ["i16"], result: "void" },
    method11: { parameters: ["i32"], result: "void" },
    method12: { parameters: ["i64"], result: "void" },
    method13: { parameters: ["f32"], result: "void" },
    method14: { parameters: ["f64"], result: "void" },
    method15: { parameters: ["pointer"], result: "void" },
    method16: { parameters: [], result: "usize" },
    method17: { parameters: [], result: "usize", nonblocking: true },
    method18: { parameters: [], result: "pointer" },
    method19: { parameters: [], result: "pointer", nonblocking: true },
  } as const,
);

// @ts-expect-error: Invalid argument
remote.symbols.method1(0);
// @ts-expect-error: Invalid return type
<number> remote.symbols.method1(0, 0);
<void> remote.symbols.method1(0, 0);

// @ts-expect-error: Invalid argument
remote.symbols.method2(null);
remote.symbols.method2(void 0);

// @ts-expect-error: Invalid argument
remote.symbols.method3(null);
remote.symbols.method3(0);

// @ts-expect-error: Invalid argument
remote.symbols.method4(null);
remote.symbols.method4(0);

// @ts-expect-error: Invalid argument
remote.symbols.method5(null);
remote.symbols.method5(0);

// @ts-expect-error: Invalid argument
remote.symbols.method6(null);
remote.symbols.method6(0);

// @ts-expect-error: Invalid argument
remote.symbols.method7(null);
remote.symbols.method7(0);

// @ts-expect-error: Invalid argument
remote.symbols.method8(null);
remote.symbols.method8(0);

// @ts-expect-error: Invalid argument
remote.symbols.method9(null);
remote.symbols.method9(0);

// @ts-expect-error: Invalid argument
remote.symbols.method10(null);
remote.symbols.method10(0);

// @ts-expect-error: Invalid argument
remote.symbols.method11(null);
remote.symbols.method11(0);

// @ts-expect-error: Invalid argument
remote.symbols.method12(null);
remote.symbols.method12(0);

// @ts-expect-error: Invalid argument
remote.symbols.method13(null);
remote.symbols.method13(0);

// @ts-expect-error: Invalid argument
remote.symbols.method14(null);
remote.symbols.method14(0);

// @ts-expect-error: Invalid argument
remote.symbols.method15(null);
remote.symbols.method15(new Uint16Array(1));
remote.symbols.method15({} as Deno.UnsafePointer);

const result = remote.symbols.method16();
// @ts-expect-error: Invalid argument
let r_0: string = result;
let r_1: number = result;

const result2 = remote.symbols.method17();
// @ts-expect-error: Invalid argument
result2.then((_0: string) => {});
result2.then((_1: number) => {});

const result3 = remote.symbols.method18();
// @ts-expect-error: Invalid argument
let r3_0: Deno.TypedArray = result3;
let r3_1: Deno.UnsafePointer = result3;

const result4 = remote.symbols.method19();
// @ts-expect-error: Invalid argument
result4.then((_0: Deno.TypedArray) => {});
result4.then((_1: Deno.UnsafePointer) => {});
