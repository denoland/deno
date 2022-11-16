import type { Fizzbuzz } from "npm:@denotest/types";
import type { SomeInterface } from "npm:@denotest/types_imported";
import type { Foobar as FooInterface } from "npm:@denotest/types_imported/subpath";

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
