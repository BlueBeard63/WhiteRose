package store

import (
	"crypto/rand"
	"database/sql"
	"encoding/hex"
	"errors"
	"fmt"

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
		return nil, errors.New("keyring is locked")
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
			return "", errors.New("keyring is locked")
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

// Resolver finds a secret wherever it is: the keyring first, then the fallback.
type Resolver struct {
	Keyring  Secrets // may be nil
	Fallback Secrets
}

func (r Resolver) Get(id string) (string, error) {
	if r.Keyring != nil {
		if v, err := r.Keyring.Get(id); err == nil {
			return v, nil
		}
	}
	return r.Fallback.Get(id)
}

// Set stores in the keyring when there is one, else the fallback.
func (r Resolver) Set(id, value string) (where string, err error) {
	if r.Keyring != nil {
		if err := r.Keyring.Set(id, value); err == nil {
			_ = r.Fallback.Delete(id)
			return r.Keyring.Name(), nil
		}
	}
	return r.Fallback.Name(), r.Fallback.Set(id, value)
}

func (r Resolver) Delete(id string) {
	if r.Keyring != nil {
		_ = r.Keyring.Delete(id)
	}
	_ = r.Fallback.Delete(id)
}
