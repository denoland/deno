// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
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
    method20: {
      parameters: [{
        function: { parameters: ["u8", "u32", "pointer"], result: "void" },
      }],
      result: "void",
    },
    method21: {
      parameters: [
        { function: { parameters: [], result: "u8" } },
      ],
      result: "void",
    },
    method22: {
      parameters: [{
        function: {
          parameters: [],
          result: { function: { parameters: [], result: "u8" } },
        },
      }],
      result: "void",
    },
    static1: { type: "usize" },
    static2: { type: "pointer" },
    static3: { type: "usize" },
    static4: { type: "isize" },
    static5: { type: "u8" },
    static6: { type: "u16" },
    static7: { type: "u32" },
    static8: { type: "u64" },
    static9: { type: "i8" },
    static10: { type: "i16" },
    static11: { type: "i32" },
    static12: { type: "i64" },
    static13: { type: "f32" },
    static14: { type: "f64" },
  } as const,
);

Deno.dlopen(
  "dummy_lib_2.so",
  // @ts-expect-error: Returning a function pointer
  // is declared using "pointer" + UnsafeFnPointer
  {
    wrong_method1: {
      parameters: [],
      result: {
        function: {
          parameters: [],
          result: "void",
        },
      },
    },
  } as const,
);

// @ts-expect-error: Invalid argument
remote.symbols.method1(0);
// @ts-expect-error: Invalid return type
<number> remote.symbols.method1(0, 0);
<void> remote.symbols.method1(0n, 0n);

// @ts-expect-error: Invalid argument
remote.symbols.method2(null);
remote.symbols.method2(void 0);

// @ts-expect-error: Invalid argument
remote.symbols.method3(null);
remote.symbols.method3(0n);

// @ts-expect-error: Invalid argument
remote.symbols.method4(null);
remote.symbols.method4(0n);

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
remote.symbols.method8(0n);

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
remote.symbols.method12(0n);

// @ts-expect-error: Invalid argument
remote.symbols.method13(null);
remote.symbols.method13(0);

// @ts-expect-error: Invalid argument
remote.symbols.method14(null);
remote.symbols.method14(0);

// @ts-expect-error: Invalid argument
remote.symbols.method15(0);
remote.symbols.method15(new Uint16Array(1));
remote.symbols.method15({} as Deno.UnsafePointer);

const result = remote.symbols.method16();
// @ts-expect-error: Invalid argument
let r_0: string = result;
let r_1: number | bigint = result;

const result2 = remote.symbols.method17();
// @ts-expect-error: Invalid argument
result2.then((_0: string) => {});
result2.then((_1: number | bigint) => {});

const result3 = remote.symbols.method18();
// @ts-expect-error: Invalid argument
let r3_0: Deno.TypedArray = result3;
let r3_1: Deno.UnsafePointer = result3;

const result4 = remote.symbols.method19();
// @ts-expect-error: Invalid argument
result4.then((_0: Deno.TypedArray) => {});
result4.then((_1: Deno.UnsafePointer) => {});

const ptr = new Deno.UnsafePointer(0n);
const fnptr = new Deno.UnsafeFnPointer(
  ptr,
  {
    parameters: ["u32", "pointer"],
    result: "void",
  } as const,
);
// @ts-expect-error: Invalid argument
fnptr.call(null, null);
fnptr.call(0, null);

const unsafe_callback_wrong1 = new Deno.UnsafeCallback(
  {
    parameters: ["i8"],
    result: "void",
  } as const,
  // @ts-expect-error: i8 is not a pointer
  (_: Deno.UnsafePointer) => {},
);
const unsafe_callback_wrong2 = new Deno.UnsafeCallback(
  {
    parameters: ["pointer"],
    result: "u64",
  } as const,
  // @ts-expect-error: must return a number or bigint
  (_: Deno.UnsafePointer) => {},
);
const unsafe_callback_wrong3 = new Deno.UnsafeCallback(
  {
    parameters: [],
    result: "void",
  } as const,
  // @ts-expect-error: no parameters
  (_: Deno.UnsafePointer) => {},
);
const unsafe_callback_wrong4 = new Deno.UnsafeCallback(
  {
    parameters: ["u64"],
    result: "void",
  } as const,
  // @ts-expect-error: Callback's 64bit parameters are always called as bigint
  (_: number) => {},
);
const unsafe_callback_right1 = new Deno.UnsafeCallback(
  {
    parameters: ["u8", "u32", "pointer"],
    result: "void",
  } as const,
  (_1: number, _2: number, _3: Deno.UnsafePointer) => {},
);
const unsafe_callback_right2 = new Deno.UnsafeCallback(
  {
    parameters: [],
    result: "u8",
  } as const,
  () => 3,
);
const unsafe_callback_right3 = new Deno.UnsafeCallback(
  {
    parameters: [],
    result: {
      function: {
        parameters: [],
        result: "u8",
      },
    },
  } as const,
  // Callbacks can return other callbacks, if really wanted.
  () => unsafe_callback_right2,
);
const unsafe_callback_right4 = new Deno.UnsafeCallback(
  {
    parameters: ["u8", "u32", "pointer"],
    result: "u8",
  } as const,
  (_1: number, _2: number, _3: Deno.UnsafePointer) => 3,
);
const unsafe_callback_right5 = new Deno.UnsafeCallback(
  {
    parameters: ["u8", "i32", "pointer"],
    result: "void",
  } as const,
  (_1: number, _2: number, _3: Deno.UnsafePointer) => {},
);

// @ts-expect-error: Must pass callback
remote.symbols.method20();
// nullptr is okay
remote.symbols.method20(null);
// Foreign function ptr received as UnsafePointer is okay
remote.symbols.method20({} as Deno.UnsafePointer);
// @ts-expect-error: Callback does not match the parameter
remote.symbols.method20(unsafe_callback_right2);
remote.symbols.method20(unsafe_callback_right1);
// @ts-expect-error: Callback must match return value as well
remote.symbols.method20(unsafe_callback_right4);
// @ts-expect-error: Subtle differences in parameter types are not allowed (i32 vs u32)
remote.symbols.method20(unsafe_callback_right5);
remote.symbols.method21(unsafe_callback_right2);
remote.symbols.method22(unsafe_callback_right3);
// @ts-expect-error: Callback returns a callback with the wrong return value
remote.symbols.method21(unsafe_callback_right3);
// @ts-expect-error: Callback returns a callback with the wrong return value
remote.symbols.method22(unsafe_callback_right2);

// @ts-expect-error: Invalid member type
const static1_wrong: null = remote.symbols.static1;
const static1_right: bigint = remote.symbols.static1;
// @ts-expect-error: Invalid member type
const static2_wrong: null = remote.symbols.static2;
const static2_right: Deno.UnsafePointer = remote.symbols.static2;
// @ts-expect-error: Invalid member type
const static3_wrong: null = remote.symbols.static3;
const static3_right: bigint = remote.symbols.static3;
// @ts-expect-error: Invalid member type
const static4_wrong: null = remote.symbols.static4;
const static4_right: bigint = remote.symbols.static4;
// @ts-expect-error: Invalid member type
const static5_wrong: null = remote.symbols.static5;
const static5_right: number = remote.symbols.static5;
// @ts-expect-error: Invalid member type
const static6_wrong: null = remote.symbols.static6;
const static6_right: number = remote.symbols.static6;
// @ts-expect-error: Invalid member type
const static7_wrong: null = remote.symbols.static7;
const static7_right: number = remote.symbols.static7;
// @ts-expect-error: Invalid member type
const static8_wrong: null = remote.symbols.static8;
const static8_right: bigint = remote.symbols.static8;
// @ts-expect-error: Invalid member type
const static9_wrong: null = remote.symbols.static9;
const static9_right: number = remote.symbols.static9;
// @ts-expect-error: Invalid member type
const static10_wrong: null = remote.symbols.static10;
const static10_right: number = remote.symbols.static10;
// @ts-expect-error: Invalid member type
const static11_wrong: null = remote.symbols.static11;
const static11_right: number = remote.symbols.static11;
// @ts-expect-error: Invalid member type
const static12_wrong: null = remote.symbols.static12;
const static12_right: bigint = remote.symbols.static12;
// @ts-expect-error: Invalid member type
const static13_wrong: null = remote.symbols.static13;
const static13_right: number = remote.symbols.static13;
// @ts-expect-error: Invalid member type
const static14_wrong: null = remote.symbols.static14;
const static14_right: number = remote.symbols.static14;
