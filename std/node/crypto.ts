// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { default as randomBytes } from "./_crypto/randomBytes.ts";
import { pbkdf2, pbkdf2Sync } from "./_crypto/pbkdf2.ts";

export default { randomBytes, pbkdf2, pbkdf2Sync };
export { pbkdf2, pbkdf2Sync, randomBytes };
