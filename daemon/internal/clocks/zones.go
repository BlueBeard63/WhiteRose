package clocks

import (
	"bufio"
	"os"
	"strings"
)

// SearchZones is an offline fallback and supports an exact IANA name typed
// into the city search. Normal city searches use weather.SearchCities first.
func SearchZones(query string) []City {
	out := []City{}
	if strings.Contains(query, "/") || query == "UTC" {
		if _, err := location(query); err == nil {
			return []City{{Name: strings.ReplaceAll(query[strings.LastIndex(query, "/")+1:], "_", " "), Zone: query}}
		}
	}
	f, err := os.Open("/usr/share/zoneinfo/zone.tab")
	if err != nil {
		return out
	}
	defer f.Close()
	query = strings.ToLower(strings.ReplaceAll(strings.TrimSpace(query), " ", "_"))
	if len(query) < 2 {
		return out
	}
	sc := bufio.NewScanner(f)
	for sc.Scan() && len(out) < 20 {
		line := strings.Fields(sc.Text())
		if len(line) < 3 || strings.HasPrefix(line[0], "#") || !strings.Contains(strings.ToLower(line[2]), query) {
			continue
		}
		zone := line[2]
		out = append(out, City{Name: strings.ReplaceAll(zone[strings.LastIndex(zone, "/")+1:], "_", " "), Country: line[0], Zone: zone})
	}
	return out
}
