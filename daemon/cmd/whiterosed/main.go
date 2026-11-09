// Command whiterosed is the WhiteRose system daemon.
//
//	whiterosed serve          the daemon: settings database, system state,
//	                          calendars, weather (the compositor starts it)
//	whiterosed settings list | get KEY | set KEY JSON | reset KEY
//	whiterosed calendar list | add NAME URL|FILE [#COLOUR] | remove N | events FROM TO
//	whiterosed monitors       list connected monitors and their stable IDs
//	whiterosed check-config   validate a displays.toml file
//	whiterosed brightness set <percent>
//	whiterosed airplane on|off
package main

import (
	"encoding/json"
	"flag"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"text/tabwriter"

	"whiterose/daemon/internal/config"
	"whiterose/daemon/internal/monitor"
)

func main() {
	os.Exit(run(os.Args[1:], os.Stdout, os.Stderr))
}

func run(args []string, stdout, stderr io.Writer) int {
	if len(args) == 0 {
		usage(stderr)
		return 2
	}
	switch args[0] {
	case "monitors":
		return cmdMonitors(args[1:], stdout, stderr)
	case "check-config":
		return cmdCheckConfig(args[1:], stdout, stderr)
	case "serve":
		return cmdServe(args[1:], stdout, stderr)
	case "settings":
		return cmdSettings(args[1:], stdout, stderr)
	case "brightness":
		return cmdBrightness(args[1:], stdout, stderr)
	case "airplane":
		return cmdAirplane(args[1:], stdout, stderr)
	case "calendar":
		return cmdCalendar(args[1:], stdout, stderr)
	case "-h", "--help", "help":
		usage(stdout)
		return 0
	default:
		fmt.Fprintf(stderr, "whiterosed: unknown command %q\n", args[0])
		usage(stderr)
		return 2
	}
}

func usage(w io.Writer) {
	fmt.Fprint(w, `usage: whiterosed <command> [flags]

commands:
  monitors       list connected monitors and their stable IDs
  check-config   validate a displays.toml file
  serve          run the daemon (settings database, system state, calendars, weather)
  settings       list | get KEY | set KEY JSON | reset KEY
  brightness     set <percent>: change the screen backlight (through logind)
  airplane       on|off: block or unblock every radio
  calendar       list | add NAME URL|FILE [#COLOUR] | remove N | events FROM TO
`)
}

type monitorJSON struct {
	Connector    string `json:"connector"`
	ID           string `json:"id"`
	Strong       bool   `json:"strong"`
	Manufacturer string `json:"manufacturer,omitempty"`
	Product      uint16 `json:"product,omitempty"`
	Name         string `json:"name,omitempty"`
	HasSerial    bool   `json:"has_serial"`
	ChecksumOK   bool   `json:"checksum_ok"`
}

func cmdMonitors(args []string, stdout, stderr io.Writer) int {
	fs := flag.NewFlagSet("monitors", flag.ContinueOnError)
	fs.SetOutput(stderr)
	root := fs.String("sysfs", monitor.SysfsDRM, "DRM sysfs directory")
	asJSON := fs.Bool("json", false, "print JSON")
	if err := fs.Parse(args); err != nil {
		return 2
	}
	outs, err := monitor.Scan(os.DirFS(*root))
	if err != nil {
		fmt.Fprintln(stderr, "whiterosed:", err)
		return 1
	}
	ids := monitor.Identify(outs)

	rows := make([]monitorJSON, len(ids))
	for i, m := range ids {
		r := monitorJSON{Connector: m.Connector, ID: m.ID, Strong: m.Strong}
		if m.EDID != nil {
			r.Manufacturer, r.Product, r.Name = m.EDID.Manufacturer, m.EDID.ProductCode, m.EDID.Name
			r.HasSerial, r.ChecksumOK = monitor.HasSerial(m.EDID), m.EDID.ChecksumOK
		}
		rows[i] = r
	}
	if *asJSON {
		enc := json.NewEncoder(stdout)
		enc.SetIndent("", "  ")
		if err := enc.Encode(rows); err != nil {
			fmt.Fprintln(stderr, "whiterosed:", err)
			return 1
		}
		return 0
	}
	if len(rows) == 0 {
		fmt.Fprintln(stdout, "no connected monitors found")
		return 0
	}
	tw := tabwriter.NewWriter(stdout, 0, 4, 2, ' ', 0)
	fmt.Fprintln(tw, "CONNECTOR\tID\tSTRENGTH\tMONITOR")
	for _, r := range rows {
		strength := "weak"
		if r.Strong {
			strength = "strong"
		}
		name := r.Name
		if name == "" && r.Manufacturer != "" {
			name = fmt.Sprintf("%s %04X", r.Manufacturer, r.Product)
		}
		fmt.Fprintf(tw, "%s\t%s\t%s\t%s\n", r.Connector, r.ID, strength, name)
	}
	if err := tw.Flush(); err != nil {
		return 1
	}
	return 0
}

func cmdCheckConfig(args []string, stdout, stderr io.Writer) int {
	fs := flag.NewFlagSet("check-config", flag.ContinueOnError)
	fs.SetOutput(stderr)
	if err := fs.Parse(args); err != nil {
		return 2
	}
	path := fs.Arg(0)
	if path == "" {
		dir, err := os.UserConfigDir()
		if err != nil {
			fmt.Fprintln(stderr, "whiterosed:", err)
			return 1
		}
		path = filepath.Join(dir, "whiterose", "displays.toml")
	}
	d, err := config.LoadDisplays(path)
	if err != nil {
		fmt.Fprintln(stderr, err)
		return 1
	}
	fmt.Fprintf(stdout, "%s: ok (%d monitors, %d profiles)\n", path, len(d.Monitors), len(d.Profiles))
	return 0
}
