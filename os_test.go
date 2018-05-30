// Copyright 2018 Ryan Dahl <ry@tinyclouds.org>
// All rights reserved. MIT License.
package deno

import (
	"path"
	"testing"
)

func AssertEqual(t *testing.T, actual string, expected string) {
	if actual != expected {
		t.Fatalf("not equal <<%s>> <<%s>>", actual, expected)
	}
}

func TestResolveModule1(t *testing.T) {
	createDirs()
	moduleName, filename, err := ResolveModule(
		"http://localhost:4545/testdata/subdir/print_hello.ts",
		"/Users/rld/go/src/github.com/ry/deno/testdata/006_url_imports.ts")
	if err != nil {
		t.Fatalf(err.Error())
	}
	AssertEqual(t, moduleName,
		"http://localhost:4545/testdata/subdir/print_hello.ts")
	AssertEqual(t, filename,
		path.Join(SrcDir, "localhost:4545/testdata/subdir/print_hello.ts"))
}

func TestResolveModule2(t *testing.T) {
	createDirs()
	moduleName, filename, err := ResolveModule(
		"./subdir/print_hello.ts",
		"/Users/rld/go/src/github.com/ry/deno/testdata/006_url_imports.ts")
	if err != nil {
		t.Fatalf(err.Error())
	}
	AssertEqual(t, moduleName,
		"/Users/rld/go/src/github.com/ry/deno/testdata/subdir/print_hello.ts")
	AssertEqual(t, filename,
		"/Users/rld/go/src/github.com/ry/deno/testdata/subdir/print_hello.ts")
}

func TestResolveModule3(t *testing.T) {
	createDirs()
	// In the case where the containingFile is a directory (indicated with a
	// trailing slash)
	moduleName, filename, err := ResolveModule(
		"testdata/001_hello.js",
		"/Users/rld/go/src/github.com/ry/deno/")
	if err != nil {
		t.Fatalf(err.Error())
	}
	AssertEqual(t, moduleName,
		"/Users/rld/go/src/github.com/ry/deno/testdata/001_hello.js")
	AssertEqual(t, filename,
		"/Users/rld/go/src/github.com/ry/deno/testdata/001_hello.js")
}

func TestResolveModule4(t *testing.T) {
	createDirs()
	// Files in SrcDir should resolve to URLs.
	moduleName, filename, err := ResolveModule(
		path.Join(SrcDir, "unpkg.com/liltest@0.0.5/index.ts"),
		".")
	if err != nil {
		t.Fatalf(err.Error())
	}
	AssertEqual(t, moduleName,
		"http://unpkg.com/liltest@0.0.5/index.ts")
	AssertEqual(t, filename,
		path.Join(SrcDir, "unpkg.com/liltest@0.0.5/index.ts"))
}

func TestResolveModuleExtensionsAintSpecial(t *testing.T) {
	createDirs()
	moduleName, filename, err := ResolveModule(
		"./util",
		path.Join(SrcDir, "unpkg.com/liltest@0.0.5/index.ts"))
	if err != nil {
		t.Fatalf(err.Error())
	}
	AssertEqual(t, moduleName,
		"http://unpkg.com/liltest@0.0.5/util")
	AssertEqual(t, filename,
		path.Join(SrcDir, "unpkg.com/liltest@0.0.5/util"))
}
