import { value } from "npm:@denotest/subpath-versions/untyped";
import { value as typed } from "npm:@denotest/subpath-versions@2.0.0/untyped";
import { value as untyped } from "oldUntyped";
const typedValue: "v2" = typed;
const anyValue: number = untyped;
const wrong: number = value;
