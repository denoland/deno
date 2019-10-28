// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.

import { evaluate, instantiate, load } from "./utils.ts";

const args = Deno.args;
const text = await load(args);
const result = evaluate(text);
instantiate(...result);
