// Copyright 2018 Ryan Dahl <ry@tinyclouds.org>
// All rights reserved. MIT License.
package deno

import (
	"encoding/json"
	"io/ioutil"
	"net/http"
	"net/url"

	"github.com/golang/protobuf/proto"
)

type requestMode string
type requestCredentials string
type requestRedirect string
type requestCache string

const (
	corsMode       = "cors"
	nocorsMode     = "no-cors"
	sameOriginMode = "same-origin"

	omitCreds       = "omit"
	sameOriginCreds = "same-origin"

	followRedirect = "follow"
	errorRedirect  = "error"
	manualRedirect = "manual"

	defaultCache = "default"
	reloadCache  = "reload"
	noCache      = "no-cache"
)

type request struct {
	URL         string             `json:"url"`
	Method      string             `json:"method"`
	Referrer    string             `json:"referrer"`
	Mode        requestMode        `json:"mode"`
	Credentials requestCredentials `json:"credentials"`
	Redirect    requestRedirect    `json:"redirect"`
	Integrity   string             `json:"integrity"`
	Cache       requestCache       `json:"cache"`
}

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

func Fetch(id int32, requestJSON string) []byte {
	logDebug("Fetch %d %s", id, requestJSON)
	r := request{}
	err := json.Unmarshal([]byte(requestJSON), &r)
	if err != nil {
		panic(err)
	}

	// Construct a http.Request with method:
	req, err := http.NewRequest(r.Method, "", nil)
	if err != nil {
		panic(err)
	}

	// Parse and set the URL:
	url, err := url.Parse(r.URL)
	if err != nil {
		panic(err)
	}
	req.URL = url

	// Check referrer field:
	if r.Referrer != "" {
		req.Header.Set("Referer", r.Referrer)
	}

	// CORS mode?
	switch r.Mode {
	case corsMode:
	case nocorsMode:
	case sameOriginMode:
	}

	// Credentials?
	switch r.Credentials {
	case omitCreds:
	case sameOriginCreds:
	}

	// Redirect policy:
	switch r.Redirect {
	case followRedirect:
	case errorRedirect:
	case manualRedirect:
	}

	// Integrity: r.Integrity
	// Cache mode
	switch r.Cache {
	case defaultCache:
	case reloadCache:
	case noCache:
	}

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

		resp, err := http.DefaultClient.Do(req)
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
		logDebug("fetch success %d %s", resMsg.FetchResStatus, req.URL.String())
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
