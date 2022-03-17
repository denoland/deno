// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

// ** Internal Interfaces **

interface Key {
  type: "number" | "date" | "string" | "binary" | "array";
  // deno-lint-ignore no-explicit-any
  value: any;
}

type TransactionState = "active" | "inactive" | "committing" | "finished";

interface KeyGenerator {
  current: number;
  generateKey(): number;
  possiblyUpdate(key: Key): void;
}
