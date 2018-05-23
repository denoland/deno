package main

import (
	"flag"
	"fmt"
	"github.com/golang/protobuf/proto"
	"github.com/ry/v8worker2"
	"log"
	"os"
	"runtime/pprof"
)

var flagReload = flag.Bool("reload", false, "Reload cached remote source code.")
var flagV8Options = flag.Bool("v8-options", false, "Print V8 command line options.")
var flagDebug = flag.Bool("debug", false, "Enable debug output.")
var flagGoProf = flag.String("goprof", "", "Write golang cpu profile to file.")

var DenoDir string
var CompileDir string
var SrcDir string

func stringAsset(path string) string {
	data, err := Asset("dist/" + path)
	check(err)
	return string(data)
}

func main() {
	flag.Parse()
	args := flag.Args()
	if *flagV8Options {
		args = append(args, "--help")
		fmt.Println(args)
	}
	args = v8worker2.SetFlags(args)

	// Maybe start Golang CPU profiler.
	// Use --prof for profiling JS.
	if *flagGoProf != "" {
		f, err := os.Create(*flagGoProf)
		if err != nil {
			log.Fatal(err)
		}
		pprof.StartCPUProfile(f)
		defer pprof.StopCPUProfile()
	}

	createDirs()
	createWorker()

	InitOS()
	InitTimers()

	main_js := stringAsset("main.js")
	check(worker.Load("/main.js", main_js))
	main_map := stringAsset("main.map")

	cwd, err := os.Getwd()
	check(err)

	out, err := proto.Marshal(&Msg{
		Payload: &Msg_Start{
			Start: &StartMsg{
				Cwd:       &cwd,
				Argv:      args,
				DebugFlag: flagDebug,
				MainJs:    &main_js,
				MainMap:   &main_map,
			},
		},
	})
	check(err)
	Pub("start", out)

	DispatchLoop()
}
