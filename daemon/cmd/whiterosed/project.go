package main

import (
	"errors"
	"fmt"
	"io"

	"whiterose/daemon/internal/project"
)

// whiterosed project FILE ACTION: what `make release` and `make changelog`
// read from project.yaml.
//
//	version          the version to build (tidied: v0.6-alpha-12 → 0.6.0-alpha.12)
//	prerelease       exit 0 if it's a pre-release, 1 if not
//	repo             owner/name on GitHub
//	notes [VERSION]  the release notes (the version's entry under releases)
//	changelog        CHANGELOG.md, every release
//	check            the file is right, and the version has an entry
func cmdProject(args []string, stdout, stderr io.Writer) int {
	if len(args) < 2 {
		fmt.Fprintln(stderr, "usage: whiterosed project FILE version|prerelease|repo|notes [VERSION]|changelog|check")
		return 2
	}
	p, err := project.Load(args[0])
	if err != nil {
		fmt.Fprintln(stderr, "whiterosed:", err)
		return 1
	}
	switch args[1] {
	case "version":
		fmt.Fprintln(stdout, p.Version)
	case "prerelease":
		if !project.Prerelease(p.Version) {
			return 1
		}
	case "repo":
		if p.Repo == "" {
			fmt.Fprintln(stderr, "whiterosed: no repo in", args[0])
			return 1
		}
		fmt.Fprintln(stdout, p.Repo)
	case "notes":
		v := p.Version
		if len(args) > 2 {
			v = args[2]
		}
		notes, err := p.Notes(v)
		if err != nil {
			if errors.Is(err, project.ErrNoEntry) {
				fmt.Fprintf(stderr, "whiterosed: %s has no entry under releases in %s yet: add what changed first\n", v, args[0])
			} else {
				fmt.Fprintln(stderr, "whiterosed:", err)
			}
			return 1
		}
		fmt.Fprint(stdout, notes)
	case "changelog":
		fmt.Fprint(stdout, p.Changelog())
	case "check":
		if p.Release(p.Version) == nil {
			fmt.Fprintf(stderr, "whiterosed: %s has no entry under releases in %s yet\n", p.Version, args[0])
			return 1
		}
		fmt.Fprintf(stdout, "%s %s: fine\n", args[0], p.Version)
	default:
		fmt.Fprintf(stderr, "whiterosed: project %q? (version, prerelease, repo, notes, changelog, check)\n", args[1])
		return 2
	}
	return 0
}
