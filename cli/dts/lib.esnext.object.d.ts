// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

/// <reference no-default-lib="true"/>

interface ObjectConstructor {
  /**
   * Determines whether an object has a property with the specified name.
   * @param o The target object.
   * @param v A property name.
   */
  hasOwn(o: object, v: PropertyKey): boolean;
}
