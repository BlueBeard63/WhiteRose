// Package audio lists where sound can go and come from, as places a person
// would name ("Speaker", "Headphones", "T34w-30"), not only the PipeWire
// nodes that exist right now. A laptop's sound card often has several
// setups (profiles) that can't run together, e.g. one with Headphones and
// one with the Speaker: the Speaker has no node until its setup is chosen,
// so picking it switches the setup first. Places with nothing plugged in
// (an empty headphone jack, an HDMI port without a screen) are listed but
// marked, because PipeWire won't use them.
//
// It reads `pw-dump` and changes things with `wpctl` (both ship with
// PipeWire and WirePlumber); no D-Bus or native bindings.
package audio

import (
	"bytes"
	"encoding/json"
	"errors"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"slices"
	"sort"
	"strings"
	"time"
)

// Port is one place sound goes to (Out) or comes from (In).
type Port struct {
	// Stable while the card stays: "<device name>#<route name>", or the
	// node name for places without a card route (Bluetooth, virtual).
	Key       string `json:"key"`
	Label     string `json:"label"`
	Direction string `json:"direction"` // "out" or "in"
	// "yes", "no" (nothing plugged in) or "unknown" (can't tell: speakers,
	// built-in microphones).
	Available string `json:"available"`
	// The PipeWire node for it now, "" until its setup is chosen.
	Node string `json:"node,omitempty"`
	// Device and setup to switch to first (Node == "").
	Device  int `json:"device,omitempty"`
	Profile int `json:"profile,omitempty"`
}

type object struct {
	ID   int    `json:"id"`
	Type string `json:"type"`
	Info struct {
		Props  map[string]any `json:"props"`
		Params struct {
			EnumProfile []profile `json:"EnumProfile"`
			Profile     []profile `json:"Profile"`
			EnumRoute   []route   `json:"EnumRoute"`
		} `json:"params"`
	} `json:"info"`
}

type profile struct {
	Index     int    `json:"index"`
	Name      string `json:"name"`
	Available string `json:"available"`
	Priority  int    `json:"priority"`
}

type route struct {
	Index       int    `json:"index"`
	Direction   string `json:"direction"`
	Name        string `json:"name"`
	Description string `json:"description"`
	Available   string `json:"available"`
	Profiles    []int  `json:"profiles"`
	Devices     []int  `json:"devices"`
}

func str(m map[string]any, k string) string {
	s, _ := m[k].(string)
	return s
}

func num(m map[string]any, k string) (int, bool) {
	switch v := m[k].(type) {
	case float64:
		return int(v), true
	case string:
		var n int
		_, err := fmt.Sscan(v, &n)
		return n, err == nil
	}
	return 0, false
}

// Parse turns `pw-dump` output into ports, outputs first, each in the
// card's order. `screens` names the screens with sound (HDMI/DisplayPort):
// with one such screen and one such port in use, the port takes its name.
func Parse(dump []byte, screens []string) ([]Port, error) {
	var objs []object
	if err := json.Unmarshal(dump, &objs); err != nil {
		return nil, fmt.Errorf("reading pw-dump: %w", err)
	}
	devices := map[int]object{}
	type node struct {
		id   int
		name string
		desc string
		dir  string
		dev  int
		at   int // card.profile.device, -1 if none
	}
	var nodes []node
	for _, o := range objs {
		p := o.Info.Props
		switch o.Type {
		case "PipeWire:Interface:Device":
			if str(p, "media.class") == "Audio/Device" {
				devices[o.ID] = o
			}
		case "PipeWire:Interface:Node":
			dir := map[string]string{"Audio/Sink": "out", "Audio/Source": "in"}[str(p, "media.class")]
			if dir == "" || strings.HasSuffix(str(p, "node.name"), ".monitor") {
				continue
			}
			n := node{id: o.ID, name: str(p, "node.name"), dir: dir, dev: -1, at: -1}
			n.desc = str(p, "node.description")
			if n.desc == "" {
				n.desc = str(p, "node.nick")
			}
			if d, ok := num(p, "device.id"); ok {
				n.dev = d
			}
			if a, ok := num(p, "card.profile.device"); ok {
				n.at = a
			}
			nodes = append(nodes, n)
		}
	}

	var out []Port
	covered := map[string]bool{}
	ids := make([]int, 0, len(devices))
	for id := range devices {
		ids = append(ids, id)
	}
	sort.Ints(ids)
	for _, id := range ids {
		d := devices[id]
		// Cards without routes (most Bluetooth and USB ones show their
		// nodes as they are) are handled with the plain nodes below.
		if len(d.Info.Params.EnumRoute) == 0 {
			continue
		}
		active := -1
		if len(d.Info.Params.Profile) > 0 {
			active = d.Info.Params.Profile[0].Index
		}
		devName := str(d.Info.Props, "device.name")
		for _, r := range d.Info.Params.EnumRoute {
			dir := map[string]string{"Output": "out", "Input": "in"}[r.Direction]
			if dir == "" {
				continue
			}
			p := Port{Key: devName + "#" + r.Name, Label: r.Description, Direction: dir, Available: r.Available}
			if p.Available == "" {
				p.Available = "unknown"
			}
			// Its node in the setup running now.
			for _, n := range nodes {
				if n.dev == id && n.dir == dir && slices.Contains(r.Devices, n.at) && slices.Contains(r.Profiles, active) {
					p.Node = n.name
					covered[n.name] = true
				}
			}
			if p.Node == "" {
				best, ok := bestProfile(d.Info.Params.EnumProfile, r.Profiles)
				if !ok {
					continue // in no setup that can run
				}
				p.Device, p.Profile = id, best
			}
			out = append(out, p)
		}
	}
	// Everything else (Bluetooth, USB, virtual sinks): as PipeWire shows it.
	for _, n := range nodes {
		if covered[n.name] {
			continue
		}
		out = append(out, Port{Key: n.name, Label: n.desc, Direction: n.dir, Available: "unknown", Node: n.name})
	}
	nameScreens(out, screens)
	sort.SliceStable(out, func(i, j int) bool { return out[i].Direction == "out" && out[j].Direction == "in" })
	return out, nil
}

// The highest-priority setup that can run and includes the route.
func bestProfile(all []profile, of []int) (int, bool) {
	best, prio, found := 0, 0, false
	for _, p := range all {
		if !slices.Contains(of, p.Index) || p.Available == "no" || p.Name == "off" {
			continue
		}
		if !found || p.Priority > prio {
			best, prio, found = p.Index, p.Priority, true
		}
	}
	return best, found
}

// An HDMI/DisplayPort port in use is named after its screen, when it's
// clear which screen that is (one screen with sound, one port in use).
func nameScreens(ports []Port, screens []string) {
	if len(screens) != 1 {
		return
	}
	var hit []int
	for i, p := range ports {
		if p.Direction == "out" && p.Available == "yes" && strings.Contains(p.Label, "HDMI") {
			hit = append(hit, i)
		}
	}
	if len(hit) == 1 {
		ports[hit[0]].Label = screens[0]
	}
}

// Screens reads the names of the screens connected with sound, from the
// ELD files ALSA keeps for each HDMI/DisplayPort pin (/proc/asound).
func Screens(root string) []string {
	files, _ := filepath.Glob(filepath.Join(root, "card*", "eld#*"))
	var out []string
	for _, f := range files {
		b, err := os.ReadFile(f)
		if err != nil {
			continue
		}
		present, name := false, ""
		for l := range strings.SplitSeq(string(b), "\n") {
			k, v, _ := strings.Cut(l, "\t")
			v = strings.TrimSpace(v)
			switch strings.TrimSpace(k) {
			case "monitor_present":
				present = v == "1"
			case "monitor_name":
				name = v
			}
		}
		if present && name != "" {
			out = append(out, name)
		}
	}
	return out
}

// Runner runs a command and returns its output (exec, or a fake in tests).
type Runner func(name string, args ...string) ([]byte, error)

// Exec runs real commands.
func Exec(name string, args ...string) ([]byte, error) {
	var stderr bytes.Buffer
	c := exec.Command(name, args...)
	c.Stderr = &stderr
	out, err := c.Output()
	if err != nil && stderr.Len() > 0 {
		err = fmt.Errorf("%s: %s", name, strings.TrimSpace(stderr.String()))
	}
	return out, err
}

// List reads what's there now.
func List(run Runner) ([]Port, error) {
	dump, err := run("pw-dump")
	if err != nil {
		return nil, err
	}
	return Parse(dump, Screens("/proc/asound"))
}

// ErrUnplugged: PipeWire won't send sound to (or take it from) a port with
// nothing plugged in.
var ErrUnplugged = errors.New("nothing is plugged in there")

// Use makes the port with `key` the default for its direction, switching
// its card's setup first if needed (waiting up to `wait` for its node).
func Use(run Runner, key string, wait time.Duration) error {
	ports, err := List(run)
	if err != nil {
		return err
	}
	var p *Port
	for i := range ports {
		if ports[i].Key == key {
			p = &ports[i]
		}
	}
	if p == nil {
		return fmt.Errorf("no such sound device: %s", key)
	}
	if p.Available == "no" {
		return ErrUnplugged
	}
	if p.Node == "" {
		if _, err := run("wpctl", "set-profile", fmt.Sprint(p.Device), fmt.Sprint(p.Profile)); err != nil {
			return err
		}
		for deadline := time.Now().Add(wait); p.Node == ""; {
			if time.Now().After(deadline) {
				return fmt.Errorf("%s didn't start", p.Label)
			}
			time.Sleep(100 * time.Millisecond)
			if ports, err = List(run); err != nil {
				return err
			}
			for _, q := range ports {
				if q.Key == key && q.Node != "" {
					p.Node = q.Node
				}
			}
		}
	}
	id, err := nodeID(run, p.Node)
	if err != nil {
		return err
	}
	_, err = run("wpctl", "set-default", fmt.Sprint(id))
	return err
}

func nodeID(run Runner, name string) (int, error) {
	dump, err := run("pw-dump")
	if err != nil {
		return 0, err
	}
	var objs []object
	if err := json.Unmarshal(dump, &objs); err != nil {
		return 0, err
	}
	for _, o := range objs {
		if o.Type == "PipeWire:Interface:Node" && str(o.Info.Props, "node.name") == name {
			return o.ID, nil
		}
	}
	return 0, fmt.Errorf("%s is gone", name)
}
