// Package store is the user's settings database: one SQLite file
// (~/.local/share/whiterose/settings.db) owned by whiterosed. Every setting is
// a key ("dock.pinned", "quick.doNotDisturb") holding a JSON value. Known keys
// have a default and a type check (keys.go); installable settings modules keep
// theirs under "module.<name>.". Secrets (calendar links) never go in here
// as plain settings: see secrets.go.
//
// Changes are pushed to subscribers, so the shell and the Settings app see an
// edit the moment it's made.
package store

import (
	"bytes"
	"database/sql"
	"encoding/json"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"sort"
	"sync"
	"time"

	_ "modernc.org/sqlite" // pure Go: no C toolchain, ships in the one package
)

// Change is one setting's new value (Value is the default when it was reset).
type Change struct {
	Key   string          `json:"key"`
	Value json.RawMessage `json:"value"`
}

type Store struct {
	db *sql.DB

	mu   sync.Mutex
	subs map[int]chan Change
	next int
}

// schema is applied in order; user_version records how far a file got.
var schema = []string{
	`CREATE TABLE settings (key TEXT PRIMARY KEY, value TEXT NOT NULL, updated INTEGER NOT NULL)`,
	`CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT NOT NULL)`,
	`CREATE TABLE secrets (id TEXT PRIMARY KEY, value TEXT NOT NULL)`,
}

// DefaultPath is $XDG_DATA_HOME/whiterose/settings.db.
func DefaultPath() string {
	dir := os.Getenv("XDG_DATA_HOME")
	if dir == "" {
		home, _ := os.UserHomeDir()
		dir = filepath.Join(home, ".local", "share")
	}
	return filepath.Join(dir, "whiterose", "settings.db")
}

// Open opens (creating if needed) the database at path. Only the user can
// read it (the folder is 0700, the file 0600).
func Open(path string) (*Store, error) {
	if err := os.MkdirAll(filepath.Dir(path), 0o700); err != nil {
		return nil, err
	}
	db, err := sql.Open("sqlite", "file:"+path+"?_pragma=journal_mode(WAL)&_pragma=busy_timeout(5000)&_pragma=synchronous(NORMAL)")
	if err != nil {
		return nil, err
	}
	db.SetMaxOpenConns(1) // one writer; SQLite serialises anyway
	s := &Store{db: db, subs: map[int]chan Change{}}
	if err := s.migrate(); err != nil {
		db.Close()
		return nil, fmt.Errorf("settings database %s: %w", path, err)
	}
	for _, p := range []string{path, path + "-wal", path + "-shm"} {
		_ = os.Chmod(p, 0o600)
	}
	return s, nil
}

func (s *Store) migrate() error {
	var v int
	if err := s.db.QueryRow(`PRAGMA user_version`).Scan(&v); err != nil {
		return err
	}
	if v > len(schema) {
		return fmt.Errorf("made by a newer WhiteRose (schema %d, this one knows %d)", v, len(schema))
	}
	for i := v; i < len(schema); i++ {
		tx, err := s.db.Begin()
		if err != nil {
			return err
		}
		if _, err := tx.Exec(schema[i]); err != nil {
			tx.Rollback()
			return err
		}
		if _, err := tx.Exec(fmt.Sprintf(`PRAGMA user_version = %d`, i+1)); err != nil {
			tx.Rollback()
			return err
		}
		if err := tx.Commit(); err != nil {
			return err
		}
	}
	return nil
}

func (s *Store) Close() error { return s.db.Close() }

// Get returns a setting, or its default when it was never set. ok is false
// for an unknown key with no stored value.
func (s *Store) Get(key string) (json.RawMessage, bool, error) {
	var v string
	err := s.db.QueryRow(`SELECT value FROM settings WHERE key = ?`, key).Scan(&v)
	if err == nil {
		return json.RawMessage(v), true, nil
	}
	if !errors.Is(err, sql.ErrNoRows) {
		return nil, false, err
	}
	if k, ok := Known[key]; ok {
		return k.Default, true, nil
	}
	return nil, false, nil
}

// GetInto decodes a setting (or its default) into out.
func (s *Store) GetInto(key string, out any) error {
	v, ok, err := s.Get(key)
	if err != nil || !ok {
		return err
	}
	return json.Unmarshal(v, out)
}

// All returns every known setting (stored or default) plus every stored
// module setting.
func (s *Store) All() (map[string]json.RawMessage, error) {
	out := map[string]json.RawMessage{}
	for k, v := range Known {
		out[k] = v.Default
	}
	rows, err := s.db.Query(`SELECT key, value FROM settings`)
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	for rows.Next() {
		var k, v string
		if err := rows.Scan(&k, &v); err != nil {
			return nil, err
		}
		out[k] = json.RawMessage(v)
	}
	return out, rows.Err()
}

// Keys lists All's keys, sorted (for listings).
func Keys(all map[string]json.RawMessage) []string {
	keys := make([]string, 0, len(all))
	for k := range all {
		keys = append(keys, k)
	}
	sort.Strings(keys)
	return keys
}

// MaxValue bounds one setting (launcher history and friends are small).
const MaxValue = 1 << 20

// Set validates and stores a setting, then tells subscribers.
func (s *Store) Set(key string, value json.RawMessage) error {
	if err := Validate(key, value); err != nil {
		return err
	}
	compact, err := compactJSON(value)
	if err != nil {
		return err
	}
	if _, err := s.db.Exec(`INSERT INTO settings (key, value, updated) VALUES (?, ?, ?)
		ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated = excluded.updated`,
		key, string(compact), time.Now().Unix()); err != nil {
		return err
	}
	s.publish(Change{Key: key, Value: compact})
	return nil
}

// SetValue is Set for a Go value.
func (s *Store) SetValue(key string, v any) error {
	b, err := json.Marshal(v)
	if err != nil {
		return err
	}
	return s.Set(key, b)
}

// Reset removes a setting, so it reads as its default again.
func (s *Store) Reset(key string) error {
	if err := checkKey(key); err != nil {
		return err
	}
	if _, err := s.db.Exec(`DELETE FROM settings WHERE key = ?`, key); err != nil {
		return err
	}
	v, _, _ := s.Get(key)
	if v == nil {
		v = json.RawMessage("null")
	}
	s.publish(Change{Key: key, Value: v})
	return nil
}

func compactJSON(v json.RawMessage) (json.RawMessage, error) {
	var buf bytes.Buffer
	if err := json.Compact(&buf, v); err != nil {
		return nil, err
	}
	return buf.Bytes(), nil
}

// Meta is bookkeeping that isn't a setting (what was imported, ...).
func (s *Store) Meta(key string) (string, bool) {
	var v string
	err := s.db.QueryRow(`SELECT value FROM meta WHERE key = ?`, key).Scan(&v)
	return v, err == nil
}

func (s *Store) SetMeta(key, value string) error {
	_, err := s.db.Exec(`INSERT INTO meta (key, value) VALUES (?, ?) ON CONFLICT(key) DO UPDATE SET value = excluded.value`, key, value)
	return err
}

// Subscribe returns a channel of every change from now on, and a function to
// stop. A subscriber that falls far behind misses changes rather than
// blocking writers (it can re-read All).
func (s *Store) Subscribe() (<-chan Change, func()) {
	s.mu.Lock()
	defer s.mu.Unlock()
	id := s.next
	s.next++
	ch := make(chan Change, 64)
	s.subs[id] = ch
	return ch, func() {
		s.mu.Lock()
		defer s.mu.Unlock()
		if c, ok := s.subs[id]; ok {
			close(c)
			delete(s.subs, id)
		}
	}
}

func (s *Store) publish(c Change) {
	s.mu.Lock()
	defer s.mu.Unlock()
	for _, ch := range s.subs {
		select {
		case ch <- c:
		default:
		}
	}
}
