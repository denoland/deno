// Copyright 2018 Ryan Dahl <ry@tinyclouds.org>
// All rights reserved. MIT License.
package main

import (
	"flag"
	"github.com/ry/v8worker2"
	"os"
	"runtime/pprof"
)

var flagReload = flag.Bool("reload", false, "Reload cached remote source code.")
var flagV8Options = flag.Bool("v8-options", false, "Print V8 command line options.")
var flagDebug = flag.Bool("debug", false, "Enable debug output.")
var flagGoProf = flag.String("goprof", "", "Write golang cpu profile to file.")

func stringAsset(path string) string {
	data, err := Asset("dist/" + path)
	check(err)
	return string(data)
}

func FlagsParse() []string {
	flag.Parse()
	args := flag.Args()
	if *flagV8Options {
		args = append(args, "--help")
	}
	// Adding this causes testdata/007_stack_trace.ts to fail without a
	// stacktrace.
	// args = append(args, "--abort-on-uncaught-exception")
	args = v8worker2.SetFlags(args)

	return args
}

func main() {
	args := FlagsParse()

	// Maybe start Golang CPU profiler.
	// Use --prof for profiling JS.
	if *flagGoProf != "" {
		f, err := os.Create(*flagGoProf)
		if err != nil {
			panic(err)
		}
		pprof.StartCPUProfile(f)
		defer pprof.StopCPUProfile()
	}

	createDirs()
	createWorker()

	InitOS()
	InitEcho()
	InitTimers()
	InitFetch()

	main_js := stringAsset("main.js")
	err := worker.Load("/main.js", main_js)
	exitOnError(err)
	main_map := stringAsset("main.map")

	cwd, err := os.Getwd()
	check(err)

	var command = Msg_START // TODO use proto3
	PubMsg("start", &Msg{
		Command:        command,
		StartCwd:       cwd,
		StartArgv:      args,
		StartDebugFlag: *flagDebug,
		StartMainJs:    main_js,
		StartMainMap:   main_map,
	})

	DispatchLoop()
}
