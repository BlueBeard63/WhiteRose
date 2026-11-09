// Package edid decodes the identity fields of a monitor's EDID base block.
//
// Only the fields WhiteRose needs to identify a monitor are decoded:
// manufacturer, product code, serial number, manufacture date and the text
// descriptors. Timing data is left to the compositor.
package edid

import (
	"bytes"
	"errors"
	"fmt"
	"strings"
)

// BlockSize is the size of an EDID base block (and of each extension block).
const BlockSize = 128

var header = []byte{0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x00}

var (
	// ErrEmpty means no EDID data was supplied (e.g. a disconnected connector).
	ErrEmpty = errors.New("edid: empty")
	// ErrShort means the data is smaller than one base block.
	ErrShort = errors.New("edid: shorter than 128 bytes")
	// ErrHeader means the fixed 8-byte header pattern is missing.
	ErrHeader = errors.New("edid: invalid header")
)

// Descriptor tags for 18-byte display descriptors.
const (
	tagSerialString = 0xFF
	tagText         = 0xFE
	tagName         = 0xFC
)

// Info holds the identity fields of an EDID base block.
type Info struct {
	// Manufacturer is the 3-letter PNP ID, e.g. "DEL".
	Manufacturer string
	// ProductCode is the manufacturer's product code.
	ProductCode uint16
	// Serial is the 32-bit serial number. Many monitors leave it 0.
	Serial uint32
	// Week of manufacture (1-54), or 0 if unspecified. When ModelYear is
	// true, Year is the model year and Week is 0.
	Week int
	// Year of manufacture (or model year).
	Year      int
	ModelYear bool
	// Name is the monitor name descriptor (0xFC), e.g. "DELL U2723QE".
	Name string
	// SerialString is the serial string descriptor (0xFF), if present.
	SerialString string
	// Version and Revision of the EDID structure, e.g. 1 and 4.
	Version, Revision int
	// Extensions is the number of extension blocks the base block announces.
	Extensions int
	// ChecksumOK reports whether the base block checksum is valid. Some
	// monitors ship with a bad checksum; the identity fields are still
	// usable, so this is reported rather than treated as fatal.
	ChecksumOK bool
}

// Parse decodes the base block of data. Extension blocks are ignored.
// Structural problems (empty, too short, bad header) are errors; a bad
// checksum is reported through Info.ChecksumOK.
func Parse(data []byte) (*Info, error) {
	if len(data) == 0 {
		return nil, ErrEmpty
	}
	if len(data) < BlockSize {
		return nil, fmt.Errorf("%w (got %d)", ErrShort, len(data))
	}
	b := data[:BlockSize]
	if !bytes.Equal(b[:8], header) {
		return nil, ErrHeader
	}

	info := &Info{
		Manufacturer: decodePNP(uint16(b[8])<<8 | uint16(b[9])),
		ProductCode:  uint16(b[10]) | uint16(b[11])<<8,
		Serial:       uint32(b[12]) | uint32(b[13])<<8 | uint32(b[14])<<16 | uint32(b[15])<<24,
		Version:      int(b[18]),
		Revision:     int(b[19]),
		Extensions:   int(b[126]),
		ChecksumOK:   checksum(b) == 0,
	}

	switch week := b[16]; week {
	case 0xFF:
		info.ModelYear = true
	default:
		info.Week = int(week)
	}
	if b[17] != 0 {
		info.Year = 1990 + int(b[17])
	}

	for off := 54; off+18 <= 126; off += 18 {
		d := b[off : off+18]
		// Display descriptors have a zero pixel clock and a zero reserved byte.
		if d[0] != 0 || d[1] != 0 || d[2] != 0 {
			continue
		}
		switch d[3] {
		case tagName:
			info.Name = descriptorText(d[5:])
		case tagSerialString:
			info.SerialString = descriptorText(d[5:])
		}
	}
	return info, nil
}

// decodePNP converts the packed 3x5-bit manufacturer ID into letters.
// Invalid letters are rendered as '?'.
func decodePNP(v uint16) string {
	letters := [3]byte{}
	for i, shift := range []uint{10, 5, 0} {
		c := (v >> shift) & 0x1F
		if c >= 1 && c <= 26 {
			letters[i] = 'A' + byte(c-1)
		} else {
			letters[i] = '?'
		}
	}
	return string(letters[:])
}

// descriptorText decodes the 13-byte text payload of a display descriptor:
// terminated by 0x0A, padded with spaces. Non-printable bytes are dropped.
func descriptorText(p []byte) string {
	if i := bytes.IndexByte(p, 0x0A); i >= 0 {
		p = p[:i]
	}
	var sb strings.Builder
	for _, c := range p {
		if c >= 0x20 && c < 0x7F {
			sb.WriteByte(c)
		}
	}
	return strings.TrimSpace(sb.String())
}

func checksum(b []byte) byte {
	var sum byte
	for _, c := range b {
		sum += c
	}
	return sum
}
