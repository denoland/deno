// Copyright 2018 Ryan Dahl <ry@tinyclouds.org>
// All rights reserved. MIT License.
package deno

import (
	"bufio"
	"fmt"
	"net"
	"sync/atomic"

	"github.com/golang/protobuf/proto"
)

const (
	netChan = "net"
)

var (
	sockets = make(map[int32]net.Conn)
	servers = make(map[int32]*server)
	clients = make(map[int32]net.Conn)

	nextClientID int32
)

type server struct {
	id   int32
	addr string

	listener    net.Listener
	connections map[int32]net.Conn
}

func getClientID() int32 {
	id := atomic.AddInt32(&nextClientID, 1)
	return id
}

func newServer(msg *Msg) (err error) {
	s := &server{
		id:   msg.NetServerId,
		addr: fmt.Sprintf(":%d", msg.NetPort),
	}
	s.listener, err = net.Listen("tcp", s.addr)
	if err != nil {
		return err
	}
	servers[msg.NetServerId] = s
	go s.handleServer()
	return nil
}

func netClientWrite(msg *Msg) {
	client, ok := clients[msg.NetClientId]
	if !ok {
		panic("[net] Client not found")
	}
	client.Write(msg.NetData)
}

func netClientClose(msg *Msg) {
	client, ok := clients[msg.NetClientId]
	if !ok {
		panic("[net] Client not found")
	}
	client.Close()
}

func (s *server) handleServer() {
	for {
		conn, err := s.listener.Accept()
		check(err)
		id := getClientID()
		// TODO: pass LocalAddr and RemoteAddr
		clients[id] = conn

		incomingMsg := &Msg{
			Command:     Msg_NET_SERVER_CONN,
			NetServerId: s.id,
			NetClientId: id,
		}
		PubMsg(netChan, incomingMsg)
		go s.handleClient(conn, id)
	}
}

func (s *server) handleClient(conn net.Conn, id int32) {
	for {
		reader := bufio.NewReader(conn)
		data, _, err := reader.ReadLine()
		if err != nil {
			break
		}
		readMsg := &Msg{
			Command:     Msg_NET_SERVER_READ,
			NetClientId: id,
			NetData:     data,
		}
		PubMsg(netChan, readMsg)
	}
}

func InitNet() {
	Sub(netChan, func(buf []byte) []byte {
		if !Perms.Net {
			panic("Network access denied")
		}
		msg := &Msg{}
		check(proto.Unmarshal(buf, msg))
		switch msg.Command {
		case Msg_NET_CONNECT:
			netSocketConnect(msg)
		case Msg_NET_WRITE:
			netSocketWrite(msg)
		case Msg_NET_SERVER_LISTEN:
			check(newServer(msg))
		case Msg_NET_SERVER_WRITE:
			netClientWrite(msg)
		case Msg_NET_SERVER_CLOSE:
			netClientClose(msg)
		default:
			panic("[net] Unexpected message " + string(buf))
		}
		return buf
	})
}

func netSocketConnect(msg *Msg) {
	addr := fmt.Sprintf("%s:%d", msg.NetAddr, msg.NetPort)

	// Establish the connection:
	conn, err := net.Dial("tcp", addr)
	check(err)

	// The connection was ok, update the socket map and spin up a goroutine for reads:
	sockets[msg.NetSocketId] = conn
	go netSocketRead(msg.NetSocketId, conn)

	// Send NET_CONNECT_OK notification:
	okMsg := &Msg{
		Command:     Msg_NET_CONNECT_OK,
		NetSocketId: msg.NetSocketId,
	}
	PubMsg(netChan, okMsg)
}

func netSocketRead(id int32, conn net.Conn) {
	reader := bufio.NewReader(conn)
	for {
		data, _, err := reader.ReadLine()
		if err != nil {
			break
		}
		readMsg := &Msg{
			Command:     Msg_NET_READ,
			NetSocketId: id,
			NetData:     data,
		}
		PubMsg(netChan, readMsg)
	}
}

func netSocketWrite(msg *Msg) {
	conn, ok := sockets[msg.NetSocketId]
	if !ok {
		panic("[net] Socket not found")
	}
	conn.Write(msg.NetData)
}
