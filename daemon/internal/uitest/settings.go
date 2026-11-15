package uitest

import (
	"encoding/json"
	"path/filepath"
	"reflect"

	"whiterose/daemon/internal/store"
)

// The session's settings database (its whiterosed's).
func (s *Session) settingsPath() string {
	return filepath.Join(s.Home, ".local/share/whiterose/settings.db")
}

// seed stores settings before the session starts.
func (s *Session) seed(values map[string]any) {
	if len(values) == 0 {
		return
	}
	st, err := store.Open(s.settingsPath())
	if err != nil {
		s.t.Fatalf("settings: %v", err)
	}
	defer st.Close()
	for k, v := range values {
		if err := st.SetValue(k, v); err != nil {
			s.t.Fatalf("setting %s: %v", k, err)
		}
	}
}

// Setting is a setting's stored value as JSON ("" if it isn't stored).
func (s *Session) Setting(key string) string {
	s.t.Helper()
	st, err := store.Open(s.settingsPath())
	if err != nil {
		s.t.Fatalf("settings: %v", err)
	}
	defer st.Close()
	v, ok, err := st.Get(key)
	if err != nil {
		s.t.Fatalf("setting %s: %v", key, err)
	}
	if !ok {
		return ""
	}
	return string(v)
}

// WaitSetting waits until the setting `key` is stored as `want`.
func (s *Session) WaitSetting(key string, want any) {
	s.t.Helper()
	var got string
	if !waitUntil(Patience, func() bool {
		got = s.Setting(key)
		var have any
		if json.Unmarshal([]byte(got), &have) != nil {
			return false
		}
		w, _ := json.Marshal(want)
		var wanted any
		json.Unmarshal(w, &wanted)
		return reflect.DeepEqual(have, wanted)
	}) {
		s.t.Fatalf("setting %s is %s, not %v", key, got, want)
	}
}
