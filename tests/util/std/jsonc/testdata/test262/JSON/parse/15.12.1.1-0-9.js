// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// Copyright (c) 2012 Ecma International.  All rights reserved.
// This code is governed by the BSD license found in the LICENSE file.

/*---
es5id: 15.12.1.1-0-9
description: Whitespace characters can appear before/after any JSONtoken
---*/

JSON.parse(
  "\t\r \n{\t\r \n" +
    '"property"\t\r \n:\t\r \n{\t\r \n}\t\r \n,\t\r \n' +
    '"prop2"\t\r \n:\t\r \n' +
    "[\t\r \ntrue\t\r \n,\t\r \nnull\t\r \n,123.456\t\r \n]" +
    "\t\r \n}\t\r \n",
); // should JSON parse without error
