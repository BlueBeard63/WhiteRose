// Package ical reads iCalendar (RFC 5545) data, the format every calendar
// service exports, and lists the events that fall in a time range, with
// repeating events expanded. It covers what real feeds (Google, Outlook,
// iCloud, Nextcloud) use: time zones, all-day events, DURATION, RRULE
// (daily/weekly/monthly/yearly with INTERVAL, COUNT, UNTIL, BYDAY,
// BYMONTHDAY), EXDATE, moved or edited occurrences (RECURRENCE-ID) and
// cancelled events.
package ical

import (
	"bufio"
	"io"
	"sort"
	"strconv"
	"strings"
	"time"
)

// Event is one occurrence, ready to show.
type Event struct {
	UID      string
	Title    string
	Location string
	Start    time.Time
	End      time.Time
	AllDay   bool
}

// prop is one content line: NAME;PARAMS:VALUE.
type prop struct {
	name   string
	params map[string]string
	value  string
}

// unfold joins continuation lines (starting with a space or tab).
func unfold(r io.Reader) ([]string, error) {
	sc := bufio.NewScanner(r)
	sc.Buffer(make([]byte, 64*1024), 4*1024*1024)
	var lines []string
	for sc.Scan() {
		l := strings.TrimRight(sc.Text(), "\r")
		if len(l) > 0 && (l[0] == ' ' || l[0] == '\t') && len(lines) > 0 {
			lines[len(lines)-1] += l[1:]
			continue
		}
		if l != "" {
			lines = append(lines, l)
		}
	}
	return lines, sc.Err()
}

func parseLine(l string) prop {
	// The value starts at the first ':' outside a quoted parameter.
	inQuote, colon := false, -1
	for i, c := range l {
		if c == '"' {
			inQuote = !inQuote
		} else if c == ':' && !inQuote {
			colon = i
			break
		}
	}
	if colon < 0 {
		return prop{name: strings.ToUpper(l)}
	}
	head, value := l[:colon], l[colon+1:]
	parts := strings.Split(head, ";")
	p := prop{name: strings.ToUpper(parts[0]), params: map[string]string{}, value: value}
	for _, kv := range parts[1:] {
		if k, v, ok := strings.Cut(kv, "="); ok {
			p.params[strings.ToUpper(k)] = strings.Trim(v, `"`)
		}
	}
	return p
}

func unescape(s string) string {
	r := strings.NewReplacer(`\n`, "\n", `\N`, "\n", `\,`, ",", `\;`, ";", `\\`, `\`)
	return r.Replace(s)
}

// Item is a VEVENT as written, before repeats are expanded.
type Item struct {
	uid, title, location, status string
	start                        time.Time
	end                          *time.Time
	duration                     *time.Duration
	allDay                       bool
	rrule                        string
	exdates                      []time.Time
	recurrenceID                 *time.Time
}

// Parse reads every VEVENT in r. loc is the zone for "floating" times (and
// the fallback for unknown TZIDs): normally time.Local.
func Parse(r io.Reader, loc *time.Location) ([]Item, error) {
	lines, err := unfold(r)
	if err != nil {
		return nil, err
	}
	var out []Item
	var cur *Item
	depth := 0 // nesting inside the VEVENT (VALARM blocks)
	for _, l := range lines {
		p := parseLine(l)
		switch {
		case p.name == "BEGIN" && strings.EqualFold(p.value, "VEVENT"):
			cur, depth = &Item{}, 0
			continue
		case p.name == "END" && strings.EqualFold(p.value, "VEVENT") && cur != nil:
			if !cur.start.IsZero() {
				out = append(out, *cur)
			}
			cur = nil
			continue
		case cur == nil:
			continue
		case p.name == "BEGIN":
			depth++
			continue
		case p.name == "END":
			depth--
			continue
		case depth > 0:
			continue
		}
		switch p.name {
		case "UID":
			cur.uid = p.value
		case "SUMMARY":
			cur.title = unescape(p.value)
		case "LOCATION":
			cur.location = unescape(p.value)
		case "STATUS":
			cur.status = strings.ToUpper(p.value)
		case "DTSTART":
			if t, allDay, ok := parseTime(p, loc); ok {
				cur.start, cur.allDay = t, allDay
			}
		case "DTEND":
			if t, _, ok := parseTime(p, loc); ok {
				cur.end = &t
			}
		case "DURATION":
			if d, ok := parseDuration(p.value); ok {
				cur.duration = &d
			}
		case "RRULE":
			cur.rrule = p.value
		case "EXDATE":
			for _, v := range strings.Split(p.value, ",") {
				q := p
				q.value = v
				if t, _, ok := parseTime(q, loc); ok {
					cur.exdates = append(cur.exdates, t)
				}
			}
		case "RECURRENCE-ID":
			if t, _, ok := parseTime(p, loc); ok {
				cur.recurrenceID = &t
			}
		}
	}
	return out, nil
}

// windowsZones maps the time zone names Outlook/Exchange use to IANA names
// (the common ones; others fall back to the local zone).
var windowsZones = map[string]string{
	"GMT Standard Time":              "Europe/London",
	"Greenwich Standard Time":        "Atlantic/Reykjavik",
	"W. Europe Standard Time":        "Europe/Berlin",
	"Romance Standard Time":          "Europe/Paris",
	"Central Europe Standard Time":   "Europe/Budapest",
	"Central European Standard Time": "Europe/Warsaw",
	"E. Europe Standard Time":        "Europe/Chisinau",
	"FLE Standard Time":              "Europe/Kiev",
	"GTB Standard Time":              "Europe/Bucharest",
	"Russian Standard Time":          "Europe/Moscow",
	"Eastern Standard Time":          "America/New_York",
	"Central Standard Time":          "America/Chicago",
	"Mountain Standard Time":         "America/Denver",
	"Pacific Standard Time":          "America/Los_Angeles",
	"Atlantic Standard Time":         "America/Halifax",
	"India Standard Time":            "Asia/Kolkata",
	"China Standard Time":            "Asia/Shanghai",
	"Tokyo Standard Time":            "Asia/Tokyo",
	"Singapore Standard Time":        "Asia/Singapore",
	"AUS Eastern Standard Time":      "Australia/Sydney",
	"New Zealand Standard Time":      "Pacific/Auckland",
	"UTC":                            "UTC",
}

func zone(tzid string, fallback *time.Location) *time.Location {
	if tzid == "" {
		return fallback
	}
	if name, ok := windowsZones[tzid]; ok {
		tzid = name
	}
	// Some exporters prefix the IANA name with a path ("/mozilla.org/.../Europe/London").
	for {
		if loc, err := time.LoadLocation(tzid); err == nil {
			return loc
		}
		i := strings.Index(tzid, "/")
		if i < 0 {
			return fallback
		}
		tzid = tzid[i+1:]
	}
}

// parseTime reads DATE ("20261109", all-day) or DATE-TIME ("20261109T150000",
// with Z for UTC, a TZID parameter, or floating).
func parseTime(p prop, loc *time.Location) (time.Time, bool, bool) {
	v := strings.TrimSpace(p.value)
	if len(v) == 8 || strings.EqualFold(p.params["VALUE"], "DATE") {
		t, err := time.ParseInLocation("20060102", v[:min(8, len(v))], loc)
		return t, true, err == nil
	}
	if strings.HasSuffix(v, "Z") {
		t, err := time.Parse("20060102T150405Z", v)
		return t, false, err == nil
	}
	t, err := time.ParseInLocation("20060102T150405", v, zone(p.params["TZID"], loc))
	return t, false, err == nil
}

// parseDuration reads RFC 5545 durations: P1W, P1D, PT1H30M, -PT15M, P1DT2H.
func parseDuration(s string) (time.Duration, bool) {
	neg := strings.HasPrefix(s, "-")
	s = strings.TrimLeft(s, "+-")
	if !strings.HasPrefix(s, "P") {
		return 0, false
	}
	var d time.Duration
	num := ""
	inTime := false
	for _, c := range s[1:] {
		switch {
		case c == 'T':
			inTime = true
		case c >= '0' && c <= '9':
			num += string(c)
		default:
			n, err := strconv.Atoi(num)
			if err != nil {
				return 0, false
			}
			num = ""
			unit := map[rune]time.Duration{'W': 7 * 24 * time.Hour, 'D': 24 * time.Hour}
			if inTime {
				unit = map[rune]time.Duration{'H': time.Hour, 'M': time.Minute, 'S': time.Second}
			}
			u, ok := unit[c]
			if !ok {
				return 0, false
			}
			d += time.Duration(n) * u
		}
	}
	if neg {
		d = -d
	}
	return d, num == ""
}

// Between lists the occurrences that overlap [from, to), sorted by start.
func Between(events []Item, from, to time.Time) []Event {
	// Edited or moved occurrences replace the original instance.
	overrides := map[string]map[int64]bool{}
	for _, e := range events {
		if e.recurrenceID != nil {
			if overrides[e.uid] == nil {
				overrides[e.uid] = map[int64]bool{}
			}
			overrides[e.uid][e.recurrenceID.Unix()] = true
		}
	}
	var out []Event
	for _, e := range events {
		if e.status == "CANCELLED" {
			continue
		}
		length := eventLength(e)
		add := func(start time.Time) {
			end := start.Add(length)
			if e.allDay {
				// Whole days stay whole across DST changes.
				days := int(length.Round(24*time.Hour) / (24 * time.Hour))
				end = start.AddDate(0, 0, max(days, 1))
			}
			if start.Before(to) && end.After(from) {
				out = append(out, Event{UID: e.uid, Title: e.title, Location: e.location, Start: start, End: end, AllDay: e.allDay})
			}
		}
		if e.rrule == "" || e.recurrenceID != nil {
			add(e.start)
			continue
		}
		skip := map[int64]bool{}
		for _, x := range e.exdates {
			skip[x.Unix()] = true
		}
		for k := range overrides[e.uid] {
			skip[k] = true
		}
		expand(e.start, parseRule(e.rrule), to, func(t time.Time) {
			if !skip[t.Unix()] {
				add(t)
			}
		})
	}
	sort.SliceStable(out, func(i, j int) bool {
		if !out[i].Start.Equal(out[j].Start) {
			return out[i].Start.Before(out[j].Start)
		}
		return out[i].AllDay && !out[j].AllDay
	})
	return out
}

func eventLength(e Item) time.Duration {
	switch {
	case e.end != nil:
		return e.end.Sub(e.start)
	case e.duration != nil:
		return *e.duration
	case e.allDay:
		return 24 * time.Hour
	}
	return 0
}

type rule struct {
	freq       string
	interval   int
	count      int
	until      time.Time
	byDay      []weekdayN // e.g. MO, 2TU, -1FR
	byMonthDay []int
}

type weekdayN struct {
	n   int // 0 = every such weekday
	day time.Weekday
}

var dayNames = map[string]time.Weekday{"SU": time.Sunday, "MO": time.Monday, "TU": time.Tuesday, "WE": time.Wednesday, "TH": time.Thursday, "FR": time.Friday, "SA": time.Saturday}

func parseRule(s string) rule {
	r := rule{interval: 1}
	for _, part := range strings.Split(s, ";") {
		k, v, _ := strings.Cut(part, "=")
		switch strings.ToUpper(k) {
		case "FREQ":
			r.freq = strings.ToUpper(v)
		case "INTERVAL":
			if n, err := strconv.Atoi(v); err == nil && n > 0 {
				r.interval = n
			}
		case "COUNT":
			r.count, _ = strconv.Atoi(v)
		case "UNTIL":
			if t, _, ok := parseTime(prop{value: v, params: map[string]string{}}, time.UTC); ok {
				if len(v) == 8 { // a date: through the end of that day
					t = t.Add(24*time.Hour - time.Second)
				}
				r.until = t
			}
		case "BYDAY":
			for _, d := range strings.Split(v, ",") {
				d = strings.ToUpper(strings.TrimSpace(d))
				if len(d) < 2 {
					continue
				}
				wd, ok := dayNames[d[len(d)-2:]]
				if !ok {
					continue
				}
				n, _ := strconv.Atoi(d[:len(d)-2])
				r.byDay = append(r.byDay, weekdayN{n: n, day: wd})
			}
		case "BYMONTHDAY":
			for _, d := range strings.Split(v, ",") {
				if n, err := strconv.Atoi(d); err == nil {
					r.byMonthDay = append(r.byMonthDay, n)
				}
			}
		}
	}
	return r
}

// maxOccurrences bounds the expansion of a rule (COUNT or a long UNTIL).
const maxOccurrences = 5000

// expand calls fn for each occurrence of r from start until `to`, in order.
// The first occurrence is start itself (RFC 5545: DTSTART is always the first).
func expand(start time.Time, r rule, to time.Time, fn func(time.Time)) {
	n := 0
	emit := func(t time.Time) bool {
		if t.Before(start) {
			return true
		}
		if !r.until.IsZero() && t.After(r.until) || !t.Before(to) || r.count > 0 && n >= r.count || n >= maxOccurrences {
			return false
		}
		n++
		fn(t)
		return true
	}
	at := func(base time.Time, y int, m time.Month, d int) time.Time {
		return time.Date(y, m, d, base.Hour(), base.Minute(), base.Second(), 0, base.Location())
	}
	for period := 0; period < maxOccurrences; period++ {
		var cands []time.Time
		switch r.freq {
		case "DAILY":
			cands = []time.Time{start.AddDate(0, 0, period*r.interval)}
		case "WEEKLY":
			if len(r.byDay) == 0 {
				cands = []time.Time{start.AddDate(0, 0, 7*period*r.interval)}
				break
			}
			// The week (Monday first) containing start, `period` intervals on.
			off := (int(start.Weekday()) + 6) % 7
			monday := start.AddDate(0, 0, -off+7*period*r.interval)
			for _, wd := range r.byDay {
				cands = append(cands, monday.AddDate(0, 0, (int(wd.day)+6)%7))
			}
		case "MONTHLY":
			first := time.Date(start.Year(), start.Month(), 1, 0, 0, 0, 0, start.Location()).AddDate(0, period*r.interval, 0)
			y, m := first.Year(), first.Month()
			days := daysIn(y, m)
			switch {
			case len(r.byMonthDay) > 0:
				for _, d := range r.byMonthDay {
					if d < 0 {
						d = days + d + 1
					}
					if d >= 1 && d <= days {
						cands = append(cands, at(start, y, m, d))
					}
				}
			case len(r.byDay) > 0:
				for _, wd := range r.byDay {
					for _, d := range nthWeekdays(y, m, wd, start.Location()) {
						cands = append(cands, at(start, y, m, d))
					}
				}
			default:
				if start.Day() <= days { // the 31st skips shorter months
					cands = []time.Time{at(start, y, m, start.Day())}
				}
			}
		case "YEARLY":
			y := start.Year() + period*r.interval
			if start.Day() <= daysIn(y, start.Month()) {
				cands = []time.Time{at(start, y, start.Month(), start.Day())}
			}
		default:
			emit(start)
			return
		}
		sort.Slice(cands, func(i, j int) bool { return cands[i].Before(cands[j]) })
		for _, c := range cands {
			if !emit(c) {
				return
			}
		}
	}
}

func daysIn(y int, m time.Month) int {
	return time.Date(y, m+1, 0, 0, 0, 0, 0, time.UTC).Day()
}

// nthWeekdays: the days of month m that match wd (every one, or the nth,
// counting from the end when negative).
func nthWeekdays(y int, m time.Month, wd weekdayN, loc *time.Location) []int {
	var all []int
	for d := 1; d <= daysIn(y, m); d++ {
		if time.Date(y, m, d, 12, 0, 0, 0, loc).Weekday() == wd.day {
			all = append(all, d)
		}
	}
	switch {
	case wd.n == 0:
		return all
	case wd.n > 0 && wd.n <= len(all):
		return []int{all[wd.n-1]}
	case wd.n < 0 && -wd.n <= len(all):
		return []int{all[len(all)+wd.n]}
	}
	return nil
}
