package main

import (
	"crypto/md5"
	"encoding/hex"
	"github.com/golang/protobuf/proto"
	"github.com/ry/v8worker2"
	"io/ioutil"
	"net/http"
	"os"
	"path"
	"runtime"
	"strings"
	"sync"
	"time"
)

var wg sync.WaitGroup
var resChan chan *Msg

func SourceCodeHash(filename string, sourceCodeBuf []byte) string {
	h := md5.New()
	h.Write([]byte(filename))
	h.Write(sourceCodeBuf)
	return hex.EncodeToString(h.Sum(nil))
}

func CacheFileName(filename string, sourceCodeBuf []byte) string {
	cacheKey := SourceCodeHash(filename, sourceCodeBuf)
	return path.Join(CompileDir, cacheKey+".js")
}

func IsRemotePath(filename string) bool {
	return strings.HasPrefix(filename, "/$remote$/")
}

func FetchRemoteSource(remotePath string) (buf []byte, err error) {
	url := strings.Replace(remotePath, "/$remote$/", "http://", 1)
	// println("FetchRemoteSource", url)
	res, err := http.Get(url)
	if err != nil {
		return
	}
	buf, err = ioutil.ReadAll(res.Body)
	//println("FetchRemoteSource", err.Error())
	res.Body.Close()
	return
}

func HandleSourceCodeFetch(filename string) []byte {
	res := &Msg{}
	var sourceCodeBuf []byte
	var err error
	if IsRemotePath(filename) {
		sourceCodeBuf, err = FetchRemoteSource(filename)
	} else {
		sourceCodeBuf, err = Asset("dist/" + filename)
		if err != nil {
			sourceCodeBuf, err = ioutil.ReadFile(filename)
		}
	}
	if err != nil {
		res.Error = err.Error()
	} else {
		cacheFn := CacheFileName(filename, sourceCodeBuf)
		outputCodeBuf, err := ioutil.ReadFile(cacheFn)
		var outputCode string
		if os.IsNotExist(err) {
			outputCode = ""
		} else if err != nil {
			res.Error = err.Error()
		} else {
			outputCode = string(outputCodeBuf)
		}

		res.Payload = &Msg_SourceCodeFetchRes{
			SourceCodeFetchRes: &SourceCodeFetchResMsg{
				SourceCode: string(sourceCodeBuf),
				OutputCode: outputCode,
			},
		}
	}
	out, err := proto.Marshal(res)
	check(err)
	return out
}

func HandleSourceCodeCache(filename string, sourceCode string,
	outputCode string) []byte {

	fn := CacheFileName(filename, []byte(sourceCode))
	outputCodeBuf := []byte(outputCode)
	err := ioutil.WriteFile(fn, outputCodeBuf, 0600)
	res := &Msg{}
	if err != nil {
		res.Error = err.Error()
	}
	out, err := proto.Marshal(res)
	check(err)
	return out
}

func HandleTimerStart(id int32, interval bool, duration int32) []byte {
	wg.Add(1)
	go func() {
		defer wg.Done()
		time.Sleep(time.Duration(duration) * time.Millisecond)
		resChan <- &Msg{
			Payload: &Msg_TimerReady{
				TimerReady: &TimerReadyMsg{
					Id: id,
				},
			},
		}
	}()
	return nil
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
	switch msg.Payload.(type) {
	case *Msg_Exit:
		payload := msg.GetExit()
		os.Exit(int(payload.Code))
	case *Msg_SourceCodeFetch:
		payload := msg.GetSourceCodeFetch()
		return HandleSourceCodeFetch(payload.Filename)
	case *Msg_SourceCodeCache:
		payload := msg.GetSourceCodeCache()
		return HandleSourceCodeCache(payload.Filename, payload.SourceCode,
			payload.OutputCode)
	case *Msg_TimerStart:
		payload := msg.GetTimerStart()
		return HandleTimerStart(payload.Id, payload.Interval, payload.Duration)
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

	resChan = make(chan *Msg)
	doneChan := make(chan bool)

	out, err := proto.Marshal(&Msg{
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
