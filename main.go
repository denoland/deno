package main

import (
	"github.com/golang/protobuf/proto"
	"github.com/ry/v8worker2"
	"os"
)

func recv(msg []byte) []byte {
	println("recv cb", string(msg))
	return nil
}

func loadAsset(w *v8worker2.Worker, path string) {
	data, err := Asset(path)
	if err != nil {
		panic("asset not found")
	}
	err = w.Load(path, string(data))
	if err != nil {
		panic(err)
	}
}

func main() {
	worker := v8worker2.New(recv)
	loadAsset(worker, "dist/main.js")

	loadMsg := &Msg{
		Kind: Msg_LOAD,
		Argv: os.Args,
	}
	out, err := proto.Marshal(loadMsg)
	if err != nil {
		panic(err)
	}
	err = worker.SendBytes(out)
	if err != nil {
		panic(err)
	}

}
