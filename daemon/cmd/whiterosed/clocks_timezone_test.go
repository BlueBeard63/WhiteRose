package main

import (
	"os"
	"path/filepath"
	"testing"
	"time"
)

func TestClockLocationReadsCopiedAndLinkedTZFiles(t *testing.T) {
	source := "/usr/share/zoneinfo/Asia/Tokyo"
	data, err := os.ReadFile(source)
	if err != nil {
		t.Skip(err)
	}
	path := filepath.Join(t.TempDir(), "localtime")
	if err = os.WriteFile(path, data, 0600); err != nil {
		t.Fatal(err)
	}
	loc, _, err := clockLocation(path)
	if err != nil {
		t.Fatal(err)
	}
	_, offset := time.Now().In(loc).Zone()
	if offset != 9*3600 {
		t.Fatal(offset)
	}
	link := filepath.Join(t.TempDir(), "localtime")
	if err = os.Symlink(source, link); err != nil {
		t.Fatal(err)
	}
	loc, _, err = clockLocation(link)
	if err != nil || loc.String() != "Asia/Tokyo" {
		t.Fatal(loc, err)
	}
}
