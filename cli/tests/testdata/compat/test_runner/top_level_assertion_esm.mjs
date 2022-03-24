import assert, { strictEqual } from "assert";

assert(!import.meta.main, "The module was loaded as a main module");
strictEqual(20, 20);
