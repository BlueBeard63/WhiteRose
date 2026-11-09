// Package accounts manages user accounts through AccountsService
// (org.freedesktop.Accounts, on Ubuntu and Fedora): names, pictures,
// passwords, administrator rights, adding and removing people.
//
// Changes go through polkit. Calls allow interactive authorization, so the
// session's polkit agent (the WhiteRose shell) asks for a password when
// one is needed; that can take as long as the person takes to type.
package accounts

import (
	"errors"
	"fmt"
	"regexp"
	"sort"
	"strings"
	"time"

	"github.com/godbus/dbus/v5"
)

const (
	busName  = "org.freedesktop.Accounts"
	basePath = "/org/freedesktop/Accounts"
	userIfc  = "org.freedesktop.Accounts.User"
)

// User is an account as the Users page shows it.
type User struct {
	UID       uint64 `json:"uid"`
	Name      string `json:"name"`
	RealName  string `json:"real_name"`
	Icon      string `json:"icon"`
	Admin     bool   `json:"admin"`
	Locked    bool   `json:"locked"`
	Me        bool   `json:"me"`
	LastLogin int64  `json:"last_login"`
}

// Service talks to AccountsService on the system bus.
type Service struct {
	conn *dbus.Conn
	me   uint64
}

// New connects; `me` is this session's user.
func New(me uint64) (*Service, error) {
	conn, err := dbus.ConnectSystemBus()
	if err != nil {
		return nil, err
	}
	return &Service{conn: conn, me: me}, nil
}

// Close drops the connection.
func (s *Service) Close() { _ = s.conn.Close() }

func (s *Service) accounts() dbus.BusObject { return s.conn.Object(busName, basePath) }

func (s *Service) userPath(uid uint64) (dbus.ObjectPath, error) {
	var p dbus.ObjectPath
	err := s.accounts().Call(busName+".FindUserById", 0, int64(uid)).Store(&p)
	return p, err
}

// call runs a method on a user, letting polkit ask for a password.
func (s *Service) call(uid uint64, method string, args ...any) error {
	p, err := s.userPath(uid)
	if err != nil {
		return err
	}
	return friendly(s.conn.Object(busName, p).Call(userIfc+"."+method, dbus.FlagAllowInteractiveAuthorization, args...).Err)
}

// friendly turns polkit refusals into words.
func friendly(err error) error {
	var e dbus.Error
	if errors.As(err, &e) {
		switch {
		case strings.HasSuffix(e.Name, "PermissionDenied"), strings.Contains(e.Name, "NotAuthorized"):
			return errors.New("not allowed (wrong password, or cancelled)")
		}
		if len(e.Body) > 0 {
			if msg, ok := e.Body[0].(string); ok && msg != "" {
				return errors.New(msg)
			}
		}
	}
	return err
}

// List returns the people on this computer (not system accounts), you first.
func (s *Service) List() ([]User, error) {
	var paths []dbus.ObjectPath
	if err := s.accounts().Call(busName+".ListCachedUsers", 0).Store(&paths); err != nil {
		return nil, err
	}
	// You, even if AccountsService hasn't cached you.
	if mine, err := s.userPath(s.me); err == nil && !containsPath(paths, mine) {
		paths = append(paths, mine)
	}
	var users []User
	for _, p := range paths {
		var props map[string]dbus.Variant
		if err := s.conn.Object(busName, p).Call("org.freedesktop.DBus.Properties.GetAll", 0, userIfc).Store(&props); err != nil {
			continue
		}
		u := fromProps(props)
		if sys, _ := props["SystemAccount"].Value().(bool); sys && u.UID != s.me {
			continue
		}
		u.Me = u.UID == s.me
		users = append(users, u)
	}
	sort.SliceStable(users, func(i, j int) bool {
		if users[i].Me != users[j].Me {
			return users[i].Me
		}
		return strings.ToLower(users[i].displayName()) < strings.ToLower(users[j].displayName())
	})
	return users, nil
}

func containsPath(ps []dbus.ObjectPath, p dbus.ObjectPath) bool {
	for _, x := range ps {
		if x == p {
			return true
		}
	}
	return false
}

func (u User) displayName() string {
	if u.RealName != "" {
		return u.RealName
	}
	return u.Name
}

func fromProps(p map[string]dbus.Variant) User {
	str := func(k string) string { v, _ := p[k].Value().(string); return v }
	var u User
	u.Name, u.RealName, u.Icon = str("UserName"), str("RealName"), str("IconFile")
	u.UID, _ = p["Uid"].Value().(uint64)
	t, _ := p["AccountType"].Value().(int32)
	u.Admin = t == 1
	u.Locked, _ = p["Locked"].Value().(bool)
	if lt, ok := p["LoginTime"].Value().(int64); ok {
		u.LastLogin = lt
	}
	return u
}

var (
	userName = regexp.MustCompile(`^[a-z_][a-z0-9_-]{0,31}$`)
	reserved = map[string]bool{"root": true, "daemon": true, "bin": true, "sys": true, "nobody": true, "admin": true, "sudo": true, "wheel": true}
)

// CheckUserName says why a new username can't be used, or nil.
func CheckUserName(name string) error {
	switch {
	case !userName.MatchString(name):
		return errors.New("usernames are lowercase letters, digits, - and _, starting with a letter (at most 32)")
	case reserved[name]:
		return fmt.Errorf("%q is reserved", name)
	}
	return nil
}

func checkRealName(name string) error {
	if len(name) > 255 || strings.ContainsAny(name, ":\n\r") {
		return errors.New("names can't contain : or line breaks (at most 255 characters)")
	}
	return nil
}

// SetRealName changes someone's full name.
func (s *Service) SetRealName(uid uint64, name string) error {
	name = strings.TrimSpace(name)
	if err := checkRealName(name); err != nil {
		return err
	}
	return s.call(uid, "SetRealName", name)
}

// SetIcon sets someone's picture from an image file (AccountsService keeps
// its own copy); "" removes it.
func (s *Service) SetIcon(uid uint64, path string) error {
	return s.call(uid, "SetIconFile", path)
}

// SetAdmin makes someone an administrator or a standard user. You can't
// remove your own administrator rights here (and lock yourself out).
func (s *Service) SetAdmin(uid uint64, admin bool) error {
	if uid == s.me && !admin {
		return errors.New("you can't remove your own administrator rights")
	}
	t := int32(0)
	if admin {
		t = 1
	}
	return s.call(uid, "SetAccountType", t)
}

// SetPassword sets a new password (hashed here, never sent in the clear).
func (s *Service) SetPassword(uid uint64, password string) error {
	if password == "" {
		return errors.New("the password can't be empty")
	}
	hash, err := Crypt(password)
	if err != nil {
		return err
	}
	return s.call(uid, "SetPassword", hash, "")
}

// Create adds someone, with a password.
func (s *Service) Create(name, realName string, admin bool, password string) (User, error) {
	if err := CheckUserName(name); err != nil {
		return User{}, err
	}
	if err := checkRealName(realName); err != nil {
		return User{}, err
	}
	if password == "" {
		return User{}, errors.New("a new account needs a password")
	}
	t := int32(0)
	if admin {
		t = 1
	}
	var p dbus.ObjectPath
	if err := friendly(s.accounts().Call(busName+".CreateUser", dbus.FlagAllowInteractiveAuthorization, name, strings.TrimSpace(realName), t).Store(&p)); err != nil {
		return User{}, err
	}
	var props map[string]dbus.Variant
	// The account exists now; its properties may take a moment to appear.
	for i := 0; i < 20; i++ {
		if err := s.conn.Object(busName, p).Call("org.freedesktop.DBus.Properties.GetAll", 0, userIfc).Store(&props); err == nil {
			break
		}
		time.Sleep(50 * time.Millisecond)
	}
	u := fromProps(props)
	return u, s.SetPassword(u.UID, password)
}

// Delete removes someone; keepFiles leaves their home folder.
func (s *Service) Delete(uid uint64, keepFiles bool) error {
	if uid == s.me {
		return errors.New("you can't remove your own account")
	}
	return friendly(s.accounts().Call(busName+".DeleteUser", dbus.FlagAllowInteractiveAuthorization, int64(uid), !keepFiles).Err)
}
