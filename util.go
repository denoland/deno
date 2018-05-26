package main

import (
	"fmt"
	"net/url"
	"os"
)

func logDebug(format string, v ...interface{}) {
	// Unless the debug flag is specified, discard logs.
	if *flagDebug {
		fmt.Printf(format+"\n", v...)
	}
}

// exists returns whether the given file or directory exists or not
func exists(path string) bool {
	_, err := os.Stat(path)
	if err == nil {
		return true
	}
	if os.IsNotExist(err) {
		return false
	}
	panic(err)
}

func assert(cond bool, msg string) {
	if !cond {
		panic(msg)
	}
}

func isRemote(filename string) bool {
	u, err := url.Parse(filename)
	check(err)
	return u.IsAbs()
}

func check(e error) {
	if e != nil {
		panic(e)
	}
}

func exitOnError(err error) {
	if err != nil {
		os.Stderr.WriteString(err.Error())
		os.Exit(1)
	}
}
