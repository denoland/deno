package main

import (
	"github.com/golang/protobuf/proto"
	"github.com/ry/v8worker2"
	"io/ioutil"
	"os"
	"path"
	"path/filepath"
	"runtime"
	"strings"
)

func HandleCompileOutput(source string, filename string) []byte {
	// println("compile output from golang", filename)
	// Remove any ".." elements. This prevents hacking by trying to move up.
	filename, err := filepath.Rel("/", filename)
	check(err)
	if strings.Contains(filename, "..") {
		panic("Assertion error.")
	}
	filename = path.Join(CompileDir, filename)
	err = os.MkdirAll(path.Dir(filename), 0700)
	check(err)
	err = ioutil.WriteFile(filename, []byte(source), 0600)
	check(err)
	return nil
}

func ReadFileSync(filename string) []byte {
	buf, err := ioutil.ReadFile(filename)
	msg := &Msg{Kind: Msg_DATA_RESPONSE}
	if err != nil {
		msg.Error = err.Error()
	} else {
		msg.Data = buf
	}
	out, err := proto.Marshal(msg)
	check(err)
	return out
}

func UserHomeDir() string {
	if runtime.GOOS == "windows" {
		home := os.Getenv("HOMEDRIVE") + os.Getenv("HOMEPATH")
		if home == "" {
			home = os.Getenv("USERPROFILE")
		}
		return home
	}
	return os.Getenv("HOME")
}

func loadAsset(w *v8worker2.Worker, path string) {
	data, err := Asset(path)
	check(err)
	err = w.Load(path, string(data))
	check(err)
}

var DenoDir string
var CompileDir string
var SrcDir string

func createDirs() {
	DenoDir = path.Join(UserHomeDir(), ".deno")
	CompileDir = path.Join(DenoDir, "compile")
	err := os.MkdirAll(CompileDir, 0700)
	check(err)
	SrcDir = path.Join(DenoDir, "src")
	err = os.MkdirAll(SrcDir, 0700)
	check(err)
}

func check(e error) {
	if e != nil {
		panic(e)
	}
}

func recv(buf []byte) []byte {
	msg := &Msg{}
	err := proto.Unmarshal(buf, msg)
	check(err)
	switch msg.Kind {
	case Msg_READ_FILE_SYNC:
		return ReadFileSync(msg.Path)
	case Msg_EXIT:
		os.Exit(int(msg.Code))
	case Msg_COMPILE_OUTPUT:
		payload := msg.GetCompileOutput()
		return HandleCompileOutput(payload.Source, payload.Filename)
	default:
		panic("Unexpected message")
	}

	return nil
}

func main() {
	args := v8worker2.SetFlags(os.Args)
	createDirs()
	worker := v8worker2.New(recv)
	loadAsset(worker, "dist/main.js")
	cwd, err := os.Getwd()
	check(err)

	out, err := proto.Marshal(&Msg{
		Kind: Msg_START,
		Payload: &Msg_Start{
			Start: &StartMsg{
				Cwd:  cwd,
				Argv: args,
			},
		},
	})
	check(err)
	err = worker.SendBytes(out)
	if err != nil {
		os.Stderr.WriteString(err.Error())
		os.Exit(1)
	}
}
