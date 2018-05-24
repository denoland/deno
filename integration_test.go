package main

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

// Some tests require an HTTP server. We start one here.
// Because we process tests synchronously in this program we must run
// the server as a subprocess.
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

func CheckOutput(t *testing.T, outFile string, denoFn string) {
	outFile = path.Join("testdata", outFile)
	jsFile := strings.TrimSuffix(outFile, ".out")

	expected, err := ioutil.ReadFile(outFile)
	if err != nil {
		t.Fatal(err.Error())
	}

	cmd := exec.Command(denoFn, jsFile, "--reload")
	var out bytes.Buffer
	cmd.Stdout = &out
	err = cmd.Run()
	if err != nil {
		t.Fatal(err.Error())
	}
	actual := out.Bytes()
	if bytes.Compare(actual, expected) != 0 {
		t.Fatalf(`Actual output does not match expected.
-----Actual-------------------
%s-----Expected-----------------
%s------------------------------`, string(actual), string(expected))
	}
}

func TestIntegration(t *testing.T) {
	startServer()
	cwd, err := os.Getwd()
	if err != nil {
		panic(err)
	}
	denoFn := path.Join(cwd, "deno")
	outFiles := listTestFiles()
	for _, outFile := range outFiles {
		t.Run(outFile, func(t *testing.T) {
			CheckOutput(t, outFile, denoFn)
		})
	}
}
