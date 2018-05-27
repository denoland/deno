package main

import (
	"github.com/golang/protobuf/proto"
	"github.com/ry/v8worker2"
	"sync"
)

var resChan = make(chan *BaseMsg, 10)
var doneChan = make(chan bool)
var wg sync.WaitGroup

var stats struct {
	v8workerSend      int
	v8workerRespond   int
	v8workerRecv      int
	v8workerBytesSent int
	v8workerBytesRecv int
}

// There is a single global worker for this process.
// This file should be the only part of deno that directly access it, so that
// all interaction with V8 can go through a single point.
var worker *v8worker2.Worker

var channels = make(map[string][]Subscriber)

type Subscriber func(payload []byte) []byte

func createWorker() {
	worker = v8worker2.New(recv)
}

func recv(buf []byte) (response []byte) {
	stats.v8workerRecv++
	stats.v8workerBytesRecv += len(buf)

	msg := &BaseMsg{}
	check(proto.Unmarshal(buf, msg))
	assert(len(msg.Payload) > 0, "BaseMsg has empty payload.")
	subscribers, ok := channels[msg.Channel]
	if !ok {
		panic("No subscribers for channel " + msg.Channel)
	}
	for i := 0; i < len(subscribers); i++ {
		s := subscribers[i]
		r := s(msg.Payload)
		if r != nil {
			response = r
		}
	}
	if response != nil {
		stats.v8workerRespond++
		stats.v8workerBytesSent += len(response)
	}
	return response
}

func Sub(channel string, cb Subscriber) {
	subscribers, ok := channels[channel]
	if !ok {
		subscribers = make([]Subscriber, 0)
	}
	subscribers = append(subscribers, cb)
	channels[channel] = subscribers
}

func Pub(channel string, payload []byte) {
	wg.Add(1)
	resChan <- &BaseMsg{
		Channel: channel,
		Payload: payload,
	}
}

func PubMsg(channel string, msg *Msg) {
	payload, err := proto.Marshal(msg)
	check(err)
	Pub(channel, payload)
}

func DispatchLoop() {
	// runtime.LockOSThread()
	wg.Add(1)
	first := true

	// In a goroutine, we wait on for all goroutines to complete (for example
	// timers). We use this to signal to the main thread to exit.
	// wg.Add(1) basically translates to uv_ref, if this was Node.
	// wg.Done() basically translates to uv_unref
	go func() {
		wg.Wait()
		doneChan <- true
	}()

	for {
		select {
		case msg := <-resChan:
			wg.Done()
			out, err := proto.Marshal(msg)
			if err != nil {
				panic(err)
			}
			err = worker.SendBytes(out)
			stats.v8workerSend++
			stats.v8workerBytesSent += len(out)
			exitOnError(err)
		case <-doneChan:
			// All goroutines have completed. Now we can exit main().
			return
		}

		// We don't want to exit until we've received at least one message.
		// This is so the program doesn't exit after sending the "start"
		// message.
		if first {
			wg.Done()
		}
		first = false
	}
}
