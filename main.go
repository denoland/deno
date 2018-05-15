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
	case Msg_EXIT:
		os.Exit(int(msg.Code))
	default:
		panic("Unexpected message")
	}

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
	args := v8worker2.SetFlags(os.Args)
	worker := v8worker2.New(recv)
	loadAsset(worker, "dist/main.js")
	cwd, err := os.Getwd()
	if err != nil {
		panic(err)
	}
	out, err := proto.Marshal(&Msg{
		Kind: Msg_START,
		Cwd:  cwd,
		Argv: args,
	})
	if err != nil {
		panic(err)
	}
	err = worker.SendBytes(out)
	if err != nil {
		os.Stderr.WriteString(err.Error())
		os.Exit(1)
	}
}
