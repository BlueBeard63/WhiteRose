package store

import (
	"crypto/rand"
	"database/sql"
	"encoding/hex"
	"errors"
	"fmt"
	"sync"
	"time"

	"github.com/godbus/dbus/v5"
)

// Secrets keeps values that mustn't sit in the settings database as plain
// settings (calendar links carry access tokens).
type Secrets interface {
	Get(id string) (string, error)
	Set(id, value string) error
	Delete(id string) error
	Name() string
}

// NewSecretID makes an id for a new secret.
func NewSecretID() string {
	b := make([]byte, 12)
	_, _ = rand.Read(b)
	return hex.EncodeToString(b)
}

// ErrNoSecret: no secret with that id.
var ErrNoSecret = errors.New("secret not found")

// ErrLocked: the keyring is there but locked, so what it keeps can't be
// read yet (a fingerprint login has no password to unlock it with).
var ErrLocked = errors.New("keyring is locked")

// ---- fallback: the database itself (0600, only the user can read it) ----

type dbSecrets struct{ db *sql.DB }

// Fallback stores secrets in the settings database, for sessions without a
// keyring.
func (s *Store) Fallback() Secrets { return dbSecrets{s.db} }

func (d dbSecrets) Name() string { return "settings database" }

func (d dbSecrets) Get(id string) (string, error) {
	var v string
	err := d.db.QueryRow(`SELECT value FROM secrets WHERE id = ?`, id).Scan(&v)
	if errors.Is(err, sql.ErrNoRows) {
		return "", ErrNoSecret
	}
	return v, err
}

func (d dbSecrets) Set(id, value string) error {
	_, err := d.db.Exec(`INSERT INTO secrets (id, value) VALUES (?, ?) ON CONFLICT(id) DO UPDATE SET value = excluded.value`, id, value)
	return err
}

func (d dbSecrets) Delete(id string) error {
	_, err := d.db.Exec(`DELETE FROM secrets WHERE id = ?`, id)
	return err
}

func (d dbSecrets) ids() ([]string, error) {
	rows, err := d.db.Query(`SELECT id FROM secrets`)
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	var out []string
	for rows.Next() {
		var id string
		if err := rows.Scan(&id); err != nil {
			return nil, err
		}
		out = append(out, id)
	}
	return out, rows.Err()
}

// ---- the desktop keyring (freedesktop Secret Service) ----

const (
	ssName    = "org.freedesktop.secrets"
	ssPath    = dbus.ObjectPath("/org/freedesktop/secrets")
	ssService = "org.freedesktop.Secret.Service"
	ssDefault = dbus.ObjectPath("/org/freedesktop/secrets/aliases/default")
)

type ssSecret struct {
	Session     dbus.ObjectPath
	Parameters  []byte
	Value       []byte
	ContentType string
}

type keyring struct {
	conn    *dbus.Conn
	session dbus.ObjectPath
}

func (k *keyring) Name() string { return "keyring" }

// Keyring connects to a Secret Service that is already running (GNOME
// Keyring, KWallet) with its default collection unlocked. It never starts
// one: that could hang or pop up an unlock prompt. Errors mean "use the
// fallback".
func Keyring() (Secrets, error) {
	conn, err := dbus.ConnectSessionBus()
	if err != nil {
		return nil, err
	}
	var owned bool
	if err := conn.BusObject().Call("org.freedesktop.DBus.NameHasOwner", 0, ssName).Store(&owned); err != nil || !owned {
		conn.Close()
		return nil, errors.New("no keyring running")
	}
	var out dbus.Variant
	var session dbus.ObjectPath
	if err := conn.Object(ssName, ssPath).Call(ssService+".OpenSession", 0, "plain", dbus.MakeVariant("")).Store(&out, &session); err != nil {
		conn.Close()
		return nil, err
	}
	var locked bool
	if v, err := conn.Object(ssName, ssDefault).GetProperty("org.freedesktop.Secret.Collection.Locked"); err != nil {
		conn.Close()
		return nil, err
	} else {
		locked, _ = v.Value().(bool)
	}
	if locked {
		conn.Close()
		return nil, ErrLocked
	}
	return &keyring{conn: conn, session: session}, nil
}

func attrs(id string) map[string]string {
	return map[string]string{"application": "whiterose", "whiterose-secret": id}
}

func (k *keyring) find(id string) (dbus.ObjectPath, error) {
	var unlocked, locked []dbus.ObjectPath
	if err := k.conn.Object(ssName, ssPath).Call(ssService+".SearchItems", 0, attrs(id)).Store(&unlocked, &locked); err != nil {
		return "", err
	}
	if len(unlocked) == 0 {
		if len(locked) > 0 {
			return "", ErrLocked
		}
		return "", ErrNoSecret
	}
	return unlocked[0], nil
}

func (k *keyring) Get(id string) (string, error) {
	item, err := k.find(id)
	if err != nil {
		return "", err
	}
	var s ssSecret
	if err := k.conn.Object(ssName, item).Call("org.freedesktop.Secret.Item.GetSecret", 0, k.session).Store(&s); err != nil {
		return "", err
	}
	return string(s.Value), nil
}

func (k *keyring) Set(id, value string) error {
	props := map[string]dbus.Variant{
		"org.freedesktop.Secret.Item.Label":      dbus.MakeVariant("WhiteRose calendar link"),
		"org.freedesktop.Secret.Item.Attributes": dbus.MakeVariant(attrs(id)),
	}
	secret := ssSecret{Session: k.session, Value: []byte(value), ContentType: "text/plain; charset=utf8"}
	var item, prompt dbus.ObjectPath
	if err := k.conn.Object(ssName, ssDefault).Call("org.freedesktop.Secret.Collection.CreateItem", 0, props, secret, true).Store(&item, &prompt); err != nil {
		return err
	}
	if prompt != "/" {
		return errors.New("keyring wants to prompt (locked)")
	}
	return nil
}

func (k *keyring) Delete(id string) error {
	item, err := k.find(id)
	if errors.Is(err, ErrNoSecret) {
		return nil
	}
	if err != nil {
		return err
	}
	var prompt dbus.ObjectPath
	return k.conn.Object(ssName, item).Call("org.freedesktop.Secret.Item.Delete", 0).Store(&prompt)
}

// MoveToKeyring moves secrets kept in the fallback (a session that had no
// keyring) into the keyring now that there is one. Returns how many moved.
func (s *Store) MoveToKeyring(k Secrets) (int, error) {
	fb := dbSecrets{s.db}
	ids, err := fb.ids()
	if err != nil {
		return 0, err
	}
	moved := 0
	for _, id := range ids {
		v, err := fb.Get(id)
		if err != nil {
			continue
		}
		// Already in the keyring (kept while it was locked): the keyring's
		// stays, since it's the one things were made with (Files' key).
		if _, err := k.Get(id); err == nil {
			_ = fb.Delete(id)
			continue
		}
		if err := k.Set(id, v); err != nil {
			return moved, fmt.Errorf("moving a secret to the keyring: %w", err)
		}
		if got, err := k.Get(id); err != nil || got != v {
			return moved, errors.New("keyring didn't keep the secret; left it in the database")
		}
		_ = fb.Delete(id)
		moved++
	}
	return moved, nil
}

// Resolver finds a secret wherever it is: the keyring first, then the
// fallback. A keyring that wasn't usable at login (not running yet, or
// locked after a fingerprint login) is tried again later: when a secret is
// wanted, and whenever WatchKeyring says it may have changed.
type Resolver struct {
	Keyring  Secrets // may be nil
	Fallback Secrets
	// Open connects to the keyring (Keyring); nil: only Keyring is used.
	Open func() (Secrets, error)
	// Opened runs (once) when the keyring opens after the start.
	Opened func(Secrets)

	mu      sync.Mutex
	tried   time.Time
	openErr error // why the keyring isn't open
}

// retryEvery is how often a wanted secret tries a keyring that wouldn't
// open (each try is a few D-Bus calls).
const retryEvery = 30 * time.Second

// keyring is the open keyring, opening it if it's time to try again (or
// `now`); nil and why if there isn't one.
func (r *Resolver) keyring(now bool) (Secrets, error) {
	r.mu.Lock()
	if r.Keyring != nil || r.Open == nil {
		k := r.Keyring
		r.mu.Unlock()
		return k, nil
	}
	if !now && time.Since(r.tried) < retryEvery {
		err := r.openErr
		r.mu.Unlock()
		return nil, err
	}
	r.tried = time.Now()
	k, err := r.Open()
	if err != nil {
		r.openErr = err
		r.mu.Unlock()
		return nil, err
	}
	r.Keyring, r.openErr = k, nil
	opened := r.Opened
	r.mu.Unlock()
	if opened != nil {
		opened(k)
	}
	return k, nil
}

// Retry tries the keyring again now (WatchKeyring saw it change); whether
// it's open.
func (r *Resolver) Retry() bool {
	k, _ := r.keyring(true)
	return k != nil
}

// Closed is why the keyring isn't open (nil when it is, or when there's
// nothing to open).
func (r *Resolver) Closed() error {
	r.mu.Lock()
	defer r.mu.Unlock()
	if r.Keyring != nil {
		return nil
	}
	return r.openErr
}

// Get is the secret `id`. ErrLocked: it may be in the keyring, which is
// locked, and it isn't in the fallback.
func (r *Resolver) Get(id string) (string, error) {
	k, kerr := r.keyring(false)
	if k != nil {
		v, err := k.Get(id)
		if err == nil {
			return v, nil
		}
		kerr = err
	}
	v, err := r.Fallback.Get(id)
	if errors.Is(err, ErrNoSecret) && errors.Is(kerr, ErrLocked) {
		return "", ErrLocked
	}
	return v, err
}

// Set stores in the keyring when there is one, else the fallback.
func (r *Resolver) Set(id, value string) (where string, err error) {
	if k, _ := r.keyring(false); k != nil {
		if err := k.Set(id, value); err == nil {
			_ = r.Fallback.Delete(id)
			return k.Name(), nil
		}
	}
	return r.Fallback.Name(), r.Fallback.Set(id, value)
}

func (r *Resolver) Delete(id string) {
	if k, _ := r.keyring(false); k != nil {
		_ = k.Delete(id)
	}
	_ = r.Fallback.Delete(id)
}

// WatchKeyring calls `changed` whenever the keyring may have become usable:
// the Secret Service starting, or a collection changing (unlocked). Runs
// until the session bus goes.
func WatchKeyring(changed func()) error {
	conn, err := dbus.ConnectSessionBus()
	if err != nil {
		return err
	}
	for _, m := range [][]dbus.MatchOption{
		{dbus.WithMatchInterface("org.freedesktop.DBus"), dbus.WithMatchMember("NameOwnerChanged"), dbus.WithMatchArg(0, ssName)},
		{dbus.WithMatchInterface("org.freedesktop.DBus.Properties"), dbus.WithMatchMember("PropertiesChanged"), dbus.WithMatchPathNamespace(ssPath)},
		{dbus.WithMatchInterface(ssService), dbus.WithMatchMember("CollectionChanged")},
	} {
		if err := conn.AddMatchSignal(m...); err != nil {
			conn.Close()
			return err
		}
	}
	ch := make(chan *dbus.Signal, 16)
	conn.Signal(ch)
	go func() {
		for range ch {
			changed()
		}
	}()
	return nil
}

// UnlockKeyring unlocks the default keyring with the user's password (typed
// at the lock screen), if it's locked. GNOME Keyring only: it's the one with
// a way to do that without its own prompt.
func UnlockKeyring(password string) error {
	conn, err := dbus.ConnectSessionBus()
	if err != nil {
		return err
	}
	defer conn.Close()
	svc := conn.Object(ssName, ssPath)
	var coll dbus.ObjectPath
	if err := svc.Call(ssService+".ReadAlias", 0, "default").Store(&coll); err != nil || coll == "/" {
		return errors.New("no default keyring")
	}
	locked := func() bool {
		v, err := conn.Object(ssName, coll).GetProperty("org.freedesktop.Secret.Collection.Locked")
		b, _ := v.Value().(bool)
		return err != nil || b
	}
	if !locked() {
		return nil
	}
	var out dbus.Variant
	var session dbus.ObjectPath
	if err := svc.Call(ssService+".OpenSession", 0, "plain", dbus.MakeVariant("")).Store(&out, &session); err != nil {
		return err
	}
	defer conn.Object(ssName, session).Call("org.freedesktop.Secret.Session.Close", 0)
	secret := ssSecret{Session: session, Parameters: []byte{}, Value: []byte(password), ContentType: "text/plain"}
	if err := svc.Call("org.gnome.keyring.InternalUnsupportedGuiltRiddenInterface.UnlockWithMasterPassword", 0, coll, secret).Err; err != nil {
		return err
	}
	if locked() {
		return errors.New("the keyring didn't take the password")
	}
	return nil
}
