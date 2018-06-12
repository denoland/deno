// Copyright 2018 Ryan Dahl <ry@tinyclouds.org>
// All rights reserved. MIT License.
package deno

import (
	"crypto/md5"
	"encoding/hex"
	"flag"
	"io"
	"io/ioutil"
	"net/http"
	"os"
	"path"
	"runtime"
	"strings"
)

var flagCacheDir = flag.String("cachedir", "",
	"Where to cache compilation artifacts. Default: ~/.deno")

var DenoDir string
var CacheDir string
var SrcDir string

func SourceCodeHash(filename string, sourceCodeBuf []byte) string {
	h := md5.New()
	h.Write([]byte(filename))
	h.Write(sourceCodeBuf)
	return hex.EncodeToString(h.Sum(nil))
}

func CacheFileName(filename string, sourceCodeBuf []byte) string {
	cacheKey := SourceCodeHash(filename, sourceCodeBuf)
	return path.Join(CacheDir, cacheKey+".js")
}

// Fetches a remoteUrl but also caches it to the localFilename.
func FetchRemoteSource(remoteUrl string, localFilename string) ([]byte, error) {
	logDebug("FetchRemoteSource %s %s", remoteUrl, localFilename)
	assert(strings.HasPrefix(localFilename, SrcDir),
		"Expected filename to start with SrcDir: "+localFilename)
	var sourceReader io.Reader

	file, err := os.Open(localFilename)
	if *flagReload || os.IsNotExist(err) {
		// Fetch from HTTP.
		println("Downloading", remoteUrl)
		res, err := http.Get(remoteUrl)
		if err != nil {
			return nil, err
		}
		defer res.Body.Close()

		err = os.MkdirAll(path.Dir(localFilename), 0700)
		if err != nil {
			return nil, err
		}

		// Write to local file. Need to reopen it for writing.
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

func LoadOutputCodeCache(filename string, sourceCodeBuf []byte) (
	outputCode string, err error) {
	cacheFn := CacheFileName(filename, sourceCodeBuf)
	outputCodeBuf, err := ioutil.ReadFile(cacheFn)
	if os.IsNotExist(err) {
		// Ignore error if we can't find the cache file.
		err = nil
	} else if err == nil {
		outputCode = string(outputCodeBuf)
	}
	return outputCode, err
}

func UserHomeDir() string {
	if runtime.GOOS == "windows" {
		home := path.Join(os.Getenv("HOMEDRIVE"), os.Getenv("HOMEPATH"))
		if home == "" {
			home = os.Getenv("USERPROFILE")
		}
		return home
	}
	return os.Getenv("HOME")
}

func createDirs() {
	if *flagCacheDir == "" {
		DenoDir = path.Join(UserHomeDir(), ".deno")
	} else {
		DenoDir = *flagCacheDir
	}
	CacheDir = path.Join(DenoDir, "cache")
	err := os.MkdirAll(CacheDir, 0700)
	check(err)
	SrcDir = path.Join(DenoDir, "src")
	err = os.MkdirAll(SrcDir, 0700)
	check(err)
}
