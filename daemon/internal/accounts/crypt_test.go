package accounts

import (
	"strings"
	"testing"
)

// Test vectors from the SHA-crypt specification (Drepper).
func TestSHA512CryptVectors(t *testing.T) {
	cases := []struct {
		pw, salt string
		rounds   int
		want     string
	}{
		{"Hello world!", "saltstring", 0,
			"$6$saltstring$svn8UoSVapNtMuq1ukKS4tPQd8iKwSMHWjl/O817G3uBnIFNjnQJuesI68u4OTLiBFdcbYEdFCoEOfaS35inz1"},
		{"Hello world!", "saltstringsaltstring", 10000,
			"$6$rounds=10000$saltstringsaltst$OW1/O6BYHV6BcXZu8QVeXbDWra3Oeqh0sbHbbMCVNSnCM/UrjmM0Dp8vOuZeHBy/YTBmSK6H9qs/y3RnOaw5v."},
		{"This is just a test", "toolongsaltstring", 5000,
			"$6$rounds=5000$toolongsaltstrin$lQ8jolhgVRVhY4b5pZKaysCLi0QBxGoNeKQzQ3glMhwllF7oGDZxUhx1yxdYcz/e1JSbq3y6JMxxl8audkUEm0"},
		{"we have a short salt string but not a short password", "short", 77777,
			"$6$rounds=77777$short$WuQyW2YR.hBNpjjRhpYD/ifIw05xdfeEyQoMxIXbkvr0gge1a1x3yRULJ5CCaUeOxFmtlcGZelFl5CxtgfiAc0"},
	}
	for _, c := range cases {
		if got := sha512Crypt(c.pw, c.salt, c.rounds); got != c.want {
			t.Errorf("sha512Crypt(%q, %q, %d)\n got %s\nwant %s", c.pw, c.salt, c.rounds, got, c.want)
		}
	}
}

func TestCryptUsesAFreshSalt(t *testing.T) {
	a, _ := Crypt("secret")
	b, _ := Crypt("secret")
	if a == b || !strings.HasPrefix(a, "$6$") {
		t.Fatalf("salts not random or wrong format: %s %s", a, b)
	}
	salt := strings.Split(a, "$")[2]
	if sha512Crypt("secret", salt, 0) != a {
		t.Fatal("Crypt doesn't match the algorithm with its own salt")
	}
}

func TestUserNames(t *testing.T) {
	for _, ok := range []string{"jack", "a", "anna-maria", "_svc", "j2"} {
		if err := CheckUserName(ok); err != nil {
			t.Errorf("%q refused: %v", ok, err)
		}
	}
	for _, bad := range []string{"", "Jack", "1jack", "jack smith", "root", "way-too-long-for-a-unix-username-x", "ünï"} {
		if CheckUserName(bad) == nil {
			t.Errorf("%q accepted", bad)
		}
	}
}

func TestNamesWithColonsAreRefused(t *testing.T) {
	if checkRealName("Jack: Admin") == nil || checkRealName("a\nb") == nil || checkRealName("Jack Morrison") != nil {
		t.Fatal("real name check")
	}
}

func TestYouCantLockYourselfOut(t *testing.T) {
	s := &Service{me: 1000}
	if s.SetAdmin(1000, false) == nil {
		t.Error("removed own admin rights")
	}
	if s.Delete(1000, true) == nil {
		t.Error("deleted own account")
	}
}
