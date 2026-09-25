// "@test/add" resolves via this workspace member's import map scope and
// "@denotest/esm-basic" via the root package.json dependencies. Both lookups
// prefix-match this module's URL, so they break if a lowercased drive letter
// in the dynamic import of this file is not normalized.
import { add } from "@test/add";
import { getValue, setValue } from "@denotest/esm-basic";

setValue(add(2, 3));
console.log(getValue());
