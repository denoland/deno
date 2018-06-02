// Copyright 2018 Ryan Dahl <ry@tinyclouds.org>
// All rights reserved. MIT License.
package deno

import (
	"github.com/golang/protobuf/proto"
	"github.com/spf13/afero"
	"io/ioutil"
	"net/url"
	"os"
	"path"
	"strings"
)

const assetPrefix string = "/$asset$/"

var fs afero.Fs

func InitOS() {
	if Perms.FsWrite {
		assert(Perms.FsRead, "Write access requires read access.")
		fs = afero.NewOsFs()
	} else if Perms.FsRead {
		fs = afero.NewReadOnlyFs(afero.NewOsFs())
	} else {
		panic("Not implemented.")
	}

	Sub("os", func(buf []byte) []byte {
		msg := &Msg{}
		check(proto.Unmarshal(buf, msg))
		switch msg.Command {
		case Msg_CODE_FETCH:
			return HandleCodeFetch(
				msg.CodeFetchModuleSpecifier,
				msg.CodeFetchContainingFile)
		case Msg_CODE_CACHE:
			return HandleCodeCache(
				msg.CodeCacheFilename,
				msg.CodeCacheSourceCode,
				msg.CodeCacheOutputCode)
		case Msg_EXIT:
			os.Exit(int(msg.ExitCode))
		case Msg_READ_FILE_SYNC:
			return ReadFileSync(msg.ReadFileSyncFilename)
		case Msg_WRITE_FILE_SYNC:
			return WriteFileSync(msg.WriteFileSyncFilename, msg.WriteFileSyncData,
				msg.WriteFileSyncPerm)
		default:
			panic("[os] Unexpected message " + string(buf))
		}
		return nil
	})
}

func SrcFileToUrl(filename string) string {
	assert(len(SrcDir) > 0, "SrcDir shouldn't be empty")
	if strings.HasPrefix(filename, SrcDir) {
		rest := strings.TrimPrefix(filename, SrcDir)
		if rest[0] == '/' {
			rest = rest[1:]
		}

		return "http://" + rest
	} else {
		return filename
	}
}

func ResolveModule(moduleSpecifier string, containingFile string) (
	moduleName string, filename string, err error) {

	logDebug("os.go ResolveModule moduleSpecifier %s containingFile %s",
		moduleSpecifier, containingFile)

	containingFile = SrcFileToUrl(containingFile)
	moduleSpecifier = SrcFileToUrl(moduleSpecifier)

	logDebug("os.go ResolveModule after moduleSpecifier %s containingFile %s",
		moduleSpecifier, containingFile)

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
	if resolved.IsAbs() {
		filename = path.Join(SrcDir, resolved.Host, resolved.Path)
	} else {
		filename = resolved.Path
	}
	return
}

func HandleCodeFetch(moduleSpecifier string, containingFile string) (out []byte) {
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

	logDebug("CodeFetch moduleName %s moduleSpecifier %s containingFile %s filename %s",
		moduleName, moduleSpecifier, containingFile, filename)

	if isRemote(moduleName) {
		sourceCodeBuf, err = FetchRemoteSource(moduleName, filename)
	} else if strings.HasPrefix(moduleName, assetPrefix) {
		f := strings.TrimPrefix(moduleName, assetPrefix)
		sourceCodeBuf, err = Asset("dist/" + f)
		if err != nil {
			logDebug("%s Asset doesn't exist. Return without error", moduleName)
			err = nil
		}
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
	res = &Msg{
		Command:                Msg_CODE_FETCH_RES,
		CodeFetchResModuleName: moduleName,
		CodeFetchResFilename:   filename,
		CodeFetchResSourceCode: sourceCode,
		CodeFetchResOutputCode: outputCode,
	}
	return
}

func HandleCodeCache(filename string, sourceCode string,
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

func ReadFileSync(filename string) []byte {
	data, err := afero.ReadFile(fs, filename)
	res := &Msg{
		Command: Msg_READ_FILE_SYNC_RES,
	}
	if err != nil {
		res.Error = err.Error()
	} else {
		res.ReadFileSyncData = data
	}
	out, err := proto.Marshal(res)
	check(err)
	return out
}

func WriteFileSync(filename string, data []byte, perm uint32) []byte {
	err := afero.WriteFile(fs, filename, data, os.FileMode(perm))
	res := &Msg{}
	if err != nil {
		res.Error = err.Error()
	}
	out, err := proto.Marshal(res)
	check(err)
	return out
}
