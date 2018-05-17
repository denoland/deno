package main

import (
	"crypto/md5"
	"encoding/hex"
	"github.com/golang/protobuf/proto"
	"github.com/ry/v8worker2"
	"io/ioutil"
	"os"
	"path"
	"runtime"
)

func SourceCodeHash(filename string, sourceCodeBuf []byte) string {
	h := md5.New()
	h.Write([]byte(filename))
	h.Write(sourceCodeBuf)
	return hex.EncodeToString(h.Sum(nil))
}

func HandleSourceCodeFetch(filename string) []byte {
	res := &Msg{Kind: Msg_SOURCE_CODE_FETCH_RES}
	sourceCodeBuf, err := Asset("dist/" + filename)
	if err != nil {
		sourceCodeBuf, err = ioutil.ReadFile(filename)
	}
	if err != nil {
		res.Error = err.Error()
	} else {
		cacheKey := SourceCodeHash(filename, sourceCodeBuf)
		println("cacheKey", filename, cacheKey)
		// TODO For now don't do any cache lookups..
		res.Payload = &Msg_SourceCodeFetchRes{
			SourceCodeFetchRes: &SourceCodeFetchResMsg{
				SourceCode: string(sourceCodeBuf),
				OutputCode: "",
			},
		}
	}
	out, err := proto.Marshal(res)
	check(err)
	return out
}

func HandleSourceCodeCache(filename string, sourceCode string, outputCode string) []byte {
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
	case Msg_SOURCE_CODE_FETCH:
		payload := msg.GetSourceCodeFetch()
		return HandleSourceCodeFetch(payload.Filename)
	case Msg_SOURCE_CODE_CACHE:
		payload := msg.GetSourceCodeCache()
		return HandleSourceCodeCache(payload.Filename, payload.SourceCode, payload.OutputCode)
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
