package uitest

import (
	"bufio"
	"encoding/json"
	"net"
	"os"
	"path/filepath"
	"time"
)

// Request addresses this isolated session's daemon, never the user's daemon.
func (s *Session) Request(op string, args any) json.RawMessage {
	s.t.Helper()
	path := filepath.Join(os.Getenv("XDG_RUNTIME_DIR"), "whiterosed-"+s.wayland+".sock")
	var conn net.Conn
	var err error
	if !waitUntil(Patience, func() bool { conn, err = net.DialTimeout("unix", path, time.Second); return err == nil }) {
		s.t.Fatalf("session daemon: %v", err)
	}
	defer conn.Close()
	conn.SetDeadline(time.Now().Add(15 * time.Second))
	json.NewEncoder(conn).Encode(map[string]any{"id": 1, "op": op, "args": args})
	sc := bufio.NewScanner(conn)
	sc.Buffer(make([]byte, 65536), 4<<20)
	for sc.Scan() {
		var reply struct {
			ID    int             `json:"reply"`
			Value json.RawMessage `json:"value"`
			Error string          `json:"error"`
		}
		if json.Unmarshal(sc.Bytes(), &reply) != nil || reply.ID != 1 {
			continue
		}
		if reply.Error != "" {
			s.t.Fatalf("%s: %s", op, reply.Error)
		}
		return reply.Value
	}
	s.t.Fatalf("%s didn't reply: %v", op, sc.Err())
	return nil
}
