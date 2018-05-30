// Copyright 2018 Ryan Dahl <ry@tinyclouds.org>
// All rights reserved. MIT License.
package deno

import (
	"github.com/golang/protobuf/proto"
	"io/ioutil"
	"net/http"
)

func InitFetch() {
	Sub("fetch", func(buf []byte) []byte {
		msg := &Msg{}
		check(proto.Unmarshal(buf, msg))
		switch msg.Command {
		case Msg_FETCH_REQ:
			return Fetch(
				msg.FetchReqId,
				msg.FetchReqUrl)
		default:
			panic("[fetch] Unexpected message " + string(buf))
		}
	})
}

func Fetch(id int32, targetUrl string) []byte {
	logDebug("Fetch %d %s", id, targetUrl)
	async(func() {
		resMsg := &Msg{
			Command:    Msg_FETCH_RES,
			FetchResId: id,
		}

		if !Perms.Net {
			resMsg.Error = "Network access denied."
			PubMsg("fetch", resMsg)
			return
		}

		resp, err := http.Get(targetUrl)
		if err != nil {
			resMsg.Error = err.Error()
			PubMsg("fetch", resMsg)
			return
		}
		if resp == nil {
			resMsg.Error = "resp is nil "
			PubMsg("fetch", resMsg)
			return
		}

		resMsg.FetchResStatus = int32(resp.StatusCode)
		logDebug("fetch success %d %s", resMsg.FetchResStatus, targetUrl)
		PubMsg("fetch", resMsg)

		// Now we read the body and send another message0

		defer resp.Body.Close()
		body, err := ioutil.ReadAll(resp.Body)
		if resp == nil {
			resMsg.Error = "resp is nil "
			PubMsg("fetch", resMsg)
			return
		}

		resMsg.FetchResBody = body
		PubMsg("fetch", resMsg)

		// TODO streaming.
	})
	return nil
}
