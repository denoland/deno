#!/usr/bin/env node

import { spawnInSubprocess } from "./index.js";

spawnInSubprocess(process.argv.slice(2));