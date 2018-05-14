package main

import (
	"github.com/golang/protobuf/proto"
	"github.com/ry/v8worker2"
	"io/ioutil"
	"os"
)

func ReadFileSync(filename string) []byte {
	buf, err := ioutil.ReadFile(filename)
	msg := &Msg{Kind: Msg_DATA_RESPONSE}
	if err != nil {
		msg.Error = err.Error()
	} else {
		msg.Data = buf
	}
	out, err := proto.Marshal(msg)
	if err != nil {
		panic(err)
	}
	return out
}

func recv(buf []byte) []byte {
	msg := &Msg{}
	err := proto.Unmarshal(buf, msg)
	if err != nil {
		panic(err)
	}
	switch msg.Kind {
	case Msg_READ_FILE_SYNC:
		return ReadFileSync(msg.Path)
	default:
		panic("Unexpected message")
	}
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
