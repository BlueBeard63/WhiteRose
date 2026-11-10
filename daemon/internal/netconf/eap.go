package netconf

// Enterprise (802.1X) Wi-Fi: university and work networks (eduroam and
// the like) where you sign in with a username and password, or a
// certificate, instead of one shared password. NetworkManager keeps this in
// its "802-1x" group; certificates are file paths it reads itself.

import (
	"bytes"
	"errors"
	"fmt"
	"os"
	"slices"
	"strings"

	"github.com/godbus/dbus/v5"
)

// EAP is an Enterprise network's sign-in, as the dialog shows it. Secrets
// are never sent back: only whether one is saved.
type EAP struct {
	// peap | ttls | tls | pwd
	Method string `json:"method"`
	// Inner sign-in for PEAP and TTLS: mschapv2 | gtc | md5 (PEAP);
	// pap | chap | mschap | mschapv2 | gtc (TTLS).
	Phase2    string `json:"phase2"`
	Identity  string `json:"identity"`
	Anonymous string `json:"anonymous"`
	// The certificate authority to check the network against: a file, or
	// the system's trusted ones (SystemCA). Neither: not checked.
	CACert   string `json:"ca_cert"`
	SystemCA bool   `json:"system_ca"`
	// The server's name must end with this (e.g. "bournemouth.ac.uk").
	Domain string `json:"domain"`
	// PEAP version: "" (automatic), "0" or "1".
	PeapVersion string `json:"peap_version"`
	// TLS: your certificate and its key.
	ClientCert string `json:"client_cert"`
	PrivateKey string `json:"private_key"`
	// A password is saved (in NetworkManager, or asked for each time).
	HasPassword bool `json:"has_password"`
}

// EAPChange is the dialog's Enterprise sign-in when saving: everything but
// the secrets is replaced; a nil secret keeps the saved one.
type EAPChange struct {
	EAP
	Password    *string `json:"password"`
	KeyPassword *string `json:"key_password"`
}

// The 802-1x keys the form sets (and the secrets it keeps or replaces).
var managed = map[string]bool{
	"eap": true, "identity": true, "anonymous-identity": true, "phase2-auth": true, "phase1-peapver": true,
	"ca-cert": true, "system-ca-certs": true, "domain-suffix-match": true, "client-cert": true, "private-key": true,
	"password": true, "password-flags": true, "private-key-password": true, "private-key-password-flags": true,
}

var phase2s = map[string][]string{
	"peap": {"mschapv2", "gtc", "md5"},
	"ttls": {"pap", "chap", "mschap", "mschapv2", "gtc"},
}

// NetworkManager's form of a certificate path: "file://<path>" and a NUL.
func certToNM(path string) []byte {
	return append([]byte("file://"+path), 0)
}

func certFromNM(b []byte) string {
	b = bytes.TrimRight(b, "\x00")
	if p, ok := strings.CutPrefix(string(b), "file://"); ok {
		return p
	}
	return "" // an embedded certificate (blob): shown as none chosen
}

func readEAP(s Settings) *EAP {
	if _, ok := s["802-1x"]; !ok {
		return nil
	}
	e := &EAP{
		Identity:    get(s, "802-1x", "identity", ""),
		Anonymous:   get(s, "802-1x", "anonymous-identity", ""),
		Domain:      get(s, "802-1x", "domain-suffix-match", ""),
		SystemCA:    get(s, "802-1x", "system-ca-certs", false),
		Phase2:      get(s, "802-1x", "phase2-auth", ""),
		PeapVersion: get(s, "802-1x", "phase1-peapver", ""),
	}
	if m := get(s, "802-1x", "eap", []string(nil)); len(m) > 0 {
		e.Method = m[0]
	}
	e.CACert = certFromNM(get(s, "802-1x", "ca-cert", []byte(nil)))
	e.ClientCert = certFromNM(get(s, "802-1x", "client-cert", []byte(nil)))
	e.PrivateKey = certFromNM(get(s, "802-1x", "private-key", []byte(nil)))
	// Saved, by NetworkManager (flags 0) or by your keyring (1), unless
	// it's marked not saved (2) or not needed (4). GetSettings never
	// includes the secret itself, so the flags are what tell.
	flags := get(s, "802-1x", "password-flags", uint32(0))
	e.HasPassword = get(s, "802-1x", "password", "") != "" || flags&(2|4) == 0
	return e
}

// Checks a certificate path the dialog sent: absolute and readable.
func certFile(what, p string) error {
	if p == "" {
		return nil
	}
	if !strings.HasPrefix(p, "/") {
		return fmt.Errorf("%s: choose the file again (%q isn't a full path)", what, p)
	}
	f, err := os.Open(p)
	if err != nil {
		return fmt.Errorf("%s: can't read %s", what, p)
	}
	f.Close()
	return nil
}

func applyEAP(s Settings, c EAPChange) error {
	m := c.Method
	switch m {
	case "peap", "ttls":
		if !slices.Contains(phase2s[m], c.Phase2) {
			return fmt.Errorf("%s needs an inner sign-in method (%s)", strings.ToUpper(m), strings.Join(phase2s[m], ", "))
		}
	case "tls":
		if c.ClientCert == "" || c.PrivateKey == "" {
			return errors.New("TLS needs your certificate and its private key")
		}
	case "pwd":
	default:
		return fmt.Errorf("unknown Enterprise method %q", m)
	}
	if strings.TrimSpace(c.Identity) == "" {
		return errors.New("Enterprise networks need a username (identity)")
	}
	for _, f := range [][2]string{{"CA certificate", c.CACert}, {"Your certificate", c.ClientCert}, {"Private key", c.PrivateKey}} {
		if err := certFile(f[0], f[1]); err != nil {
			return err
		}
	}
	if c.PeapVersion != "" && c.PeapVersion != "0" && c.PeapVersion != "1" {
		return fmt.Errorf("unknown PEAP version %q", c.PeapVersion)
	}
	// Everything this form sets is replaced; anything else the profile has
	// (set elsewhere, e.g. by GNOME or nmcli) is kept.
	old := s["802-1x"]
	s["802-1x"] = map[string]dbus.Variant{}
	for k, x := range old {
		if !managed[k] {
			s["802-1x"][k] = x
		}
	}
	set(s, "802-1x", "eap", []string{m})
	set(s, "802-1x", "identity", strings.TrimSpace(c.Identity))
	if a := strings.TrimSpace(c.Anonymous); a != "" {
		set(s, "802-1x", "anonymous-identity", a)
	}
	if m == "peap" || m == "ttls" {
		set(s, "802-1x", "phase2-auth", c.Phase2)
	}
	if m == "peap" && c.PeapVersion != "" {
		set(s, "802-1x", "phase1-peapver", c.PeapVersion)
	}
	if c.CACert != "" {
		set(s, "802-1x", "ca-cert", certToNM(c.CACert))
	} else if c.SystemCA {
		set(s, "802-1x", "system-ca-certs", true)
	}
	if d := strings.TrimSpace(c.Domain); d != "" {
		set(s, "802-1x", "domain-suffix-match", d)
	}
	if m == "tls" {
		set(s, "802-1x", "client-cert", certToNM(c.ClientCert))
		set(s, "802-1x", "private-key", certToNM(c.PrivateKey))
	}
	// Secrets: the new one, else the saved one (merged in by Update).
	keep := func(key string, v *string) {
		if v != nil {
			set(s, "802-1x", key, *v)
			set(s, "802-1x", key+"-flags", uint32(0)) // saved by NetworkManager
		} else if old != nil {
			if x, ok := old[key]; ok {
				s["802-1x"][key] = x
			}
			if x, ok := old[key+"-flags"]; ok {
				s["802-1x"][key+"-flags"] = x
			}
		}
	}
	if m != "tls" {
		keep("password", c.Password)
	} else {
		keep("private-key-password", c.KeyPassword)
	}
	set(s, "802-11-wireless-security", "key-mgmt", "wpa-eap")
	set(s, "802-11-wireless", "security", "802-11-wireless-security")
	return nil
}

// NewEnterprise is a new profile for joining an Enterprise network.
func NewEnterprise(ssid string, c EAPChange) (Settings, error) {
	if strings.TrimSpace(ssid) == "" {
		return nil, errors.New("no network name")
	}
	s := Settings{
		"connection":      {"id": variant(ssid), "type": variant("802-11-wireless"), "autoconnect": variant(true)},
		"802-11-wireless": {"ssid": variant([]byte(ssid)), "mode": variant("infrastructure")},
		"ipv4":            {"method": variant("auto")},
		"ipv6":            {"method": variant("auto")},
	}
	if err := applyEAP(s, c); err != nil {
		return nil, err
	}
	return s, nil
}

func variant(x any) dbus.Variant { return dbus.MakeVariant(x) }
