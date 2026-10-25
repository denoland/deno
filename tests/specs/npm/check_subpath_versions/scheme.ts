import {
  value as old,
  type VersionMarker as Old,
} from "npm:@denotest/subpath-versions/feature";
import {
  value as newer,
  type VersionMarker as New,
} from "npm:@denotest/subpath-versions@2.0.0/feature";
const oldMarker: Old = { version: "v1" };
const newMarker: New = { version: "v2" };
const oldValue: "v1" = old;
const newValue: "v2" = newer;
