package netconf

import (
	"testing"

	"github.com/godbus/dbus/v5"
)

func v(x any) dbus.Variant { return dbus.MakeVariant(x) }

// A typical saved Wi-Fi profile as NetworkManager returns it (with secrets merged).
func wifi() Settings {
	return Settings{
		"connection":               {"id": v("Home-5G"), "uuid": v("u-1"), "type": v("802-11-wireless"), "autoconnect": v(true), "metered": v(int32(2))},
		"802-11-wireless":          {"ssid": v([]byte("Home-5G")), "mode": v("infrastructure"), "assigned-mac-address": v("stable")},
		"802-11-wireless-security": {"key-mgmt": v("sae"), "pmf": v(int32(3)), "psk": v("hunter2hunter2")},
		"ipv4":                     {"method": v("auto"), "dns": v([]uint32{0x01010101}), "ignore-auto-dns": v(true)},
		"ipv6":                     {"method": v("auto")},
	}
}

func TestReadAWifiProfile(t *testing.T) {
	p := Read(wifi())
	if p.Name != "Home-5G" || p.SSID != "Home-5G" || p.Security != "sae" || p.PMF != "required" || p.MAC != "stable" || p.Metered != "no" {
		t.Fatalf("%+v", p)
	}
	if p.IPv4.Method != "auto" || len(p.IPv4.DNS) != 1 || p.IPv4.DNS[0] != "1.1.1.1" || !p.IPv4.IgnoreAutoDNS {
		t.Fatalf("ipv4: %+v", p.IPv4)
	}
}

func TestDNSUsesNetworkManagersByteOrder(t *testing.T) {
	n, err := ip4ToNM("192.168.1.42")
	if err != nil || ip4FromNM(n) != "192.168.1.42" {
		t.Fatalf("%x %v", n, err)
	}
	// NetworkManager's integers are the address bytes in memory order:
	// 8.8.4.4 -> bytes 08 08 04 04 -> little-endian 0x04040808.
	if n, _ := ip4ToNM("8.8.4.4"); n != 0x04040808 {
		t.Fatalf("got %#x", n)
	}
	if b, err := ip6ToNM("2606:4700:4700::1111"); err != nil || len(b) != 16 || b[0] != 0x26 {
		t.Fatalf("%v %v", b, err)
	}
	for _, bad := range []string{"1.1.1", "::1x", "fe80::1"} {
		if _, err := ip4ToNM(bad); err == nil {
			t.Errorf("%q accepted as IPv4", bad)
		}
	}
}

func TestManualIPv4KeepsTheSecretAndTypes(t *testing.T) {
	s := wifi()
	s["ipv4"]["addresses"] = v([][]uint32{{1, 2, 3}}) // deprecated duplicate
	err := Apply(s, Changes{IPv4: &IP{
		Method:    "manual",
		Addresses: []Address{{Address: "192.168.1.42", Prefix: 24}},
		Gateway:   "192.168.1.1",
		DNS:       []string{"1.1.1.1", "9.9.9.9"},
	}})
	if err != nil {
		t.Fatal(err)
	}
	ad := s["ipv4"]["address-data"].Value().([]map[string]dbus.Variant)
	if ad[0]["address"].Value() != "192.168.1.42" || ad[0]["prefix"].Value() != uint32(24) {
		t.Fatalf("address-data: %v (prefix must be uint32)", ad)
	}
	if _, still := s["ipv4"]["addresses"]; still {
		t.Fatal("deprecated addresses left in place")
	}
	if dns := s["ipv4"]["dns"].Value().([]uint32); len(dns) != 2 || ip4FromNM(dns[1]) != "9.9.9.9" {
		t.Fatalf("dns: %v", dns)
	}
	if s["802-11-wireless-security"]["psk"].Value() != "hunter2hunter2" {
		t.Fatal("editing IPv4 lost the password")
	}
	// And back to automatic: the manual address and gateway go.
	if err := Apply(s, Changes{IPv4: &IP{Method: "auto"}}); err != nil {
		t.Fatal(err)
	}
	if _, ok := s["ipv4"]["address-data"]; ok {
		t.Fatal("manual address kept with automatic setup")
	}
}

func TestSecurityPasswordMACAndValidation(t *testing.T) {
	s := wifi()
	pw := "correct horse battery"
	sec, mac, pmf, metered := "wpa-psk", "random", "optional", "yes"
	if err := Apply(s, Changes{Security: &sec, Password: &pw, MAC: &mac, PMF: &pmf, Metered: &metered}); err != nil {
		t.Fatal(err)
	}
	p := Read(s)
	if p.Security != "wpa-psk" || p.MAC != "random" || p.PMF != "optional" || p.Metered != "yes" || s["802-11-wireless-security"]["psk"].Value() != pw {
		t.Fatalf("%+v", p)
	}
	open := "none"
	if err := Apply(s, Changes{Security: &open}); err != nil || Read(s).Security != "none" {
		t.Fatalf("open: %v %+v", err, Read(s))
	}
	bad := []Changes{
		{Password: &pw},               // open network has no password
		{IPv4: &IP{Method: "manual"}}, // no address
		{IPv4: &IP{Method: "manual", Addresses: []Address{{"10.0.0.1", 40}}}},
		{IPv4: &IP{Method: "manual", Addresses: []Address{{"fe80::1", 64}}}},
		{IPv6: &IP{Method: "link-local"}},
		{IPv4: &IP{Method: "auto", DNS: []string{"not-an-ip"}}},
		{Metered: ptr("maybe")},
		{Name: ptr("  ")},
		{MAC: ptr("sometimes")},
	}
	for i, c := range bad {
		if err := Apply(wifiOpen(), c); err == nil {
			t.Errorf("bad change %d accepted", i)
		}
	}
	short := "short"
	if err := Apply(wifi(), Changes{Password: &short}); err == nil {
		t.Error("7-char password accepted")
	}
	wired := Settings{"connection": {"type": v("802-3-ethernet")}}
	if err := Apply(wired, Changes{MAC: &mac}); err == nil {
		t.Error("Wi-Fi settings accepted on Ethernet")
	}
}

func wifiOpen() Settings {
	s := wifi()
	delete(s, "802-11-wireless-security")
	return s
}

func ptr(s string) *string { return &s }
