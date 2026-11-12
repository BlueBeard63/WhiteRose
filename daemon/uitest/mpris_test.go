//go:build uitest

// A stand-in music player on a test session's D-Bus (MPRIS), for the
// Music module's tests: a track playing, and the controls it was sent.
package uitest_test

import (
	"sync"
	"testing"

	"github.com/godbus/dbus/v5"
	"github.com/godbus/dbus/v5/prop"

	"whiterose/daemon/internal/uitest"
)

type player struct {
	mu    sync.Mutex
	calls []string
	props *prop.Properties
}

func (p *player) did(name string) *dbus.Error {
	p.mu.Lock()
	defer p.mu.Unlock()
	p.calls = append(p.calls, name)
	return nil
}
func (p *player) Next() *dbus.Error           { return p.did("Next") }
func (p *player) Previous() *dbus.Error       { return p.did("Previous") }
func (p *player) PlayPause() *dbus.Error      { return p.did("PlayPause") }
func (p *player) Play() *dbus.Error           { return p.did("Play") }
func (p *player) Pause() *dbus.Error          { return p.did("Pause") }
func (p *player) Stop() *dbus.Error           { return p.did("Stop") }
func (p *player) SeekTo(us int64) *dbus.Error { return p.did("Seek") }
func (p *player) SetPosition(id dbus.ObjectPath, us int64) *dbus.Error {
	return p.did("SetPosition")
}

func (p *player) Calls() []string {
	p.mu.Lock()
	defer p.mu.Unlock()
	return append([]string(nil), p.calls...)
}

// fakePlayer puts a player playing "Midnight City" by M83 on the session's bus.
func fakePlayer(t *testing.T, s *uitest.Session) *player {
	t.Helper()
	return namedPlayer(t, s, "test", "Midnight City", "Playing")
}

// namedPlayer puts player `name` (org.mpris.MediaPlayer2.<name>) on the
// session's bus with `title` by M83, its PlaybackStatus `status`.
func namedPlayer(t *testing.T, s *uitest.Session, name, title, status string) *player {
	t.Helper()
	conn, err := dbus.Connect(s.Bus())
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { conn.Close() })
	p := &player{}
	const path = "/org/mpris/MediaPlayer2"
	// (SeekTo in Go: vet wants Seek to be io.Seeker's.)
	if err := conn.ExportWithMap(p, map[string]string{"SeekTo": "Seek"}, path, "org.mpris.MediaPlayer2.Player"); err != nil {
		t.Fatal(err)
	}
	meta := map[string]dbus.Variant{
		"mpris:trackid": dbus.MakeVariant(dbus.ObjectPath("/track/1")),
		"mpris:length":  dbus.MakeVariant(int64(243_000_000)),
		"xesam:title":   dbus.MakeVariant(title),
		"xesam:artist":  dbus.MakeVariant([]string{"M83"}),
		"xesam:album":   dbus.MakeVariant("Hurry Up, We're Dreaming"),
	}
	ro := func(v any) *prop.Prop { return &prop.Prop{Value: v, Emit: prop.EmitTrue} }
	props, err := prop.Export(conn, path, prop.Map{
		"org.mpris.MediaPlayer2": {
			"Identity": ro("Test player"), "CanQuit": ro(false), "CanRaise": ro(false),
			"HasTrackList": ro(false), "SupportedUriSchemes": ro([]string{}), "SupportedMimeTypes": ro([]string{}),
		},
		"org.mpris.MediaPlayer2.Player": {
			"PlaybackStatus": ro(status), "Metadata": ro(meta), "Position": ro(int64(134_000_000)),
			"Rate": ro(1.0), "MinimumRate": ro(1.0), "MaximumRate": ro(1.0), "Volume": ro(1.0),
			"CanGoNext": ro(true), "CanGoPrevious": ro(true), "CanPlay": ro(true), "CanPause": ro(true),
			"CanSeek": ro(true), "CanControl": ro(true),
			"Shuffle":    {Value: false, Writable: true, Emit: prop.EmitTrue},
			"LoopStatus": {Value: "None", Writable: true, Emit: prop.EmitTrue},
		},
	})
	if err != nil {
		t.Fatal(err)
	}
	p.props = props
	if _, err := conn.RequestName("org.mpris.MediaPlayer2."+name, dbus.NameFlagDoNotQueue); err != nil {
		t.Fatal(err)
	}
	return p
}
