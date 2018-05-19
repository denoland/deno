package main

import (
	"crypto/md5"
	"encoding/hex"
	"github.com/golang/protobuf/proto"
	"github.com/ry/v8worker2"
	"io"
	"io/ioutil"
	"net/http"
	"net/url"
	"os"
	"path"
	"runtime"
	"strings"
	"sync"
	"time"
)

var DenoDir string
var CompileDir string
var SrcDir string

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

func IsRemote(filename string) bool {
	u, err := url.Parse(filename)
	check(err)
	return u.IsAbs()
}

// Fetches a remoteUrl but also caches it to the localFilename.
func FetchRemoteSource(remoteUrl string, localFilename string) ([]byte, error) {
	Assert(strings.HasPrefix(localFilename, SrcDir), localFilename)
	var sourceReader io.Reader

	file, err := os.Open(localFilename)
	if os.IsNotExist(err) {
		// Fetch from HTTP.
		res, err := http.Get(remoteUrl)
		if err != nil {
			return nil, err
		}
		defer res.Body.Close()

		err = os.MkdirAll(path.Dir(localFilename), 0700)
		if err != nil {
			return nil, err
		}

		// Write to to file. Need to reopen it for writing.
		file, err = os.OpenFile(localFilename, os.O_RDWR|os.O_CREATE, 0700)
		if err != nil {
			return nil, err
		}
		sourceReader = io.TeeReader(res.Body, file) // Fancy!

	} else if err != nil {
		return nil, err
	} else {
		sourceReader = file
	}
	defer file.Close()
	return ioutil.ReadAll(sourceReader)
}

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

const assetPrefix string = "/$asset$/"

func HandleSourceCodeFetch(moduleSpecifier string, containingFile string) (out []byte) {
	res := &Msg{}
	var sourceCodeBuf []byte
	var err error

	defer func() {
		if err != nil {
			res.Error = err.Error()
		}
		out, err = proto.Marshal(res)
		check(err)
	}()

	moduleName, filename, err := ResolveModule(moduleSpecifier, containingFile)
	if err != nil {
		return
	}

	if IsRemote(moduleName) {
		sourceCodeBuf, err = FetchRemoteSource(moduleName, filename)
	} else if strings.HasPrefix(moduleName, assetPrefix) {
		f := strings.TrimPrefix(moduleName, assetPrefix)
		sourceCodeBuf, err = Asset("dist/" + f)
	} else {
		Assert(moduleName == filename,
			"if a module isn't remote, it should have the same filename")
		sourceCodeBuf, err = ioutil.ReadFile(moduleName)
	}
	if err != nil {
		return
	}

	outputCode, err := LoadOutputCodeCache(filename, sourceCodeBuf)
	if err != nil {
		return
	}

	res.Payload = &Msg_SourceCodeFetchRes{
		SourceCodeFetchRes: &SourceCodeFetchResMsg{
			ModuleName: moduleName,
			Filename:   filename,
			SourceCode: string(sourceCodeBuf),
			OutputCode: outputCode,
		},
	}
	return
}

func LoadOutputCodeCache(filename string, sourceCodeBuf []byte) (outputCode string, err error) {
	cacheFn := CacheFileName(filename, sourceCodeBuf)
	outputCodeBuf, err := ioutil.ReadFile(cacheFn)
	if os.IsNotExist(err) {
		err = nil // Ignore error if we can't load the cache.
	} else if err != nil {
		outputCode = string(outputCodeBuf)
	}
	return
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
		return HandleSourceCodeFetch(payload.ModuleSpecifier, payload.ContainingFile)
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
