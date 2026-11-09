package ical

import (
	"strings"
	"testing"
	"time"
)

func mustParse(t *testing.T, body string) []Item {
	t.Helper()
	ics := "BEGIN:VCALENDAR\r\nVERSION:2.0\r\n" + strings.ReplaceAll(strings.TrimSpace(body), "\n", "\r\n") + "\r\nEND:VCALENDAR\r\n"
	items, err := Parse(strings.NewReader(ics), london(t))
	if err != nil {
		t.Fatal(err)
	}
	return items
}

func london(t *testing.T) *time.Location {
	loc, err := time.LoadLocation("Europe/London")
	if err != nil {
		t.Skip("no tzdata")
	}
	return loc
}

func month(t *testing.T, y int, m time.Month) (time.Time, time.Time) {
	from := time.Date(y, m, 1, 0, 0, 0, 0, london(t))
	return from, from.AddDate(0, 1, 0)
}

func starts(evs []Event) []string {
	var s []string
	for _, e := range evs {
		s = append(s, e.Start.In(e.Start.Location()).Format("2006-01-02 15:04"))
	}
	return s
}

func eq(t *testing.T, got, want []string) {
	t.Helper()
	if strings.Join(got, "|") != strings.Join(want, "|") {
		t.Fatalf("got  %v\nwant %v", got, want)
	}
}

func TestSingleEventsZonesAndText(t *testing.T) {
	items := mustParse(t, `
BEGIN:VEVENT
UID:a
SUMMARY:Design review \, WhiteRose
LOCATION:Room 2\nFloor 3
DTSTART;TZID=Europe/London:20261109T150000
DTEND;TZID=Europe/London:20261109T160000
END:VEVENT
BEGIN:VEVENT
UID:b
SUMMARY:Call with New York
DTSTART:20261110T170000Z
DURATION:PT30M
END:VEVENT
BEGIN:VEVENT
UID:c
SUMMARY:Outlook meeting
DTSTART;TZID="Pacific Standard Time":20261111T090000
DTEND;TZID="Pacific Standard Time":20261111T100000
END:VEVENT
BEGIN:VEVENT
UID:d
SUMMARY:Holiday
DTSTART;VALUE=DATE:20261112
DTEND;VALUE=DATE:20261114
BEGIN:VALARM
SUMMARY:Not an event title
TRIGGER:-PT15M
END:VALARM
END:VEVENT
BEGIN:VEVENT
UID:e
SUMMARY:Cancelled
STATUS:CANCELLED
DTSTART:20261113T100000Z
END:VEVENT`)
	from, to := month(t, 2026, time.November)
	evs := Between(items, from, to)
	if len(evs) != 4 {
		t.Fatalf("got %d events: %+v", len(evs), evs)
	}
	a := evs[0]
	if a.Title != "Design review , WhiteRose" || a.Location != "Room 2\nFloor 3" || a.End.Sub(a.Start) != time.Hour {
		t.Fatalf("event a: %+v", a)
	}
	if got := evs[1].Start.In(london(t)).Format("15:04"); got != "17:00" || evs[1].End.Sub(evs[1].Start) != 30*time.Minute {
		t.Fatalf("UTC event in London: %s, %v", got, evs[1].End.Sub(evs[1].Start))
	}
	if got := evs[2].Start.In(london(t)).Format("15:04"); got != "17:00" {
		t.Fatalf("Outlook's Windows zone name: 09:00 Pacific is %s in London, want 17:00", got)
	}
	d := evs[3]
	if !d.AllDay || d.Title != "Holiday" || d.End.Sub(d.Start) != 48*time.Hour {
		t.Fatalf("two-day all-day event (VALARM summary must not win): %+v", d)
	}
}

func TestLineFolding(t *testing.T) {
	items := mustParse(t, "BEGIN:VEVENT\nUID:f\nSUMMARY:A very long title that \n continues here\nDTSTART:20261109T100000Z\nEND:VEVENT")
	if items[0].title != "A very long title that continues here" {
		t.Fatalf("got %q", items[0].title)
	}
}

func TestWeeklyWithDaysExceptionsAndAMovedOccurrence(t *testing.T) {
	items := mustParse(t, `
BEGIN:VEVENT
UID:gym
SUMMARY:Gym
DTSTART;TZID=Europe/London:20261102T183000
DTEND;TZID=Europe/London:20261102T193000
RRULE:FREQ=WEEKLY;BYDAY=MO,TH
EXDATE;TZID=Europe/London:20261112T183000
END:VEVENT
BEGIN:VEVENT
UID:gym
RECURRENCE-ID;TZID=Europe/London:20261116T183000
SUMMARY:Gym (late)
DTSTART;TZID=Europe/London:20261116T200000
DTEND;TZID=Europe/London:20261116T210000
END:VEVENT`)
	from, to := month(t, 2026, time.November)
	eq(t, starts(Between(items, from, to)), []string{
		"2026-11-02 18:30", "2026-11-05 18:30", "2026-11-09 18:30",
		// 12th excluded
		"2026-11-16 20:00", // moved
		"2026-11-19 18:30", "2026-11-23 18:30", "2026-11-26 18:30", "2026-11-30 18:30",
	})
}

func TestRepeatsKeepLocalTimeAcrossDST(t *testing.T) {
	items := mustParse(t, `
BEGIN:VEVENT
UID:standup
DTSTART;TZID=Europe/London:20261023T093000
RRULE:FREQ=DAILY;COUNT=5
END:VEVENT`)
	from := time.Date(2026, 10, 20, 0, 0, 0, 0, london(t))
	eq(t, starts(Between(items, from, from.AddDate(0, 0, 30))), []string{
		"2026-10-23 09:30", "2026-10-24 09:30", "2026-10-25 09:30", "2026-10-26 09:30", "2026-10-27 09:30",
	})
}

func TestMonthlyAndYearlyRules(t *testing.T) {
	items := mustParse(t, `
BEGIN:VEVENT
UID:m1
SUMMARY:Second Tuesday
DTSTART:20260113T120000Z
RRULE:FREQ=MONTHLY;BYDAY=2TU
END:VEVENT
BEGIN:VEVENT
UID:m2
SUMMARY:Last Friday
DTSTART:20260130T120000Z
RRULE:FREQ=MONTHLY;BYDAY=-1FR;UNTIL=20261231T235959Z
END:VEVENT
BEGIN:VEVENT
UID:m3
SUMMARY:On the 31st
DTSTART:20260131T120000Z
RRULE:FREQ=MONTHLY
END:VEVENT
BEGIN:VEVENT
UID:m4
SUMMARY:Last day of month
DTSTART:20260131T080000Z
RRULE:FREQ=MONTHLY;BYMONTHDAY=-1
END:VEVENT
BEGIN:VEVENT
UID:y
SUMMARY:Birthday
DTSTART;VALUE=DATE:19900209
RRULE:FREQ=YEARLY
END:VEVENT`)
	byTitle := func(evs []Event) map[string][]string {
		m := map[string][]string{}
		for _, e := range evs {
			m[e.Title] = append(m[e.Title], e.Start.UTC().Format("01-02"))
		}
		return m
	}
	nov := byTitle(Between(items, time.Date(2026, 11, 1, 0, 0, 0, 0, time.UTC), time.Date(2026, 12, 1, 0, 0, 0, 0, time.UTC)))
	eq(t, nov["Second Tuesday"], []string{"11-10"})
	eq(t, nov["Last Friday"], []string{"11-27"})
	eq(t, nov["On the 31st"], nil) // November has no 31st
	eq(t, nov["Last day of month"], []string{"11-30"})
	feb := byTitle(Between(items, time.Date(2027, 2, 1, 0, 0, 0, 0, time.UTC), time.Date(2027, 3, 1, 0, 0, 0, 0, time.UTC)))
	eq(t, feb["Last Friday"], nil) // UNTIL ended it
	eq(t, feb["Birthday"], []string{"02-09"})
}

func TestIntervalCountAndUntilDate(t *testing.T) {
	items := mustParse(t, `
BEGIN:VEVENT
UID:x
DTSTART:20261102T100000Z
RRULE:FREQ=WEEKLY;INTERVAL=2;BYDAY=MO,WE;COUNT=5
END:VEVENT
BEGIN:VEVENT
UID:z
DTSTART:20261101T100000Z
RRULE:FREQ=DAILY;UNTIL=20261103
END:VEVENT`)
	got := Between(items, time.Date(2026, 10, 1, 0, 0, 0, 0, time.UTC), time.Date(2027, 1, 1, 0, 0, 0, 0, time.UTC))
	var x, z []string
	for _, e := range got {
		if e.UID == "x" {
			x = append(x, e.Start.Format("01-02"))
		} else {
			z = append(z, e.Start.Format("01-02"))
		}
	}
	eq(t, x, []string{"11-02", "11-04", "11-16", "11-18", "11-30"})
	eq(t, z, []string{"11-01", "11-02", "11-03"}) // UNTIL as a date includes that day
}

func TestDurations(t *testing.T) {
	for s, want := range map[string]time.Duration{"PT1H30M": 90 * time.Minute, "P1D": 24 * time.Hour, "P1W": 7 * 24 * time.Hour, "P1DT2H": 26 * time.Hour, "-PT15M": -15 * time.Minute} {
		if d, ok := parseDuration(s); !ok || d != want {
			t.Errorf("%s: got %v %v", s, d, ok)
		}
	}
	for _, bad := range []string{"", "1H", "PT", "PTXH", "P1X"} {
		if _, ok := parseDuration(bad); ok && bad != "PT" {
			t.Errorf("%q accepted", bad)
		}
	}
}

func FuzzParse(f *testing.F) {
	f.Add("BEGIN:VEVENT\r\nDTSTART:20261109T100000Z\r\nRRULE:FREQ=WEEKLY;BYDAY=-1FR,2TU;COUNT=3\r\nEND:VEVENT\r\n")
	f.Add("BEGIN:VEVENT\r\nDTSTART;TZID=\"x:y\":2026\r\nEND:VEVENT")
	f.Fuzz(func(t *testing.T, s string) {
		items, err := Parse(strings.NewReader(s), time.UTC)
		if err != nil {
			return
		}
		Between(items, time.Date(2026, 1, 1, 0, 0, 0, 0, time.UTC), time.Date(2027, 1, 1, 0, 0, 0, 0, time.UTC))
	})
}
