// Copyright 2018 Ryan Dahl <ry@tinyclouds.org>
// All rights reserved. MIT License.
package deno

import (
	"github.com/golang/protobuf/proto"
	"time"
)

type Timer struct {
	Id       int32
	Done     bool
	Cleared  bool
	Interval bool
	Delay    int32 // In milliseconds
}

var timers = make(map[int32]*Timer)

func InitTimers() {
	Sub("timers", func(buf []byte) []byte {
		msg := &Msg{}
		check(proto.Unmarshal(buf, msg))
		switch msg.Command {
		case Msg_TIMER_START:
			id := msg.TimerStartId
			t := &Timer{
				Id:       id,
				Done:     false,
				Interval: msg.TimerStartInterval,
				Delay:    msg.TimerStartDelay,
				Cleared:  false,
			}
			if t.Delay < 10 {
				t.Delay = 10
			}
			t.StartTimer()
			timers[id] = t
			return nil
		case Msg_TIMER_CLEAR:
			// TODO maybe need mutex here.
			timer := timers[msg.TimerClearId]
			timer.Clear()
		default:
			panic("[timers] Unexpected message " + string(buf))
		}
		return nil
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
			time.Sleep(time.Duration(t.Delay) * time.Millisecond)
			if !t.Interval {
				t.Done = true
			}
			PubMsg("timers", &Msg{
				Command:        Msg_TIMER_READY,
				TimerReadyId:   t.Id,
				TimerReadyDone: t.Done,
			})
			if t.Done {
				return
			}
		}
	}()
}
