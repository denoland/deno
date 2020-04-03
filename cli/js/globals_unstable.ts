// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

import { readOnly } from "./globals.ts";

export const unstableGlobalMethods = {};

export const unstableGlobalProperties = {
  __unstable: readOnly(true),
};
