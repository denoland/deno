// `Foo` is imported type-only and then re-exported without the `type`
// modifier. Under verbatimModuleSyntax the type-only import is elided but
// the `export { Foo }` is kept as a value export, leaving it dangling.
import { type Foo } from "./types.ts";
export { Foo };
export const value = 1;
