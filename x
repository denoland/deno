#!/usr/bin/env -S deno run --allow-all --ext=ts
// Copyright 2018-2026 the Deno authors. MIT license.

/**
 * x - Developer CLI for contributing to Deno
 *
 * Inspired by Servo's mach tool, this script provides a unified
 * interface for common development tasks like building, testing, and more.
 *
 * Usage:
 *   ./x <command> [options]
 *
 * Run `./x --help` for more information.
 */

import { main } from "./tools/x/main.ts";

await main(import.meta.dirname!);
