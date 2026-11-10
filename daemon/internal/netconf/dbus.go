package netconf

import (
	"errors"
	"fmt"

	"github.com/godbus/dbus/v5"
)

const (
	nm        = "org.freedesktop.NetworkManager"
	nmSetting = "org.freedesktop.NetworkManager.Settings"
	nmConn    = "org.freedesktop.NetworkManager.Settings.Connection"
)

func connection(conn *dbus.Conn, uuid string) (dbus.BusObject, error) {
	var path dbus.ObjectPath
	if err := conn.Object(nm, "/org/freedesktop/NetworkManager/Settings").Call(nmSetting+".GetConnectionByUuid", 0, uuid).Store(&path); err != nil {
		return nil, fmt.Errorf("no saved network with that id")
	}
	return conn.Object(nm, path), nil
}

// Details is what's live on the connection right now (Details tab).
type Details struct {
	Device   string   `json:"device"`
	MAC      string   `json:"mac"`
	Speed    uint32   `json:"speed"` // Mb/s
	IPv4     []string `json:"ipv4"`
	Gateway4 string   `json:"gateway4"`
	DNS      []string `json:"dns"`
	IPv6     []string `json:"ipv6"`
	Active   bool     `json:"active"`
}

// Get reads a profile and, if it's active, its live details.
func Get(conn *dbus.Conn, uuid string) (Profile, Details, error) {
	c, err := connection(conn, uuid)
	if err != nil {
		return Profile{}, Details{}, err
	}
	var s Settings
	if err := c.Call(nmConn+".GetSettings", 0).Store(&s); err != nil {
		return Profile{}, Details{}, err
	}
	return Read(s), details(conn, uuid), nil
}

func prop[T any](o dbus.BusObject, iface, name string, def T) T {
	v, err := o.GetProperty(iface + "." + name)
	if err != nil {
		return def
	}
	if x, ok := v.Value().(T); ok {
		return x
	}
	return def
}

func details(conn *dbus.Conn, uuid string) Details {
	var d Details
	root := conn.Object(nm, "/org/freedesktop/NetworkManager")
	for _, ap := range prop(root, nm, "ActiveConnections", []dbus.ObjectPath(nil)) {
		ac := conn.Object(nm, ap)
		if prop(ac, nm+".Connection.Active", "Uuid", "") != uuid {
			continue
		}
		d.Active = true
		if devs := prop(ac, nm+".Connection.Active", "Devices", []dbus.ObjectPath(nil)); len(devs) > 0 {
			dev := conn.Object(nm, devs[0])
			d.Device = prop(dev, nm+".Device", "Interface", "")
			d.MAC = prop(dev, nm+".Device.Wireless", "HwAddress", prop(dev, nm+".Device.Wired", "HwAddress", ""))
			d.Speed = prop(dev, nm+".Device.Wireless", "Bitrate", uint32(0)) / 1000
			if d.Speed == 0 {
				d.Speed = prop(dev, nm+".Device.Wired", "Speed", uint32(0))
			}
		}
		if p := prop(ac, nm+".Connection.Active", "Ip4Config", dbus.ObjectPath("/")); p != "/" {
			cfg := conn.Object(nm, p)
			for _, a := range prop(cfg, nm+".IP4Config", "AddressData", []map[string]dbus.Variant(nil)) {
				addr, _ := a["address"].Value().(string)
				prefix, _ := a["prefix"].Value().(uint32)
				d.IPv4 = append(d.IPv4, fmt.Sprintf("%s/%d", addr, prefix))
			}
			d.Gateway4 = prop(cfg, nm+".IP4Config", "Gateway", "")
			for _, n := range prop(cfg, nm+".IP4Config", "NameserverData", []map[string]dbus.Variant(nil)) {
				if a, ok := n["address"].Value().(string); ok {
					d.DNS = append(d.DNS, a)
				}
			}
		}
		if p := prop(ac, nm+".Connection.Active", "Ip6Config", dbus.ObjectPath("/")); p != "/" {
			for _, a := range prop(conn.Object(nm, p), nm+".IP6Config", "AddressData", []map[string]dbus.Variant(nil)) {
				addr, _ := a["address"].Value().(string)
				prefix, _ := a["prefix"].Value().(uint32)
				d.IPv6 = append(d.IPv6, fmt.Sprintf("%s/%d", addr, prefix))
			}
		}
		break
	}
	return d
}

// Update applies changes to a saved profile and, if it's in use, re-applies
// it so they take effect now.
func Update(conn *dbus.Conn, uuid string, ch Changes) error {
	c, err := connection(conn, uuid)
	if err != nil {
		return err
	}
	var s Settings
	if err := c.Call(nmConn+".GetSettings", 0).Store(&s); err != nil {
		return err
	}
	// Keep the stored secrets: Update replaces the whole profile.
	if _, wifi := s["802-11-wireless-security"]; wifi && ch.Password == nil {
		var secrets Settings
		if err := c.Call(nmConn+".GetSecrets", 0, "802-11-wireless-security").Store(&secrets); err == nil {
			for k, v := range secrets["802-11-wireless-security"] {
				s["802-11-wireless-security"][k] = v
			}
		} else if get(s, "802-11-wireless-security", "psk-flags", uint32(0)) == 0 &&
			(get(s, "802-11-wireless-security", "key-mgmt", "") == "wpa-psk" || get(s, "802-11-wireless-security", "key-mgmt", "") == "sae") {
			// The password is stored by NetworkManager itself and we couldn't
			// read it: saving now would lose it.
			return errors.New("couldn't read the saved password, so nothing was changed (type it again to save)")
		}
	}
	// And an Enterprise network's (its password, or its key's password).
	if _, eap := s["802-1x"]; eap {
		var secrets Settings
		if err := c.Call(nmConn+".GetSecrets", 0, "802-1x").Store(&secrets); err == nil {
			for k, v := range secrets["802-1x"] {
				s["802-1x"][k] = v
			}
		}
	}
	// The deprecated forms of addresses and routes duplicate address-data
	// and route-data (which NetworkManager always sends too), and their
	// struct types don't survive the round trip when empty.
	for _, g := range []string{"ipv4", "ipv6"} {
		del(s, g, "addresses")
		del(s, g, "routes")
	}
	if err := Apply(s, ch); err != nil {
		return err
	}
	if err := c.Call(nmConn+".Update", 0, s).Err; err != nil {
		return err
	}
	// Reapply on the device using it, if any.
	d := details(conn, uuid)
	if d.Active && d.Device != "" {
		var dev dbus.ObjectPath
		if err := conn.Object(nm, "/org/freedesktop/NetworkManager").Call(nm+".GetDeviceByIpIface", 0, d.Device).Store(&dev); err == nil {
			_ = conn.Object(nm, dev).Call(nm+".Device.Reapply", 0, s, uint64(0), uint32(0)).Err
		}
	}
	return nil
}

// JoinEnterprise saves and connects to an Enterprise network on the Wi-Fi
// device (the first one, if there are several).
func JoinEnterprise(conn *dbus.Conn, ssid string, c EAPChange) error {
	s, err := NewEnterprise(ssid, c)
	if err != nil {
		return err
	}
	root := conn.Object(nm, "/org/freedesktop/NetworkManager")
	var devs []dbus.ObjectPath
	if err := root.Call(nm+".GetDevices", 0).Store(&devs); err != nil {
		return err
	}
	var wifi dbus.ObjectPath
	for _, d := range devs {
		if prop(conn.Object(nm, d), nm+".Device", "DeviceType", uint32(0)) == 2 { // NM_DEVICE_TYPE_WIFI
			wifi = d
			break
		}
	}
	if wifi == "" {
		return errors.New("no Wi-Fi device")
	}
	var path, active dbus.ObjectPath
	return root.Call(nm+".AddAndActivateConnection", 0, s, wifi, dbus.ObjectPath("/")).Store(&path, &active)
}
