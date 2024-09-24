// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
// deno-lint-ignore-file
// Only for testing types. Invoke with `deno install --entrypoint`

const remote = Deno.dlopen("dummy_lib.so", {
  method1: { parameters: ["usize", "bool"], result: "void" },
  method2: { parameters: [], result: "void" },
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
    parameters: ["pointer"],
    result: "void",
  },
  method21: {
    parameters: ["pointer"],
    result: "void",
  },
  method22: {
    parameters: ["pointer"],
    result: "void",
  },
  method23: {
    parameters: ["buffer"],
    result: "void",
  },
  method24: {
    parameters: ["bool"],
    result: "bool",
  },
  method25: {
    parameters: [],
    result: "void",
    optional: true,
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
  static15: { type: "bool" },
  static16: {
    type: "bool",
    optional: true,
  },
});

Deno.dlopen("dummy_lib_2.so", {
  wrong_method1: {
    parameters: [],
    result: "function",
  },
});

// @ts-expect-error: Invalid argument
remote.symbols.method1(0);
// @ts-expect-error: Invalid argument
remote.symbols.method1(0, 0);
// @ts-expect-error: Invalid argument
remote.symbols.method1(true, true);
// @ts-expect-error: Invalid return type
<number> remote.symbols.method1(0, true);
<void> remote.symbols.method1(0n, true);

// @ts-expect-error: Expected 0 arguments, but got 1.
remote.symbols.method2(null);
remote.symbols.method2();

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
remote.symbols.method15("foo");
// @ts-expect-error: Invalid argument
remote.symbols.method15(new Uint16Array(1));
remote.symbols.method15(null);
remote.symbols.method15({} as Deno.PointerValue);

const result = remote.symbols.method16();
// @ts-expect-error: Invalid argument
let r_0: number = result;
let r_1: bigint = result;

const result2 = remote.symbols.method17();
// @ts-expect-error: Invalid argument
result2.then((_0: string) => {});
result2.then((_1: bigint) => {});

const result3 = remote.symbols.method18();
// @ts-expect-error: Invalid argument
let r3_0: Deno.BufferSource = result3;
let r3_1: null | Deno.UnsafePointer = result3;

const result4 = remote.symbols.method19();
// @ts-expect-error: Invalid argument
result4.then((_0: Deno.BufferSource) => {});
result4.then((_1: null | Deno.UnsafePointer) => {});

const fnptr = new Deno.UnsafeFnPointer(
  {} as Deno.PointerObject<Deno.ForeignFunction<["u32", "pointer"], "void">>,
  {
    parameters: ["u32", "pointer"],
    result: "void",
  },
);
// @ts-expect-error: Invalid argument
fnptr.call(null, null);
fnptr.call(0, null);

const unsafe_callback_wrong1 = new Deno.UnsafeCallback(
  {
    parameters: ["i8"],
    result: "void",
  },
  // @ts-expect-error: i8 is not a pointer
  (_: bigint) => {},
);
const unsafe_callback_wrong2 = new Deno.UnsafeCallback(
  {
    parameters: ["pointer"],
    result: "u64",
  },
  // @ts-expect-error: must return a number or bigint
  (_: Deno.UnsafePointer) => {},
);
const unsafe_callback_wrong3 = new Deno.UnsafeCallback(
  {
    parameters: [],
    result: "void",
  },
  // @ts-expect-error: no parameters
  (_: Deno.UnsafePointer) => {},
);
const unsafe_callback_wrong4 = new Deno.UnsafeCallback(
  {
    parameters: ["u64"],
    result: "void",
  },
  // @ts-expect-error: Callback's 64bit parameters are either number or bigint
  (_: number) => {},
);
const unsafe_callback_right1 = new Deno.UnsafeCallback(
  {
    parameters: ["u8", "u32", "pointer"],
    result: "void",
  },
  (_1: number, _2: number, _3: null | Deno.PointerValue) => {},
);
const unsafe_callback_right2 = new Deno.UnsafeCallback(
  {
    parameters: [],
    result: "u8",
  },
  () => 3,
);
const unsafe_callback_right3 = new Deno.UnsafeCallback(
  {
    parameters: [],
    result: "function",
  },
  // Callbacks can return other callbacks' pointers, if really wanted.
  () => unsafe_callback_right2.pointer,
);
const unsafe_callback_right4 = new Deno.UnsafeCallback(
  {
    parameters: ["u8", "u32", "pointer"],
    result: "u8",
  },
  (_1: number, _2: number, _3: null | Deno.PointerValue) => 3,
);
const unsafe_callback_right5 = new Deno.UnsafeCallback(
  {
    parameters: ["u8", "i32", "pointer"],
    result: "void",
  },
  (_1: number, _2: number, _3: null | Deno.PointerValue) => {},
);

// @ts-expect-error: Must pass callback
remote.symbols.method20();
// nullptr is okay
remote.symbols.method20(null);
// @ts-expect-error: Callback cannot be passed directly
remote.symbols.method20(unsafe_callback_right2);
remote.symbols.method20(unsafe_callback_right1.pointer);

remote.symbols.method23(new Uint8Array(1));
remote.symbols.method23(new Uint32Array(1));
remote.symbols.method23(new Uint8Array(1));

// @ts-expect-error: Cannot pass pointer values as buffer.
remote.symbols.method23({});
// @ts-expect-error: Cannot pass pointer values as buffer.
remote.symbols.method23({});
remote.symbols.method23(null);

// @ts-expect-error: Cannot pass number as bool.
remote.symbols.method24(0);
// @ts-expect-error: Cannot pass number as bool.
remote.symbols.method24(1);
// @ts-expect-error: Cannot pass null as bool.
remote.symbols.method24(null);
remote.symbols.method24(true);
remote.symbols.method24(false);
// @ts-expect-error: Cannot assert return type as a number.
<number> remote.symbols.method24(true);
// @ts-expect-error: Cannot assert return type truthiness.
let r24_0: true = remote.symbols.method24(true);
// @ts-expect-error: Cannot assert return type as a number.
let r42_1: number = remote.symbols.method24(true);
<boolean> remote.symbols.method24(Math.random() > 0.5);

// @ts-expect-error: Optional symbol; can be null.
remote.symbols.method25();

// @ts-expect-error: Invalid member type
const static1_wrong: number = remote.symbols.static1;
const static1_right: bigint = remote.symbols.static1;
// @ts-expect-error: Invalid member type
const static2_wrong: null = remote.symbols.static2;
const static2_right: null | Deno.UnsafePointer = remote.symbols.static2;
// @ts-expect-error: Invalid member type
const static3_wrong: number = remote.symbols.static3;
const static3_right: bigint = remote.symbols.static3;
// @ts-expect-error: Invalid member type
const static4_wrong: number = remote.symbols.static4;
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
const static8_wrong: number = remote.symbols.static8;
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
const static12_wrong: number = remote.symbols.static12;
const static12_right: bigint = remote.symbols.static12;
// @ts-expect-error: Invalid member type
const static13_wrong: null = remote.symbols.static13;
const static13_right: number = remote.symbols.static13;
// @ts-expect-error: Invalid member type
const static14_wrong: null = remote.symbols.static14;
const static14_right: number = remote.symbols.static14;
// @ts-expect-error: Invalid member type
const static15_wrong: number = remote.symbols.static15;
const static15_right: boolean = remote.symbols.static15;
// @ts-expect-error: Invalid member type
const static16_wrong: boolean = remote.symbols.static16;
const static16_right: boolean | null = remote.symbols.static16;

// Adapted from https://stackoverflow.com/a/53808212/10873797
type Equal<T, U> = (<G>() => G extends T ? 1 : 2) extends
  <G>() => G extends U ? 1 : 2 ? true
  : false;

type AssertEqual<
  Expected extends $,
  Got extends $$,
  $ = [Equal<Got, Expected>] extends [true] ? Expected
    : [Expected] extends [Got] ? never
    : Got,
  $$ = [Equal<Expected, Got>] extends [true] ? Got
    : [Got] extends [Expected] ? never
    : Got,
> = never;

type AssertNotEqual<
  Expected extends $,
  Got,
  $ = [Equal<Expected, Got>] extends [true] ? never : Expected,
> = never;

const enum FooEnum {
  Foo,
  Bar,
}
const foo = "u8" as Deno.NativeU8Enum<FooEnum>;

declare const brand: unique symbol;
class MyPointerClass {}
type MyPointer = Deno.PointerObject & { [brand]: MyPointerClass };
const myPointer = "pointer" as Deno.NativeTypedPointer<MyPointer>;
type MyFunctionDefinition = Deno.UnsafeCallbackDefinition<
  [typeof foo, "u32"],
  typeof myPointer
>;
const myFunction = "function" as Deno.NativeTypedFunction<MyFunctionDefinition>;

type __Tests__ = [
  empty: AssertEqual<
    { symbols: Record<never, never>; close(): void },
    Deno.DynamicLibrary<Record<never, never>>
  >,
  basic: AssertEqual<
    { symbols: { add: (n1: number, n2: number) => number }; close(): void },
    Deno.DynamicLibrary<{ add: { parameters: ["i32", "u8"]; result: "i32" } }>
  >,
  higher_order_params: AssertEqual<
    {
      symbols: {
        pushBuf: (
          buf: BufferSource | null,
          ptr: Deno.PointerValue | null,
          func: Deno.PointerValue | null,
        ) => void;
      };
      close(): void;
    },
    Deno.DynamicLibrary<{
      pushBuf: {
        parameters: ["buffer", "pointer", "function"];
        result: "void";
      };
    }>
  >,
  higher_order_returns: AssertEqual<
    {
      symbols: {
        pushBuf: (
          buf: BufferSource | null,
          ptr: Deno.PointerValue,
          func: Deno.PointerValue,
        ) => Deno.PointerValue;
      };
      close(): void;
    },
    Deno.DynamicLibrary<{
      pushBuf: {
        parameters: ["buffer", "pointer", "function"];
        result: "pointer";
      };
    }>
  >,
  non_exact_params: AssertEqual<
    {
      symbols: {
        foo: (...args: (number | Deno.PointerValue | null)[]) => bigint;
      };
      close(): void;
    },
    Deno.DynamicLibrary<{
      foo: { parameters: ("i32" | "pointer")[]; result: "u64" };
    }>
  >,
  non_exact_params_empty: AssertEqual<
    {
      symbols: {
        foo: () => number;
      };
      close(): void;
    },
    Deno.DynamicLibrary<{ foo: { parameters: []; result: "i32" } }>
  >,
  non_exact_params_empty: AssertNotEqual<
    {
      symbols: {
        foo: (a: number) => number;
      };
      close(): void;
    },
    Deno.DynamicLibrary<{ foo: { parameters: []; result: "i32" } }>
  >,
  enum_param: AssertEqual<
    {
      symbols: {
        foo: (arg: FooEnum) => void;
      };
      close(): void;
    },
    Deno.DynamicLibrary<{ foo: { parameters: [typeof foo]; result: "void" } }>
  >,
  enum_return: AssertEqual<
    {
      symbols: {
        foo: () => FooEnum;
      };
      close(): void;
    },
    Deno.DynamicLibrary<{ foo: { parameters: []; result: typeof foo } }>
  >,
  typed_pointer_param: AssertEqual<
    {
      symbols: {
        foo: (arg: MyPointer | null) => void;
      };
      close(): void;
    },
    Deno.DynamicLibrary<{
      foo: { parameters: [typeof myPointer]; result: "void" };
    }>
  >,
  typed_pointer_return: AssertEqual<
    {
      symbols: {
        foo: () => MyPointer | null;
      };
      close(): void;
    },
    Deno.DynamicLibrary<{ foo: { parameters: []; result: typeof myPointer } }>
  >,
  typed_function_param: AssertEqual<
    {
      symbols: {
        foo: (arg: Deno.PointerObject<MyFunctionDefinition> | null) => void;
      };
      close(): void;
    },
    Deno.DynamicLibrary<{
      foo: { parameters: [typeof myFunction]; result: "void" };
    }>
  >,
  typed_function_return: AssertEqual<
    {
      symbols: {
        foo: () => Deno.PointerObject<MyFunctionDefinition> | null;
      };
      close(): void;
    },
    Deno.DynamicLibrary<{ foo: { parameters: []; result: typeof myFunction } }>
  >,
];
