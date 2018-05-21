package main

import (
	"flag"
	"fmt"
	"github.com/golang/protobuf/proto"
	"github.com/ry/v8worker2"
	"net/url"
	"os"
	"path"
	"sync"
)

var flagReload = flag.Bool("reload", false, "Reload cached remote source code.")
var flagV8Options = flag.Bool("v8-options", false, "Print V8 command line options.")
var flagDebug = flag.Bool("debug", false, "Enable debug output.")

var DenoDir string
var CompileDir string
var SrcDir string

var wg sync.WaitGroup
var resChan chan *Msg

func ResolveModule(moduleSpecifier string, containingFile string) (
	moduleName string, filename string, err error) {
	moduleUrl, err := url.Parse(moduleSpecifier)
	if err != nil {
		return
	}
	baseUrl, err := url.Parse(containingFile)
	if err != nil {
		return
	}
	resolved := baseUrl.ResolveReference(moduleUrl)
	moduleName = resolved.String()
	if moduleUrl.IsAbs() {
		filename = path.Join(SrcDir, resolved.Host, resolved.Path)
	} else {
		filename = resolved.Path
	}
	return
}

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

	createDirs()
	worker := v8worker2.New(recv)

	main_js := stringAsset("main.js")
	check(worker.Load("/main.js", main_js))
	main_map := stringAsset("main.map")

	cwd, err := os.Getwd()
	check(err)

	resChan = make(chan *Msg)
	doneChan := make(chan bool)

	out, err := proto.Marshal(&Msg{
		Payload: &Msg_Start{
			Start: &StartMsg{
				Cwd:       cwd,
				Argv:      args,
				DebugFlag: *flagDebug,
				MainJs:    main_js,
				MainMap:   main_map,
			},
		},
	})
	check(err)
	err = worker.SendBytes(out)
	if err != nil {
		os.Stderr.WriteString(err.Error())
		os.Exit(1)
	}

	// In a goroutine, we wait on for all goroutines to complete (for example
	// timers). We use this to signal to the main thread to exit.
	go func() {
		wg.Wait()
		doneChan <- true
	}()

	for {
		select {
		case msg := <-resChan:
			out, err := proto.Marshal(msg)
			err = worker.SendBytes(out)
			check(err)
		case <-doneChan:
			// All goroutines have completed. Now we can exit main().
			return
		}
	}
}
