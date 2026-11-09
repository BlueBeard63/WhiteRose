package accounts

import (
	"crypto/rand"
	"crypto/sha512"
	"strconv"
	"strings"
)

// SHA-512 crypt ("$6$", Ulrich Drepper's specification), the password hash
// /etc/shadow uses on Ubuntu and Fedora. AccountsService takes passwords
// already hashed, so they never travel over D-Bus in the clear.

const cryptAlphabet = "./0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz"

// Crypt hashes password with a fresh random salt.
func Crypt(password string) (string, error) {
	b := make([]byte, 16)
	if _, err := rand.Read(b); err != nil {
		return "", err
	}
	salt := make([]byte, 16)
	for i, v := range b {
		salt[i] = cryptAlphabet[int(v)%len(cryptAlphabet)]
	}
	return sha512Crypt(password, string(salt), 0), nil
}

// sha512Crypt is the algorithm itself; rounds 0 means the default 5000 (and
// leaves "rounds=" out of the result).
func sha512Crypt(password, salt string, rounds int) string {
	const defaultRounds = 5000
	explicit := rounds != 0
	if !explicit {
		rounds = defaultRounds
	}
	rounds = max(1000, min(rounds, 999999999))
	if len(salt) > 16 {
		salt = salt[:16]
	}
	pw, s := []byte(password), []byte(salt)

	b := sha512.New()
	b.Write(pw)
	b.Write(s)
	b.Write(pw)
	sumB := b.Sum(nil)

	a := sha512.New()
	a.Write(pw)
	a.Write(s)
	for n := len(pw); n > 0; n -= 64 {
		a.Write(sumB[:min(n, 64)])
	}
	for n := len(pw); n > 0; n >>= 1 {
		if n&1 != 0 {
			a.Write(sumB)
		} else {
			a.Write(pw)
		}
	}
	sumA := a.Sum(nil)

	dp := sha512.New()
	for range pw {
		dp.Write(pw)
	}
	p := repeat(dp.Sum(nil), len(pw))

	ds := sha512.New()
	for i := 0; i < 16+int(sumA[0]); i++ {
		ds.Write(s)
	}
	sb := repeat(ds.Sum(nil), len(s))

	c := sumA
	for i := 0; i < rounds; i++ {
		h := sha512.New()
		if i&1 != 0 {
			h.Write(p)
		} else {
			h.Write(c)
		}
		if i%3 != 0 {
			h.Write(sb)
		}
		if i%7 != 0 {
			h.Write(p)
		}
		if i&1 != 0 {
			h.Write(c)
		} else {
			h.Write(p)
		}
		c = h.Sum(nil)
	}

	var out strings.Builder
	out.WriteString("$6$")
	if explicit {
		out.WriteString("rounds=" + strconv.Itoa(rounds) + "$")
	}
	out.WriteString(salt + "$")
	enc := func(b2, b1, b0 byte, n int) {
		w := uint(b2)<<16 | uint(b1)<<8 | uint(b0)
		for ; n > 0; n-- {
			out.WriteByte(cryptAlphabet[w&0x3f])
			w >>= 6
		}
	}
	order := [][3]int{{0, 21, 42}, {22, 43, 1}, {44, 2, 23}, {3, 24, 45}, {25, 46, 4}, {47, 5, 26}, {6, 27, 48},
		{28, 49, 7}, {50, 8, 29}, {9, 30, 51}, {31, 52, 10}, {53, 11, 32}, {12, 33, 54}, {34, 55, 13},
		{56, 14, 35}, {15, 36, 57}, {37, 58, 16}, {59, 17, 38}, {18, 39, 60}, {40, 61, 19}, {62, 20, 41}}
	for _, o := range order {
		enc(c[o[0]], c[o[1]], c[o[2]], 4)
	}
	enc(0, 0, c[63], 2)
	return out.String()
}

func repeat(sum []byte, n int) []byte {
	out := make([]byte, 0, n)
	for len(out) < n {
		out = append(out, sum[:min(64, n-len(out))]...)
	}
	return out
}
