// Copyright 2018 Ryan Dahl <ry@tinyclouds.org>
// All rights reserved. MIT License.
package deno

import (
	"flag"
	"fmt"
	"github.com/ry/v8worker2"
	"os"
	"runtime/pprof"
)

var flagReload = flag.Bool("reload", false, "Reload cached remote source code.")
var flagV8Options = flag.Bool("v8-options", false, "Print V8 command line options.")
var flagDebug = flag.Bool("debug", false, "Enable debug output.")
var flagGoProf = flag.String("goprof", "", "Write golang cpu profile to file.")

var flagAllowRead = flag.Bool("allow-read", true,
	"Allow program to read file system.")
var flagAllowWrite = flag.Bool("allow-write", false,
	"Allow program to write to the fs.")
var flagAllowNet = flag.Bool("allow-net", false,
	"Allow program to make network connection.")

var Perms struct {
	FsRead  bool
	FsWrite bool
	Net     bool
}

func setPerms() {
	Perms.FsRead = *flagAllowRead
	Perms.FsWrite = *flagAllowWrite
	Perms.Net = *flagAllowNet
}

func stringAsset(path string) string {
	data, err := Asset("dist/" + path)
	check(err)
	return string(data)
}

func FlagsParse() []string {
	flag.Parse()
	args := flag.Args()
	setPerms()
	if *flagV8Options {
		args = append(args, "--help")
	}
	// Adding this causes testdata/007_stack_trace.ts to fail without a
	// stacktrace.
	// args = append(args, "--abort-on-uncaught-exception")
	args = v8worker2.SetFlags(args)

	return args
}

// There is a single global worker for this process.
// This file should be the only part of deno that directly access it, so that
// all interaction with V8 can go through a single point.
var worker *v8worker2.Worker
var workerArgs []string
var main_js string
var main_map string

func Init() {
	workerArgs = FlagsParse()

	if len(workerArgs) == 0 {
		fmt.Fprintf(os.Stderr, "Usage: %s file.ts\n", os.Args[0])
		flag.PrintDefaults()
		os.Exit(1)
	}

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
	InitOS()
	InitEcho()
	InitTimers()
	InitFetch()

	worker = v8worker2.New(recv)

	main_js = stringAsset("main.js")
	err := worker.Load("/main.js", main_js)
	exitOnError(err)
	main_map = stringAsset("main.map")
}

// It's up to library users to call
// deno.Eval("deno_main.js", "denoMain()")
func Eval(filename string, code string) {
	err := worker.Load(filename, code)
	exitOnError(err)
}

func Loop() {
	cwd, err := os.Getwd()
	check(err)
	PubMsg("start", &Msg{
		Command:        Msg_START,
		StartCwd:       cwd,
		StartArgv:      workerArgs,
		StartDebugFlag: *flagDebug,
		StartMainJs:    main_js,
		StartMainMap:   main_map,
	})
	DispatchLoop()
}
