package main

import (
	"github.com/golang/protobuf/proto"
	"io/ioutil"
	"os"
	"strings"
	"time"
)

const assetPrefix string = "/$asset$/"

func InitHandlers() {
	Sub("os", func(buf []byte) []byte {
		msg := &Msg{}
		check(proto.Unmarshal(buf, msg))
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
		default:
			panic("[os] Unexpected message " + string(buf))
		}
		return nil
	})

	Sub("timers", func(buf []byte) []byte {
		msg := &Msg{}
		check(proto.Unmarshal(buf, msg))
		switch msg.Payload.(type) {
		case *Msg_TimerStart:
			payload := msg.GetTimerStart()
			return HandleTimerStart(payload.Id, payload.Interval, payload.Duration)
		default:
			panic("[timers] Unexpected message " + string(buf))
		}
	})
}

func HandleSourceCodeFetch(moduleSpecifier string, containingFile string) (out []byte) {
	assert(moduleSpecifier != "", "moduleSpecifier shouldn't be empty")
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

	//println("HandleSourceCodeFetch", "moduleSpecifier", moduleSpecifier,
	//		"containingFile", containingFile, "filename", filename)

	if isRemote(moduleName) {
		sourceCodeBuf, err = FetchRemoteSource(moduleName, filename)
	} else if strings.HasPrefix(moduleName, assetPrefix) {
		f := strings.TrimPrefix(moduleName, assetPrefix)
		sourceCodeBuf, err = Asset("dist/" + f)
	} else {
		assert(moduleName == filename,
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
		payload, err := proto.Marshal(&Msg{
			Payload: &Msg_TimerReady{
				TimerReady: &TimerReadyMsg{
					Id: id,
				},
			},
		})
		check(err)
		Pub("timers", payload)
	}()
	return nil
}
