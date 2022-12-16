import type { Fizzbuzz } from "npm:@denotest/types";
import type { SomeInterface } from "npm:@denotest/types_imported";
import type { Foobar as FooInterface } from "npm:@denotest/types_imported/subpath";
import { getClient } from "npm:@denotest/types-exports-subpaths/client";
import { entryB } from "npm:@denotest/types-exports-subpaths/entry-b";
import { entryC } from "npm:@denotest/types-exports-subpaths/entry-c";

const foobar: FooInterface = {
  foo: "foo",
  bar: 1,
};

const i: SomeInterface = {
  prop: 1,
  prop2: "asdf",
};

const fizzbuzz: Fizzbuzz = {
  fizz: 1,
  buzz: 2,
};

const valueA: "test1" = getClient();
const valueB: "test2" = entryB();
const valueC: "test3" = entryC();
