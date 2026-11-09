package edid_test

import (
	"errors"
	"testing"

	"whiterose/daemon/internal/edid"
	"whiterose/daemon/internal/edid/edidtest"
)

func TestParse(t *testing.T) {
	tests := []struct {
		name string
		in   edidtest.Builder
		want edid.Info
	}{
		{
			name: "full identity",
			in: edidtest.Builder{
				Manufacturer: "DEL", ProductCode: 0xA1B2, Serial: 0x4C4F5250,
				Week: 12, Year: 2023, Name: "DELL U2723QE", SerialString: "7F3K2Q3",
			},
			want: edid.Info{
				Manufacturer: "DEL", ProductCode: 0xA1B2, Serial: 0x4C4F5250,
				Week: 12, Year: 2023, Name: "DELL U2723QE", SerialString: "7F3K2Q3",
				Version: 1, Revision: 4, ChecksumOK: true,
			},
		},
		{
			name: "no serial, no descriptors",
			in:   edidtest.Builder{Manufacturer: "AUO", ProductCode: 0x123D, Year: 2021},
			want: edid.Info{Manufacturer: "AUO", ProductCode: 0x123D, Year: 2021, Version: 1, Revision: 4, ChecksumOK: true},
		},
		{
			name: "model year flag",
			in:   edidtest.Builder{Manufacturer: "GSM", Week: 0xFF, Year: 2020},
			want: edid.Info{Manufacturer: "GSM", ModelYear: true, Year: 2020, Version: 1, Revision: 4, ChecksumOK: true},
		},
		{
			name: "name fills all 13 bytes without terminator",
			in:   edidtest.Builder{Name: "ABCDEFGHIJKLM"},
			want: edid.Info{Manufacturer: "AAA", Name: "ABCDEFGHIJKLM", Version: 1, Revision: 4, ChecksumOK: true},
		},
		{
			name: "bad checksum still decodes",
			in:   edidtest.Builder{Manufacturer: "SAM", Serial: 42, BadChecksum: true},
			want: edid.Info{Manufacturer: "SAM", Serial: 42, Version: 1, Revision: 4, ChecksumOK: false},
		},
		{
			name: "extension count",
			in:   edidtest.Builder{Extensions: 2},
			want: edid.Info{Manufacturer: "AAA", Version: 1, Revision: 4, Extensions: 2, ChecksumOK: true},
		},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got, err := edid.Parse(tt.in.Bytes())
			if err != nil {
				t.Fatalf("Parse: %v", err)
			}
			if *got != tt.want {
				t.Errorf("Parse mismatch\n got: %+v\nwant: %+v", *got, tt.want)
			}
		})
	}
}

func TestParseIgnoresExtensionBlocks(t *testing.T) {
	data := append(edidtest.Builder{Manufacturer: "DEL", Extensions: 1}.Bytes(), make([]byte, 128)...)
	got, err := edid.Parse(data)
	if err != nil {
		t.Fatal(err)
	}
	if got.Manufacturer != "DEL" || !got.ChecksumOK {
		t.Errorf("got %+v", got)
	}
}

func TestParseErrors(t *testing.T) {
	valid := edidtest.Builder{}.Bytes()
	badHeader := append([]byte(nil), valid...)
	badHeader[0] = 0x01

	tests := []struct {
		name string
		in   []byte
		want error
	}{
		{"nil", nil, edid.ErrEmpty},
		{"empty", []byte{}, edid.ErrEmpty},
		{"short", valid[:127], edid.ErrShort},
		{"bad header", badHeader, edid.ErrHeader},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			_, err := edid.Parse(tt.in)
			if !errors.Is(err, tt.want) {
				t.Errorf("err = %v, want %v", err, tt.want)
			}
		})
	}
}

func TestDescriptorTextStripsNonPrintable(t *testing.T) {
	data := edidtest.Builder{Name: "X"}.Bytes()
	// Overwrite the name payload (slot at 72, text at +5) with junk + text.
	copy(data[77:], []byte{0x01, 'H', 'i', 0x7F, '!', 0x0A})
	fixChecksum(data)
	got, err := edid.Parse(data)
	if err != nil {
		t.Fatal(err)
	}
	if got.Name != "Hi!" {
		t.Errorf("Name = %q, want %q", got.Name, "Hi!")
	}
}

func fixChecksum(b []byte) {
	var sum byte
	for _, c := range b[:127] {
		sum += c
	}
	b[127] = -sum
}

func FuzzParse(f *testing.F) {
	f.Add(edidtest.Builder{}.Bytes())
	f.Add(edidtest.Builder{Manufacturer: "DEL", Serial: 1, Name: "DELL", SerialString: "ABC"}.Bytes())
	f.Add(edidtest.Builder{BadChecksum: true}.Bytes())
	f.Add([]byte{0x00, 0xFF})
	f.Fuzz(func(t *testing.T, data []byte) {
		info, err := edid.Parse(data)
		if err != nil {
			if info != nil {
				t.Fatal("non-nil info with error")
			}
			return
		}
		if len(info.Manufacturer) != 3 {
			t.Fatalf("manufacturer %q is not 3 chars", info.Manufacturer)
		}
		if len(info.Name) > 13 || len(info.SerialString) > 13 {
			t.Fatalf("descriptor text too long: %q / %q", info.Name, info.SerialString)
		}
	})
}
