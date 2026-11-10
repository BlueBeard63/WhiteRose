package netconf

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

// An eduroam-style profile as NetworkManager returns it (password merged in).
func enterprise() Settings {
	return Settings{
		"connection":               {"id": v("BU-WiFi"), "uuid": v("u-2"), "type": v("802-11-wireless")},
		"802-11-wireless":          {"ssid": v([]byte("BU-WiFi")), "mode": v("infrastructure")},
		"802-11-wireless-security": {"key-mgmt": v("wpa-eap")},
		"802-1x": {
			"eap": v([]string{"peap"}), "identity": v("s1234567@bournemouth.ac.uk"), "phase2-auth": v("mschapv2"),
			"ca-cert": v(certToNM("/etc/ssl/certs/ca.pem")), "domain-suffix-match": v("bournemouth.ac.uk"),
			"password": v("secret"), "password-flags": v(uint32(0)),
		},
		"ipv4": {"method": v("auto")},
		"ipv6": {"method": v("auto")},
	}
}

func TestReadAnEnterpriseProfile(t *testing.T) {
	p := Read(enterprise())
	if p.Security != "wpa-eap" || p.EAP == nil {
		t.Fatalf("%+v", p)
	}
	e := *p.EAP
	if e.Method != "peap" || e.Phase2 != "mschapv2" || e.Identity != "s1234567@bournemouth.ac.uk" ||
		e.CACert != "/etc/ssl/certs/ca.pem" || e.Domain != "bournemouth.ac.uk" || !e.HasPassword {
		t.Fatalf("%+v", e)
	}
}

func TestEditingEnterpriseKeepsThePassword(t *testing.T) {
	s := enterprise()
	err := Apply(s, Changes{EAP: &EAPChange{EAP: EAP{Method: "ttls", Phase2: "pap", Identity: "jack", SystemCA: true}}})
	if err != nil {
		t.Fatal(err)
	}
	g := s["802-1x"]
	if g["password"].Value() != "secret" || g["phase2-auth"].Value() != "pap" || g["system-ca-certs"].Value() != true {
		t.Fatalf("%v", g)
	}
	if _, ok := g["ca-cert"]; ok {
		t.Error("the old CA file is kept after choosing the system's")
	}
	// A new password replaces it, saved by NetworkManager.
	pw := "new"
	if err := Apply(s, Changes{EAP: &EAPChange{EAP: EAP{Method: "peap", Phase2: "mschapv2", Identity: "jack"}, Password: &pw}}); err != nil {
		t.Fatal(err)
	}
	if s["802-1x"]["password"].Value() != "new" || s["802-1x"]["password-flags"].Value() != uint32(0) {
		t.Fatalf("%v", s["802-1x"])
	}
}

func TestKeyringPasswordCountsAsSavedAndOtherKeysAreKept(t *testing.T) {
	s := enterprise()
	delete(s["802-1x"], "password") // GetSettings never sends it
	s["802-1x"]["password-flags"] = v(uint32(1))
	s["802-1x"]["phase1-peapver"] = v("0")
	s["802-1x"]["auth-timeout"] = v(int32(40)) // set elsewhere: not the form's
	if e := Read(s).EAP; !e.HasPassword || e.PeapVersion != "0" {
		t.Fatalf("%+v", e)
	}
	if err := Apply(s, Changes{EAP: &EAPChange{EAP: EAP{Method: "peap", Phase2: "mschapv2", Identity: "s5732062", Domain: "bournemouth.ac.uk"}}}); err != nil {
		t.Fatal(err)
	}
	g := s["802-1x"]
	if g["auth-timeout"].Value() != int32(40) || g["password-flags"].Value() != uint32(1) {
		t.Errorf("lost keys: %v", g)
	}
	if _, ok := g["phase1-peapver"]; ok {
		t.Error("PEAP version kept after choosing automatic")
	}
	s["802-1x"]["password-flags"] = v(uint32(2))
	if Read(s).EAP.HasPassword {
		t.Error("a password marked not saved counts as saved")
	}
}

func TestEnterpriseChecks(t *testing.T) {
	bad := []EAP{
		{Method: "peap", Phase2: "pap", Identity: "x"},                        // PAP isn't a PEAP inner method
		{Method: "peap", Phase2: "mschapv2"},                                  // no username
		{Method: "tls", Identity: "x"},                                        // no certificate
		{Method: "leap", Identity: "x"},                                       // not offered
		{Method: "peap", Phase2: "mschapv2", Identity: "x", CACert: "ca.pem"}, // not a full path
		{Method: "peap", Phase2: "mschapv2", Identity: "x", CACert: "/no/such/ca.pem"},
	}
	for _, e := range bad {
		if err := Apply(enterprise(), Changes{EAP: &EAPChange{EAP: e}}); err == nil {
			t.Errorf("accepted %+v", e)
		}
	}
}

func TestSwitchingToAPasswordNetworkDropsTheSignIn(t *testing.T) {
	s := enterprise()
	sec := "wpa-psk"
	if err := Apply(s, Changes{Security: &sec}); err != nil {
		t.Fatal(err)
	}
	if _, ok := s["802-1x"]; ok {
		t.Error("802-1x kept")
	}
}

func TestNewEnterpriseNetwork(t *testing.T) {
	ca := filepath.Join(t.TempDir(), "ca.pem")
	os.WriteFile(ca, []byte("cert"), 0o644)
	pw := "pw"
	s, err := NewEnterprise("eduroam", EAPChange{EAP: EAP{Method: "peap", Phase2: "mschapv2", Identity: "me@uni.ac.uk", CACert: ca}, Password: &pw})
	if err != nil {
		t.Fatal(err)
	}
	if string(s["802-11-wireless"]["ssid"].Value().([]byte)) != "eduroam" || s["802-11-wireless-security"]["key-mgmt"].Value() != "wpa-eap" {
		t.Fatalf("%v", s)
	}
	if got := string(s["802-1x"]["ca-cert"].Value().([]byte)); !strings.HasPrefix(got, "file://"+ca) || !strings.HasSuffix(got, "\x00") {
		t.Errorf("ca-cert %q", got)
	}
	if s["802-1x"]["password"].Value() != "pw" {
		t.Error("password")
	}
}
