// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// Copyright (c) 2012 Ecma International.  All rights reserved.
// This code is governed by the BSD license found in the LICENSE file.

/*---
es5id: 15.12.2-2-4
description: >
    JSON.parse - parsing an object where property name starts and ends
    with a null character
---*/

var nullChars = new Array();
nullChars[0] = '"\u0000"';
nullChars[1] = '"\u0001"';
nullChars[2] = '"\u0002"';
nullChars[3] = '"\u0003"';
nullChars[4] = '"\u0004"';
nullChars[5] = '"\u0005"';
nullChars[6] = '"\u0006"';
nullChars[7] = '"\u0007"';
nullChars[8] = '"\u0008"';
nullChars[9] = '"\u0009"';
nullChars[10] = '"\u000A"';
nullChars[11] = '"\u000B"';
nullChars[12] = '"\u000C"';
nullChars[13] = '"\u000D"';
nullChars[14] = '"\u000E"';
nullChars[15] = '"\u000F"';
nullChars[16] = '"\u0010"';
nullChars[17] = '"\u0011"';
nullChars[18] = '"\u0012"';
nullChars[19] = '"\u0013"';
nullChars[20] = '"\u0014"';
nullChars[21] = '"\u0015"';
nullChars[22] = '"\u0016"';
nullChars[23] = '"\u0017"';
nullChars[24] = '"\u0018"';
nullChars[25] = '"\u0019"';
nullChars[26] = '"\u001A"';
nullChars[27] = '"\u001B"';
nullChars[28] = '"\u001C"';
nullChars[29] = '"\u001D"';
nullChars[30] = '"\u001E"';
nullChars[31] = '"\u001F"';

for (var index in nullChars) {
  assert.throws(SyntaxError, function () {
    var obj = JSON.parse(
      "{" + nullChars[index] + "name" + nullChars[index] + ' : "John" } ',
    );
  });
}
