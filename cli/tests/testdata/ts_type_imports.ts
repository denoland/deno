// deno-lint-ignore-file

type Foo = import("./ts_type_imports_foo.ts").Foo;

const foo: Foo = new Map<string, string>();
