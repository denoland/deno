import {
  type DeclarationMarker as Root,
  value as root,
} from "npm:@denotest/subpath-declarations@1.0.0";
import {
  type DeclarationMarker as String,
  value as string,
} from "npm:@denotest/subpath-declarations@1.0.0/string";
import {
  type DeclarationMarker as Default,
  value as defaultExport,
} from "npm:@denotest/subpath-declarations@1.0.0/default";
import {
  type DeclarationMarker as Conditional,
  value as conditional,
} from "npm:@denotest/subpath-declarations@1.0.0/conditional";
import {
  type DeclarationMarker as Explicit,
  value as explicit,
} from "npm:@denotest/subpath-declarations@1.0.0/explicit";
import {
  type DeclarationMarker as Mjs,
  value as mjs,
} from "npm:@denotest/subpath-declarations@1.0.0/mjs";
import {
  type DeclarationMarker as Cjs,
  value as cjs,
} from "npm:@denotest/subpath-declarations@1.0.0/cjs";
import {
  type DeclarationMarker as Wildcard,
  value as wildcard,
} from "npm:@denotest/subpath-declarations@1.0.0/wild/foo";
import {
  type DeclarationMarker as Deep,
  value as deep,
} from "npm:@denotest/subpath-declarations@1.0.0/wild/deep/foo";
import {
  type DeclarationMarker as Exact,
  value as exact,
} from "npm:@denotest/subpath-declarations@1.0.0/wild/exact";
import { type DeclarationMarker as Alias, value as aliased } from "entry";
import {
  type DeclarationMarker as Prefix,
  value as prefixed,
} from "fixture/default";
const aliasMarker: Alias = { kind: "typed" };
const prefixMarker: Prefix = { kind: "typed" };
const aliasValue: "typed" = aliased;
const prefixValue: "typed" = prefixed;
const rootMarker: Root = { kind: "typed" };
const stringMarker: String = { kind: "typed" };
const defaultMarker: Default = { kind: "typed" };
const conditionalMarker: Conditional = { kind: "typed" };
const explicitMarker: Explicit = { kind: "typed" };
const mjsMarker: Mjs = { kind: "mts" };
const cjsMarker: Cjs = { kind: "cts" };
const wildcardMarker: Wildcard = { kind: "typed" };
const deepMarker: Deep = { kind: "deep" };
const exactMarker: Exact = { kind: "exact" };
const rootValue: "typed" = root;
const stringValue: "typed" = string;
const defaultValue: "typed" = defaultExport;
const conditionalValue: "typed" = conditional;
const explicitValue: "typed" = explicit;
const mjsValue: "typed" = mjs;
const cjsValue: "typed" = cjs;
const wildcardValue: "typed" = wildcard;
const deepValue: "typed" = deep;
const exactValue: "typed" = exact;
console.log(
  rootValue,
  stringValue,
  defaultValue,
  conditionalValue,
  explicitValue,
  mjsValue,
  cjsValue,
  wildcardValue,
  deepValue,
  exactValue,
);
