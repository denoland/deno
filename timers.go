package main

import (
	"github.com/golang/protobuf/proto"
	"time"
)

func InitTimers() {
	Sub("timers", func(buf []byte) []byte {
		msg := &Msg{}
		check(proto.Unmarshal(buf, msg))
		switch msg.Payload.(type) {
		case *Msg_TimerStart:
			payload := msg.GetTimerStart()
			return HandleTimerStart(payload.Id, payload.Interval, payload.Duration)
		default:
			panic("[timers] Unexpected message " + string(buf))
		}
	})
}

func HandleTimerStart(id int32, interval bool, duration int32) []byte {
	wg.Add(1)
	go func() {
		defer wg.Done()
		time.Sleep(time.Duration(duration) * time.Millisecond)
		payload, err := proto.Marshal(&Msg{
			Payload: &Msg_TimerReady{
				TimerReady: &TimerReadyMsg{
					Id: id,
				},
			},
		})
		check(err)
		Pub("timers", payload)
	}()
	return nil
}
