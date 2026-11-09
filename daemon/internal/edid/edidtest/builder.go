// Package edidtest builds synthetic EDID base blocks for tests.
package edidtest

// Builder describes an EDID base block. The zero value builds a valid block
// with manufacturer "AAA" and no descriptors.
type Builder struct {
	Manufacturer string // 3 uppercase letters; defaults to "AAA"
	ProductCode  uint16
	Serial       uint32
	Week         byte // 0xFF marks a model year
	Year         int  // full year, e.g. 2023; 0 leaves the byte empty
	Name         string
	SerialString string
	Extensions   byte

	// BadChecksum corrupts the checksum byte.
	BadChecksum bool
}

// Bytes encodes the block. The result is always 128 bytes.
func (b Builder) Bytes() []byte {
	out := make([]byte, 128)
	copy(out, []byte{0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x00})

	mfr := b.Manufacturer
	if mfr == "" {
		mfr = "AAA"
	}
	var packed uint16
	for i := 0; i < 3; i++ {
		packed = packed<<5 | uint16(mfr[i]-'A'+1)&0x1F
	}
	out[8], out[9] = byte(packed>>8), byte(packed)
	out[10], out[11] = byte(b.ProductCode), byte(b.ProductCode>>8)
	out[12], out[13], out[14], out[15] = byte(b.Serial), byte(b.Serial>>8), byte(b.Serial>>16), byte(b.Serial>>24)
	out[16] = b.Week
	if b.Year != 0 {
		out[17] = byte(b.Year - 1990)
	}
	out[18], out[19] = 1, 4

	// Descriptor slot 0 holds a fake detailed timing (non-zero pixel clock)
	// so parsers must skip it, as they would on real hardware.
	out[54], out[55] = 0x01, 0x1D

	slot := 72
	put := func(tag byte, text string) {
		d := out[slot : slot+18]
		d[3] = tag
		p := d[5:]
		for i := range p {
			p[i] = ' '
		}
		n := copy(p, text)
		if n < len(p) {
			p[n] = 0x0A
		}
		slot += 18
	}
	if b.Name != "" {
		put(0xFC, b.Name)
	}
	if b.SerialString != "" {
		put(0xFF, b.SerialString)
	}

	out[126] = b.Extensions
	var sum byte
	for _, c := range out[:127] {
		sum += c
	}
	out[127] = -sum
	if b.BadChecksum {
		out[127]++
	}
	return out
}
