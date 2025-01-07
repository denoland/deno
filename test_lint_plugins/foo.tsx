// Copyright 2018-2025 the Deno authors. MIT license.

const a = <div style={{}} />;
const b = <div style="foo" />;
const c: Foo<number> = {
  node: 2,
};

interface Foo<T> {
  node: T;
}
