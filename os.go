package main

import (
	"github.com/golang/protobuf/proto"
	"io/ioutil"
	"os"
	"strings"
)

const assetPrefix string = "/$asset$/"

func InitOS() {
	Sub("os", func(buf []byte) []byte {
		msg := &Msg{}
		check(proto.Unmarshal(buf, msg))
		switch msg.Payload.(type) {
		case *Msg_Exit:
			payload := msg.GetExit()
			os.Exit(int(*payload.Code))
		case *Msg_SourceCodeFetch:
			payload := msg.GetSourceCodeFetch()
			return HandleSourceCodeFetch(*payload.ModuleSpecifier, *payload.ContainingFile)
		case *Msg_SourceCodeCache:
			payload := msg.GetSourceCodeCache()
			return HandleSourceCodeCache(*payload.Filename, *payload.SourceCode,
				*payload.OutputCode)
		default:
			panic("[os] Unexpected message " + string(buf))
		}
		return nil
	})
}

func HandleSourceCodeFetch(moduleSpecifier string, containingFile string) (out []byte) {
	assert(moduleSpecifier != "", "moduleSpecifier shouldn't be empty")
	res := &Msg{}
	var sourceCodeBuf []byte
	var err error

	defer func() {
		if err != nil {
			var errStr = err.Error()
			res.Error = &errStr
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

	var sourceCode = string(sourceCodeBuf)
	res.Payload = &Msg_SourceCodeFetchRes{
		SourceCodeFetchRes: &SourceCodeFetchResMsg{
			ModuleName: &moduleName,
			Filename:   &filename,
			SourceCode: &sourceCode,
			OutputCode: &outputCode,
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
		var errStr = err.Error()
		res.Error = &errStr
	}
	out, err := proto.Marshal(res)
	check(err)
	return out
}
