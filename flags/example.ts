// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { args } from "deno";
import { parse } from "./mod.ts";

console.dir(parse(args));
