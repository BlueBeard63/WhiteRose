// Package netconf reads and edits NetworkManager connection profiles for the
// Network settings dialog, in a simple shape (security, IPv4/IPv6, DNS, MAC
// privacy...), converting to NetworkManager's exact D-Bus types: IPv4 DNS as
// network-order integers, IPv6 DNS as 16-byte addresses, prefixes as uint32.
// Saving merges the changes into the full profile and keeps its stored
// secrets, so editing DNS never loses the Wi-Fi password.
package netconf

import (
	"encoding/binary"
	"errors"
	"fmt"
	"net/netip"
	"strings"

	"github.com/godbus/dbus/v5"
)

// Settings is NetworkManager's a{sa{sv}}.
type Settings = map[string]map[string]dbus.Variant

type Address struct {
	Address string `json:"address"`
	Prefix  uint32 `json:"prefix"`
}

type IP struct {
	Method        string    `json:"method"` // auto | manual | link-local | disabled (v4); auto | dhcp | manual | ignore | disabled (v6)
	Addresses     []Address `json:"addresses"`
	Gateway       string    `json:"gateway"`
	DNS           []string  `json:"dns"`
	IgnoreAutoDNS bool      `json:"ignore_auto_dns"`
}

// Profile is a connection as the dialog shows it.
type Profile struct {
	UUID        string `json:"uuid"`
	Name        string `json:"name"`
	Type        string `json:"type"` // 802-11-wireless | 802-3-ethernet | ...
	Autoconnect bool   `json:"autoconnect"`
	Metered     string `json:"metered"` // auto | yes | no
	Zone        string `json:"zone"`    // firewall zone ("" = default)
	// Wi-Fi only.
	SSID     string `json:"ssid,omitempty"`
	Security string `json:"security,omitempty"` // none | wpa-psk | sae | wpa-eap | owe | enhanced-open
	MAC      string `json:"mac,omitempty"`      // cloned MAC policy: default | permanent | random | stable
	PMF      string `json:"pmf,omitempty"`      // default | disable | optional | required
	IPv4     IP     `json:"ipv4"`
	IPv6     IP     `json:"ipv6"`
}

// Changes are the fields the dialog edited (nil = unchanged).
type Changes struct {
	Name        *string `json:"name"`
	Autoconnect *bool   `json:"autoconnect"`
	Metered     *string `json:"metered"`
	Zone        *string `json:"zone"`
	Security    *string `json:"security"`
	Password    *string `json:"password"`
	MAC         *string `json:"mac"`
	PMF         *string `json:"pmf"`
	IPv4        *IP     `json:"ipv4"`
	IPv6        *IP     `json:"ipv6"`
}

func get[T any](s Settings, group, key string, def T) T {
	if g, ok := s[group]; ok {
		if v, ok := g[key]; ok {
			if x, ok := v.Value().(T); ok {
				return x
			}
		}
	}
	return def
}

func set(s Settings, group, key string, v any) {
	if s[group] == nil {
		s[group] = map[string]dbus.Variant{}
	}
	s[group][key] = dbus.MakeVariant(v)
}

func del(s Settings, group, key string) {
	if s[group] != nil {
		delete(s[group], key)
	}
}

var meteredNames = map[int32]string{0: "auto", 1: "yes", 2: "no", 3: "yes", 4: "no"}
var pmfNames = map[int32]string{0: "default", 1: "disable", 2: "optional", 3: "required"}

func invert[K comparable, V comparable](m map[K]V) map[V]K {
	out := map[V]K{}
	for k, v := range m {
		if _, seen := out[v]; !seen {
			out[v] = k
		}
	}
	return out
}

// IPv4 DNS servers are uint32 in network byte order.
func ip4ToNM(s string) (uint32, error) {
	a, err := netip.ParseAddr(strings.TrimSpace(s))
	if err != nil || !a.Is4() {
		return 0, fmt.Errorf("%q isn't an IPv4 address", s)
	}
	b := a.As4()
	return binary.LittleEndian.Uint32(b[:]), nil
}

func ip4FromNM(n uint32) string {
	var b [4]byte
	binary.LittleEndian.PutUint32(b[:], n)
	return netip.AddrFrom4(b).String()
}

func ip6ToNM(s string) ([]byte, error) {
	a, err := netip.ParseAddr(strings.TrimSpace(s))
	if err != nil || !a.Is6() || a.Is4In6() {
		return nil, fmt.Errorf("%q isn't an IPv6 address", s)
	}
	b := a.As16()
	return b[:], nil
}

func readIP(s Settings, group string) IP {
	ip := IP{
		Method:        get(s, group, "method", "auto"),
		Gateway:       get(s, group, "gateway", ""),
		IgnoreAutoDNS: get(s, group, "ignore-auto-dns", false),
		Addresses:     []Address{},
		DNS:           []string{},
	}
	for _, a := range get(s, group, "address-data", []map[string]dbus.Variant(nil)) {
		addr, _ := a["address"].Value().(string)
		prefix, _ := a["prefix"].Value().(uint32)
		ip.Addresses = append(ip.Addresses, Address{Address: addr, Prefix: prefix})
	}
	if group == "ipv4" {
		for _, n := range get(s, group, "dns", []uint32(nil)) {
			ip.DNS = append(ip.DNS, ip4FromNM(n))
		}
	} else {
		for _, b := range get(s, group, "dns", [][]byte(nil)) {
			if a, ok := netip.AddrFromSlice(b); ok {
				ip.DNS = append(ip.DNS, a.String())
			}
		}
	}
	return ip
}

// Read turns NetworkManager settings into a Profile.
func Read(s Settings) Profile {
	p := Profile{
		UUID:        get(s, "connection", "uuid", ""),
		Name:        get(s, "connection", "id", ""),
		Type:        get(s, "connection", "type", ""),
		Autoconnect: get(s, "connection", "autoconnect", true),
		Metered:     meteredNames[get(s, "connection", "metered", int32(0))],
		Zone:        get(s, "connection", "zone", ""),
		IPv4:        readIP(s, "ipv4"),
		IPv6:        readIP(s, "ipv6"),
	}
	if p.Type == "802-11-wireless" {
		p.SSID = string(get(s, "802-11-wireless", "ssid", []byte(nil)))
		// assigned-mac-address is the string form; cloned-mac-address over
		// D-Bus is the raw bytes of a fixed address.
		p.MAC = get(s, "802-11-wireless", "assigned-mac-address", "")
		if p.MAC == "" || p.MAC == "preserve" {
			p.MAC = "default"
		}
		if _, ok := s["802-11-wireless-security"]; ok {
			p.Security = get(s, "802-11-wireless-security", "key-mgmt", "none")
			p.PMF = pmfNames[get(s, "802-11-wireless-security", "pmf", int32(0))]
		} else {
			p.Security = "none"
			p.PMF = "default"
		}
	}
	return p
}

func applyIP(s Settings, group string, ip IP) error {
	valid := map[string]bool{"auto": true, "manual": true, "link-local": true, "disabled": true, "dhcp": true, "ignore": true}
	if !valid[ip.Method] || group == "ipv4" && (ip.Method == "dhcp" || ip.Method == "ignore") || group == "ipv6" && ip.Method == "link-local" {
		return fmt.Errorf("%s: unknown method %q", group, ip.Method)
	}
	if ip.Method == "manual" && len(ip.Addresses) == 0 {
		return fmt.Errorf("%s: a manual setup needs an address", group)
	}
	set(s, group, "method", ip.Method)
	var data []map[string]dbus.Variant
	for _, a := range ip.Addresses {
		addr, err := netip.ParseAddr(strings.TrimSpace(a.Address))
		if err != nil || (group == "ipv4") != addr.Is4() {
			return fmt.Errorf("%s: %q isn't a valid address", group, a.Address)
		}
		max := uint32(32)
		if group == "ipv6" {
			max = 128
		}
		if a.Prefix == 0 || a.Prefix > max {
			return fmt.Errorf("%s: prefix /%d out of range", group, a.Prefix)
		}
		data = append(data, map[string]dbus.Variant{"address": dbus.MakeVariant(addr.String()), "prefix": dbus.MakeVariant(a.Prefix)})
	}
	// address-data replaces the deprecated "addresses"; NetworkManager
	// refuses a profile carrying both that disagree.
	del(s, group, "addresses")
	if ip.Method == "manual" {
		set(s, group, "address-data", data)
	} else {
		del(s, group, "address-data")
	}
	if ip.Gateway != "" && ip.Method == "manual" {
		g, err := netip.ParseAddr(ip.Gateway)
		if err != nil || (group == "ipv4") != g.Is4() {
			return fmt.Errorf("%s: gateway %q isn't valid", group, ip.Gateway)
		}
		set(s, group, "gateway", g.String())
	} else {
		del(s, group, "gateway")
	}
	if group == "ipv4" {
		var dns []uint32
		for _, d := range ip.DNS {
			n, err := ip4ToNM(d)
			if err != nil {
				return err
			}
			dns = append(dns, n)
		}
		set(s, group, "dns", dns)
	} else {
		dns := [][]byte{}
		for _, d := range ip.DNS {
			b, err := ip6ToNM(d)
			if err != nil {
				return err
			}
			dns = append(dns, b)
		}
		set(s, group, "dns", dns)
	}
	del(s, group, "dns-data") // newer duplicate of "dns"
	set(s, group, "ignore-auto-dns", ip.IgnoreAutoDNS)
	return nil
}

// Apply edits full settings (secrets merged in) with the dialog's changes.
func Apply(s Settings, c Changes) error {
	if c.Name != nil {
		if strings.TrimSpace(*c.Name) == "" {
			return errors.New("the name can't be empty")
		}
		set(s, "connection", "id", strings.TrimSpace(*c.Name))
	}
	if c.Autoconnect != nil {
		set(s, "connection", "autoconnect", *c.Autoconnect)
	}
	if c.Metered != nil {
		v, ok := invert(meteredNames)[*c.Metered]
		if !ok {
			return fmt.Errorf("metered must be auto, yes or no")
		}
		set(s, "connection", "metered", v)
	}
	if c.Zone != nil {
		if *c.Zone == "" {
			del(s, "connection", "zone")
		} else {
			set(s, "connection", "zone", *c.Zone)
		}
	}
	wifi := get(s, "connection", "type", "") == "802-11-wireless"
	if (c.Security != nil || c.Password != nil || c.MAC != nil || c.PMF != nil) && !wifi {
		return errors.New("security and MAC settings are for Wi-Fi profiles")
	}
	if c.MAC != nil {
		switch *c.MAC {
		case "default":
			del(s, "802-11-wireless", "assigned-mac-address")
			del(s, "802-11-wireless", "cloned-mac-address")
		case "permanent", "random", "stable":
			set(s, "802-11-wireless", "assigned-mac-address", *c.MAC)
			del(s, "802-11-wireless", "cloned-mac-address")
		default:
			return fmt.Errorf("unknown MAC policy %q", *c.MAC)
		}
	}
	if c.Security != nil {
		switch *c.Security {
		case "none":
			delete(s, "802-11-wireless-security")
			del(s, "802-11-wireless", "security")
		case "wpa-psk", "sae", "owe":
			set(s, "802-11-wireless-security", "key-mgmt", *c.Security)
			set(s, "802-11-wireless", "security", "802-11-wireless-security")
		default:
			return fmt.Errorf("security %q can't be set here", *c.Security)
		}
	}
	if c.PMF != nil {
		v, ok := invert(pmfNames)[*c.PMF]
		if !ok {
			return fmt.Errorf("unknown frame protection %q", *c.PMF)
		}
		if _, has := s["802-11-wireless-security"]; has {
			set(s, "802-11-wireless-security", "pmf", v)
		}
	}
	if c.Password != nil {
		km := get(s, "802-11-wireless-security", "key-mgmt", "")
		if km != "wpa-psk" && km != "sae" {
			return errors.New("this network doesn't use a password")
		}
		if n := len(*c.Password); n < 8 || n > 63 {
			return errors.New("Wi-Fi passwords are 8 to 63 characters")
		}
		set(s, "802-11-wireless-security", "psk", *c.Password)
	}
	if c.IPv4 != nil {
		if err := applyIP(s, "ipv4", *c.IPv4); err != nil {
			return err
		}
	}
	if c.IPv6 != nil {
		if err := applyIP(s, "ipv6", *c.IPv6); err != nil {
			return err
		}
	}
	return nil
}
