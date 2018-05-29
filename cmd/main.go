// Copyright 2018 Ryan Dahl <ry@tinyclouds.org>
// All rights reserved. MIT License.
package main

import (
	"github.com/ry/deno"
)

func main() {
	deno.Init()
	deno.Eval("deno_main.js", "denoMain()")
	deno.Loop()
}
