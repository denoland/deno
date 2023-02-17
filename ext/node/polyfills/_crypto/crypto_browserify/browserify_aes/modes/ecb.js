// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// Copyright 2014-2017 browserify-aes contributors. All rights reserved. MIT license.
// Copyright 2013 Maxwell Krohn. All rights reserved. MIT license.
// Copyright 2009-2013 Jeff Mott. All rights reserved. MIT license.

export const encrypt = function (self, block) {
  return self._cipher.encryptBlock(block);
};

export const decrypt = function (self, block) {
  return self._cipher.decryptBlock(block);
};
