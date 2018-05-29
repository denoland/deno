// Copyright 2018 Ryan Dahl <ry@tinyclouds.org>
// All rights reserved. MIT License.
package deno

import (
	"bytes"
	"io/ioutil"
	"net"
	"net/http"
	"os"
	"os/exec"
	"path"
	"strings"
	"testing"
)

var denoFn string

// Some tests require an HTTP server. We start one here.
// Note that "localhost:4545" is hardcoded into the tests at the moment,
// so if the server runs on a different port, it will fail.
func startServer() {
	l, err := net.Listen("tcp", ":4545")
	if err != nil {
		panic(err)
	}
	rootHandler := http.FileServer(http.Dir("."))
	go func() {
		if err := http.Serve(l, rootHandler); err != nil {
			panic(err)
		}
	}()
}

func listTestFiles() []string {
	files, err := ioutil.ReadDir("testdata")
	if err != nil {
		panic(err)
	}
	out := make([]string, 0)
	for _, file := range files {
		fn := file.Name()
		if strings.HasSuffix(fn, ".out") {
			out = append(out, fn)
		}
	}
	return out
}

func checkOutput(t *testing.T, outFile string) {
	outFile = path.Join("testdata", outFile)
	jsFile := strings.TrimSuffix(outFile, ".out")

	expected, err := ioutil.ReadFile(outFile)
	if err != nil {
		t.Fatal(err.Error())
	}

	actual, _, err := deno(jsFile)
	if err != nil {
		t.Fatal(err.Error())
	}
	if bytes.Compare(actual, expected) != 0 {
		t.Fatalf(`Actual output does not match expected.
-----Actual-------------------
%s-----Expected-----------------
%s------------------------------`, string(actual), string(expected))
	}
}

func deno(inputFn string) (actual []byte, cachedir string, err error) {
	cachedir, err = ioutil.TempDir("", "TestIntegration")
	if err != nil {
		panic(err)
	}

	cmd := exec.Command(denoFn, "--cachedir="+cachedir, inputFn)
	var out bytes.Buffer
	cmd.Stdout = &out
	err = cmd.Run()
	if err == nil {
		actual = out.Bytes()
	}
	return
}

func integrationTestSetup() {
	if denoFn == "" {
		startServer()
		cwd, err := os.Getwd()
		if err != nil {
			panic(err)
		}
		denoFn = path.Join(cwd, "deno")
	}
}

func TestIntegrationFiles(t *testing.T) {
	integrationTestSetup()
	outFiles := listTestFiles()
	for _, outFile := range outFiles {
		t.Run(outFile, func(t *testing.T) {
			checkOutput(t, outFile)
		})
	}
}

func TestIntegrationUrlArgs(t *testing.T) {
	integrationTestSetup()

	// Using good port 4545
	_, cachedir, err := deno("http://localhost:4545/testdata/001_hello.js")
	if err != nil {
		t.Fatalf("Expected success. %s", err.Error())
	}
	cacheFn := path.Join(cachedir, "src/localhost:4545/testdata/001_hello.js")
	println("good cacheFn", cacheFn)
	if !exists(cacheFn) {
		t.Fatalf("Expected 200 at '%s'", cacheFn)
	}
	// TODO check output

	// Using bad port 4546 instead of 4545.
	_, cachedir, err = deno("http://localhost:4546/testdata/001_hello.js")
	if err == nil {
		t.Fatalf("Expected 404. %s", err.Error())
	}
	// Check that cache dir is emtpy.
	cacheFn = path.Join(cachedir, "src/localhost:4546/testdata/001_hello.js")
	println("bad cacheFn", cacheFn)
	if exists(cacheFn) {
		t.Fatalf("Expected 404 at '%s'", cacheFn)
	}
}

func TestErrors(t *testing.T) {
	integrationTestSetup()

	_, _, err := deno("testdata/013_async_throw.ts")
	if err == nil {
		t.Fatalf("Expected error.")
	}

	_, _, err = deno("testdata/007_stack_trace.ts")
	if err == nil {
		t.Fatalf("Expected error.")
	}
}

func TestTestsTs(t *testing.T) {
	integrationTestSetup()
	cmd := exec.Command(denoFn, "tests.ts")
	cmd.Stdout = os.Stdout
	cmd.Stderr = os.Stderr
	err := cmd.Run()
	if err != nil {
		t.Fatal(err.Error())
	}
}
