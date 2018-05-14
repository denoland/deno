// To test: make && ./out/render test_input.js
package main

//go:generate ./node_modules/.bin/parcel build --out-dir=dist/ --no-minify main.ts
//go:generate go-bindata -pkg $GOPACKAGE -o assets.go dist/

import (
	"github.com/ry/v8worker2"
)

func recv(msg []byte) []byte {
	println("recv cb", string(msg))
	return nil
}

func main() {
	indexFn := "dist/main.js"
	data, err := Asset(indexFn)
	if err != nil {
		panic("asset not found")
	}
	code := string(data)

	worker := v8worker2.New(recv)

	// Load up index.js code.
	err = worker.Load(indexFn, code)
	if err != nil {
		println("Problem executing Javascript.")
		panic(err)
	}
}
