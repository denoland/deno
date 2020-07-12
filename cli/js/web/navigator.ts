// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

import { PermissionsImpl as Permissions } from "./permissions.ts";

export class NavigatorImpl implements Navigator {
  permissions = new Permissions();
}

Object.defineProperty(NavigatorImpl, "name", {
  value: "Navigator",
  configurable: true,
});
