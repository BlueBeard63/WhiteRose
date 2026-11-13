package launch

import (
	"fmt"
	"io"
	"os"
	"path/filepath"
	"sync"
	"time"
)

// LogMax is how big the session log gets before its older half is dropped.
const LogMax = 8 << 20

// SessionLog and SessionLogBefore: what this session printed (compositor,
// daemon and shell share the launcher's stderr), and the session before
// (after a crash, the one that crashed). Bug reports take their ends.
func SessionLog(stateHome string) string {
	return filepath.Join(stateHome, "whiterose", "session.log")
}
func SessionLogBefore(stateHome string) string {
	return filepath.Join(stateHome, "whiterose", "session.1.log")
}

// capLog is a file that never grows past max: when it does, its newer
// half is kept (cut at a line start) and writing goes on after it.
type capLog struct {
	mu   sync.Mutex
	f    *os.File
	size int64
	max  int64
}

// openSessionLog moves the last session's log to session.1.log and starts
// a new one, headed with the version and time.
func openSessionLog(stateHome, version string, max int64) (*capLog, error) {
	path := SessionLog(stateHome)
	if err := os.MkdirAll(filepath.Dir(path), 0o700); err != nil {
		return nil, err
	}
	if _, err := os.Stat(path); err == nil {
		_ = os.Rename(path, SessionLogBefore(stateHome))
	}
	f, err := os.OpenFile(path, os.O_CREATE|os.O_TRUNC|os.O_RDWR, 0o600)
	if err != nil {
		return nil, err
	}
	l := &capLog{f: f, max: max}
	fmt.Fprintf(l, "WhiteRose %s, session started %s\n", version, time.Now().Format(time.DateTime))
	return l, nil
}

func (l *capLog) Write(p []byte) (int, error) {
	l.mu.Lock()
	defer l.mu.Unlock()
	n, err := l.f.Write(p)
	l.size += int64(n)
	if l.size > l.max {
		l.shrink()
	}
	// A log that can't be written never stops the session's output.
	_ = err
	return len(p), nil
}

// shrink keeps the newer half.
func (l *capLog) shrink() {
	keep := l.max / 2
	buf := make([]byte, keep)
	n, _ := l.f.ReadAt(buf, l.size-keep)
	buf = buf[:n]
	// From the next line on: no half line at the top.
	for i, b := range buf {
		if b == '\n' {
			buf = buf[i+1:]
			break
		}
	}
	head := []byte("(older lines dropped: the log keeps the last " + fmt.Sprint(l.max>>20) + " MB or so)\n")
	if err := l.f.Truncate(0); err != nil {
		return
	}
	if _, err := l.f.Seek(0, io.SeekStart); err != nil {
		return
	}
	a, _ := l.f.Write(head)
	b, _ := l.f.Write(buf)
	l.size = int64(a + b)
}

func (l *capLog) Close() error { return l.f.Close() }
