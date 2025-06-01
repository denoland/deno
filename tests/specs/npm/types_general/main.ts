import type { Fizzbuzz } from "npm:@denotest/types";
import type { SomeInterface } from "npm:@denotest/types_imported";
import type { Foobar as FooInterface } from "npm:@denotest/types_imported/subpath";
import { getClient } from "npm:@denotest/types-exports-subpaths/client";
import { entryImport } from "npm:@denotest/types-exports-subpaths/entry-import";
import { entryA } from "npm:@denotest/types-exports-subpaths/entry-a";
import { entryTypes } from "npm:@denotest/types-exports-subpaths/entry-types-last-no-declaration-before";

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
const valueB: "test2" = entryImport();
const valueC: "test3" = entryA();
const valueD: "test4" = entryTypes();
