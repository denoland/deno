// Copyright 2018 Ryan Dahl <ry@tinyclouds.org>
// All rights reserved. MIT License.
package deno

import (
	"fmt"
	"io/ioutil"
	"net/http"
	"sync/atomic"

	"github.com/golang/protobuf/proto"
)

const (
	httpChan     = "http"
	serverHeader = "Deno"
)

var (
	httpServers = make(map[int32]*http.Server)
)

func InitHTTP() {
	Sub(httpChan, func(buf []byte) []byte {
		msg := &Msg{}
		check(proto.Unmarshal(buf, msg))
		switch msg.Command {
		case Msg_HTTP_CREATE:
			httpCreate(msg.HttpServerId)
		case Msg_HTTP_LISTEN:
			httpListen(msg.HttpServerId, msg.HttpListenPort)
		default:
			panic("[http] Unexpected message " + string(buf))
		}
		return buf
	})
}

var nextReqID int32

func getReqID() (int32, string) {
	id := atomic.AddInt32(&nextReqID, 1)
	return id, fmt.Sprintf("%s/%d", httpChan, id)
}

func buildHTTPHandler(serverID int32) func(w http.ResponseWriter, r *http.Request) {
	return func(w http.ResponseWriter, r *http.Request) {
		// Increment and get an ID for this request:
		id, ch := getReqID()

		// Used to signal end:
		done := make(chan bool)

		// Subscribe to this channel and handle stuff:
		Sub(ch, func(buf []byte) []byte {
			msg := &Msg{}
			proto.Unmarshal(buf, msg)
			switch msg.Command {
			case Msg_HTTP_RES_WRITE:
				w.Write(msg.HttpResBody)
			case Msg_HTTP_RES_STATUS:
				w.WriteHeader(int(msg.HttpResCode))
			case Msg_HTTP_RES_END:
				done <- true
			}
			return buf
		})

		// Prepare and publish request message:
		var body []byte
		if r.Body != nil {
			body, _ = ioutil.ReadAll(r.Body)
		}
		msg := &Msg{}
		msg.HttpReqId = id
		msg.HttpReqBody = body
		msg.Command = Msg_HTTP_REQ
		msg.HttpReqPath = r.URL.Path
		msg.HttpReqMethod = r.Method
		go PubMsg(httpChan, msg)

		w.Header().Set("Server", serverHeader)

		// Block and wait for done signal:
		<-done
	}
}

func httpCreate(serverID int32) {
	if !Perms.Net {
		panic("Network access denied")
	}
	httpServers[serverID] = &http.Server{}
}

func httpListen(serverID int32, port int32) {
	if !Perms.Net {
		panic("Network access denied")
	}
	s := httpServers[serverID]
	listenAddr := fmt.Sprintf(":%d", port)
	handler := buildHTTPHandler(serverID)
	s.Addr = listenAddr
	s.Handler = http.HandlerFunc(handler)
	go s.ListenAndServe()
}
