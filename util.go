package main

func Assert(cond bool, msg string) {
	if !cond {
		panic(msg)
	}
}
