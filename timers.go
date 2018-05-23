package main

import (
	"github.com/golang/protobuf/proto"
	"time"
)

type Timer struct {
	Id       int32
	Done     bool
	Cleared  bool
	Interval bool
	Duration int32 // In milliseconds
}

var timers = make(map[int32]*Timer)

func InitTimers() {
	Sub("timers", func(buf []byte) []byte {
		msg := &Msg{}
		check(proto.Unmarshal(buf, msg))
		switch msg.Payload.(type) {
		case *Msg_TimerStart:
			payload := msg.GetTimerStart()
			timers[*payload.Id] = &Timer{
				Id:       *payload.Id,
				Done:     false,
				Interval: *payload.Interval,
				Duration: *payload.Duration,
				Cleared:  false,
			}
			timers[*payload.Id].StartTimer()
			return nil
		case *Msg_TimerClear:
			payload := msg.GetTimerClear()
			// TODO maybe need mutex here.
			timer := timers[*payload.Id]
			timer.Clear()
			return nil
		default:
			panic("[timers] Unexpected message " + string(buf))
		}
	})
}

func (t *Timer) Clear() {
	if !t.Cleared {
		wg.Done()
		t.Cleared = true
		delete(timers, t.Id)
	}
	t.Done = true
}

func (t *Timer) StartTimer() {
	wg.Add(1)
	go func() {
		defer t.Clear()
		for {
			time.Sleep(time.Duration(t.Duration) * time.Millisecond)
			if !t.Interval {
				t.Done = true
			}
			PubMsg("timers", &Msg{
				Payload: &Msg_TimerReady{
					TimerReady: &TimerReadyMsg{
						Id:   &t.Id,
						Done: &t.Done,
					},
				},
			})
			if t.Done {
				return
			}
		}
	}()
}
