// Package project reads project.yaml, at the top of the repository: the
// version being built, the GitHub repository releases go to, and every
// release's changelog (what changed, who helped). `make release` takes the
// version and the release notes from it, and CHANGELOG.md is written from
// it, so the release history lives in one place.
//
//	name: WhiteRose
//	repo: BlueBeard63/WhiteRose
//	version: 0.6.0-alpha.12
//
//	releases:
//	  - version: 0.6.0-alpha.12
//	    date: 2026-12-01
//	    title: Login
//	    summary: One paragraph about the release.
//	    added: [ … ]
//	    changed: [ … ]
//	    fixed: [ … ]
//	    removed: [ … ]
//	    contributors: [BlueBeard63]
//	    notes: |
//	      Anything else, in Markdown.
//
// Versions are semantic versions: 0.6.0, or a pre-release such as
// 0.6.0-alpha.12. Looser spellings are tidied ("v0.6-alpha-12" is
// 0.6.0-alpha.12).
package project

import (
	"errors"
	"fmt"
	"os"
	"regexp"
	"strconv"
	"strings"
	"time"

	"gopkg.in/yaml.v3"
)

type Release struct {
	Version      string   `yaml:"version"`
	Date         string   `yaml:"date"`
	Title        string   `yaml:"title"`
	Summary      string   `yaml:"summary"`
	Added        []string `yaml:"added"`
	Changed      []string `yaml:"changed"`
	Fixed        []string `yaml:"fixed"`
	Removed      []string `yaml:"removed"`
	Contributors []string `yaml:"contributors"`
	Notes        string   `yaml:"notes"`
}

type Project struct {
	Name     string    `yaml:"name"`
	Repo     string    `yaml:"repo"`
	Version  string    `yaml:"version"`
	Releases []Release `yaml:"releases"`
}

// Load reads and checks project.yaml (a key it doesn't know is a mistake:
// a typo would otherwise vanish quietly).
func Load(path string) (*Project, error) {
	f, err := os.Open(path)
	if err != nil {
		return nil, err
	}
	defer f.Close()
	var p Project
	d := yaml.NewDecoder(f)
	d.KnownFields(true)
	if err := d.Decode(&p); err != nil {
		return nil, fmt.Errorf("%s: %w", path, err)
	}
	if err := p.check(); err != nil {
		return nil, fmt.Errorf("%s: %w", path, err)
	}
	return &p, nil
}

var (
	looseRe  = regexp.MustCompile(`^v?(\d+)(?:\.(\d+))?(?:\.(\d+))?(?:[-~](.+))?$`)
	preRe    = regexp.MustCompile(`^[0-9a-z]+(\.[0-9a-z]+)*$`)
	githubRe = regexp.MustCompile(`^[A-Za-z0-9](?:[A-Za-z0-9-]{0,38})$`)
)

// Normalize tidies a version: "v0.6-alpha-12" → "0.6.0-alpha.12". It
// refuses what isn't a version (the result names packages and folders).
func Normalize(v string) (string, error) {
	m := looseRe.FindStringSubmatch(strings.TrimSpace(v))
	if m == nil {
		return "", fmt.Errorf("%q isn't a version (like 0.6.0, or 0.6.0-alpha.12)", v)
	}
	num := func(s string) string { n, _ := strconv.Atoi(s); return strconv.Itoa(n) } // ("" and "007" too)
	out := num(m[1]) + "." + num(m[2]) + "." + num(m[3])
	if m[4] != "" {
		pre := strings.ToLower(strings.ReplaceAll(m[4], "-", "."))
		if !preRe.MatchString(pre) {
			return "", fmt.Errorf("%q: a pre-release is letters and numbers, like alpha.12", v)
		}
		out += "-" + pre
	}
	return out, nil
}

// Prerelease says whether a version is a pre-release (has a "-…").
func Prerelease(v string) bool { return strings.Contains(v, "-") }

func (p *Project) check() error {
	if p.Repo != "" && !regexp.MustCompile(`^[\w.-]+/[\w.-]+$`).MatchString(p.Repo) {
		return fmt.Errorf("repo %q: want owner/name", p.Repo)
	}
	v, err := Normalize(p.Version)
	if err != nil {
		return fmt.Errorf("version: %w", err)
	}
	p.Version = v
	seen := map[string]bool{}
	for i := range p.Releases {
		r := &p.Releases[i]
		rv, err := Normalize(r.Version)
		if err != nil {
			return fmt.Errorf("release %d: %w", i+1, err)
		}
		if seen[rv] {
			return fmt.Errorf("release %s is there twice", rv)
		}
		seen[rv] = true
		r.Version = rv
		if r.Date != "" {
			if _, err := time.Parse("2006-01-02", r.Date); err != nil {
				return fmt.Errorf("release %s: date %q: want YYYY-MM-DD", rv, r.Date)
			}
		}
		for _, c := range r.Contributors {
			if !githubRe.MatchString(strings.TrimPrefix(c, "@")) {
				return fmt.Errorf("release %s: contributor %q: want a GitHub username", rv, c)
			}
		}
	}
	return nil
}

// (Releases are listed newest first, as CHANGELOG.md shows them.)

// Release is the changelog entry for version v (nil if there's none).
func (p *Project) Release(v string) *Release {
	n, err := Normalize(v)
	if err != nil {
		return nil
	}
	for i := range p.Releases {
		if p.Releases[i].Version == n {
			return &p.Releases[i]
		}
	}
	return nil
}

// ErrNoEntry: the version has nothing in the changelog yet.
var ErrNoEntry = errors.New("no entry under releases for this version in project.yaml")

// sections in the order they're shown; the first bullets are what
// Settings › Updates shows of a release, so what's new comes first.
func (r *Release) sections() [][2]any {
	return [][2]any{{"Added", r.Added}, {"Changed", r.Changed}, {"Fixed", r.Fixed}, {"Removed", r.Removed}}
}

func bullets(b *strings.Builder, title string, items []string, level string) {
	if len(items) == 0 {
		return
	}
	fmt.Fprintf(b, "%s %s\n\n", level, title)
	for _, it := range items {
		fmt.Fprintf(b, "- %s\n", strings.TrimSpace(it))
	}
	b.WriteString("\n")
}

func contributors(names []string) string {
	var out []string
	for _, c := range names {
		out = append(out, "@"+strings.TrimPrefix(c, "@"))
	}
	return strings.Join(out, ", ")
}

// Notes are the GitHub release notes for version v: what changed, who
// helped, any notes, and how to install.
func (p *Project) Notes(v string) (string, error) {
	r := p.Release(v)
	if r == nil {
		return "", ErrNoEntry
	}
	var b strings.Builder
	if r.Summary != "" {
		fmt.Fprint(&b, strings.TrimSpace(r.Summary), "\n\n")
	}
	for _, s := range r.sections() {
		bullets(&b, s[0].(string), s[1].([]string), "##")
	}
	if n := strings.TrimSpace(r.Notes); n != "" {
		fmt.Fprint(&b, n, "\n\n")
	}
	if len(r.Contributors) > 0 {
		fmt.Fprintf(&b, "## Contributors\n\nThanks to %s.\n\n", contributors(r.Contributors))
	}
	pkg := strings.ReplaceAll(r.Version, "~", "-")
	b.WriteString("## Installing\n\n")
	fmt.Fprintf(&b, "Ubuntu: `whiterose_%s_amd64.deb` and `whiterose-core-%s_%s_amd64.deb`.\n", pkg, pkg, pkg)
	fmt.Fprintf(&b, "Fedora: `whiterose-%s-1.x86_64.rpm` and `whiterose-core-%s-%s-1.x86_64.rpm`.\n\n", pkg, pkg, pkg)
	b.WriteString("Every file is in `SHA256SUMS`, signed in `SHA256SUMS.sig`. WhiteRose only installs updates signed with the release key; after this they come through **Settings › Updates**.\n")
	if Prerelease(r.Version) {
		b.WriteString("\nThis is a pre-release: for trying out, not yet for every day.\n")
	}
	return b.String(), nil
}

// Changelog is CHANGELOG.md: every release, newest first.
func (p *Project) Changelog() string {
	var b strings.Builder
	name := p.Name
	if name == "" {
		name = "The project"
	}
	fmt.Fprintf(&b, "# Changelog\n\nEvery %s release, newest first. Written from `project.yaml` by `make changelog`; edit that, not this.\n\n", name)
	for _, r := range p.Releases {
		head := "## " + r.Version
		if r.Title != "" {
			head += " · " + r.Title
		}
		if r.Date != "" {
			head += " (" + r.Date + ")"
		}
		fmt.Fprint(&b, head, "\n\n")
		if r.Summary != "" {
			fmt.Fprint(&b, strings.TrimSpace(r.Summary), "\n\n")
		}
		for _, s := range r.sections() {
			bullets(&b, s[0].(string), s[1].([]string), "###")
		}
		if n := strings.TrimSpace(r.Notes); n != "" {
			fmt.Fprint(&b, n, "\n\n")
		}
		if len(r.Contributors) > 0 {
			fmt.Fprintf(&b, "Contributors: %s\n\n", contributors(r.Contributors))
		}
	}
	return strings.TrimRight(b.String(), "\n") + "\n"
}

// Exists reports whether the file is there (for a clear message first).
func Exists(path string) bool { _, err := os.Stat(path); return err == nil }
