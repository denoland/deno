import {
  value as v0,
  type VersionMarker as M0,
} from "npm:@denotest/subpath-versions@1.0.0/feature";
const marker0: M0 = { version: "v1" };
const expected0: "v1" = v0;
import {
  value as v1,
  type VersionMarker as M1,
} from "npm:@denotest/subpath-versions@2.0.0/feature";
const marker1: M1 = { version: "v2" };
const expected1: "v2" = v1;
import {
  value as v2,
  type VersionMarker as M2,
} from "npm:@denotest/subpath-versions/feature";
const marker2: M2 = { version: "v2" };
const expected2: "v2" = v2;
import { value as v3, type VersionMarker as M3 } from "newRoot";
const marker3: M3 = { version: "v2" };
const expected3: "v2" = v3;
import { value as v4, type VersionMarker as M4 } from "oldRoot";
const marker4: M4 = { version: "v1" };
const expected4: "v1" = v4;
import { value as v5, type VersionMarker as M5 } from "newRoot/feature";
const marker5: M5 = { version: "v2" };
const expected5: "v2" = v5;
import { value as v6, type VersionMarker as M6 } from "oldRoot/feature";
const marker6: M6 = { version: "v1" };
const expected6: "v1" = v6;
import { value as v7, type VersionMarker as M7 } from "shared/feature";
const marker7: M7 = { version: "v1" };
const expected7: "v1" = v7;
import {
  value as v8,
  type VersionMarker as M8,
} from "npm:@denotest/subpath-versions/untyped";
const marker8: M8 = { version: "v2" };
const expected8: "v2" = v8;
import { value as untyped } from "oldUntyped";
const anyValue: number = untyped;
console.log(
  expected0,
  expected1,
  expected2,
  expected3,
  expected4,
  expected5,
  expected6,
  expected7,
  expected8,
  anyValue,
);
