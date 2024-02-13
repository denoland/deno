// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// This module is browser compatible.

import { stringify } from "../stringify.ts";

console.log(
  stringify({
    foo: {
      bar: true,
      test: [
        "a",
        "b",
        {
          a: false,
        },
        {
          a: false,
        },
      ],
    },
    test: "foobar",
  }),
);
