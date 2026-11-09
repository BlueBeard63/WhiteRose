package apps

import (
	"bufio"
	"context"
	"errors"
	"fmt"
	"io"
	"net/http"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"time"
)

// Event is progress reported while installing (one JSON line each from the
// root helper to the daemon).
type Event struct {
	Step  string   `json:"step"`
	Apps  []string `json:"apps"`
	State string   `json:"state"` // running | done | failed
	Line  string   `json:"line,omitempty"`
	Error string   `json:"error,omitempty"`
}

// Run carries out the steps as root, reporting each through emit. A failed
// step fails its apps; the steps after it still run (one app's trouble
// doesn't stop the rest). Returns an error if anything failed.
func Run(ctx context.Context, d Distro, steps []Step, emit func(Event)) error {
	var failed []string
	for _, s := range steps {
		emit(Event{Step: s.Label, Apps: s.Apps, State: "running"})
		var err error
		if s.Repo != "" {
			err = addRepo(ctx, d, s.Repo)
		} else {
			err = runCmd(ctx, s.Cmd, func(line string) {
				emit(Event{Step: s.Label, Apps: s.Apps, State: "running", Line: line})
			})
		}
		if err != nil {
			failed = append(failed, s.Label)
			emit(Event{Step: s.Label, Apps: s.Apps, State: "failed", Error: err.Error()})
			continue
		}
		emit(Event{Step: s.Label, Apps: s.Apps, State: "done"})
	}
	if len(failed) > 0 {
		return fmt.Errorf("didn't finish: %s", strings.Join(failed, "; "))
	}
	return nil
}

func runCmd(ctx context.Context, argv []string, line func(string)) error {
	cmd := exec.CommandContext(ctx, argv[0], argv[1:]...)
	cmd.Env = append(os.Environ(), "DEBIAN_FRONTEND=noninteractive", "LC_ALL=C")
	out, err := cmd.StdoutPipe()
	if err != nil {
		return err
	}
	cmd.Stderr = cmd.Stdout
	if err := cmd.Start(); err != nil {
		return err
	}
	var last string
	sc := bufio.NewScanner(out)
	for sc.Scan() {
		if t := strings.TrimSpace(sc.Text()); t != "" {
			last = t
			line(t)
		}
	}
	scanErr := sc.Err()
	if err := cmd.Wait(); err != nil {
		if last != "" {
			return fmt.Errorf("%s: %s", filepath.Base(argv[0]), last)
		}
		return err
	}
	return scanErr
}

// addRepo puts a vendor repository in place: its key and its source file.
func addRepo(ctx context.Context, d Distro, name string) error {
	r, ok := Repos[name]
	if !ok {
		return fmt.Errorf("unknown repository %q", name)
	}
	switch d {
	case Ubuntu:
		key, err := fetch(ctx, r.KeyURL)
		if err != nil {
			return err
		}
		if !strings.Contains(string(key), "BEGIN PGP PUBLIC KEY BLOCK") {
			return errors.New("the signing key isn't a PGP key")
		}
		if err := writeFile(r.AptKeyPath, key); err != nil {
			return err
		}
		return writeFile(r.AptSourcesPath, []byte(r.AptSources))
	case Fedora:
		if r.DnfRepo == "" {
			return fmt.Errorf("%s has no Fedora repository", name)
		}
		if err := runCmd(ctx, []string{"rpm", "--import", r.KeyURL}, func(string) {}); err != nil {
			return err
		}
		return writeFile(r.DnfRepoPath, []byte(r.DnfRepo))
	}
	return fmt.Errorf("unsupported distro %q", d)
}

func fetch(ctx context.Context, url string) ([]byte, error) {
	ctx, cancel := context.WithTimeout(ctx, 30*time.Second)
	defer cancel()
	req, err := http.NewRequestWithContext(ctx, http.MethodGet, url, nil)
	if err != nil {
		return nil, err
	}
	resp, err := http.DefaultClient.Do(req)
	if err != nil {
		return nil, err
	}
	defer resp.Body.Close()
	if resp.StatusCode != http.StatusOK {
		return nil, fmt.Errorf("%s: %s", url, resp.Status)
	}
	return io.ReadAll(io.LimitReader(resp.Body, 1<<20))
}

// writeFile writes a system file atomically, readable by all.
func writeFile(path string, data []byte) error {
	tmp := path + ".whiterose-new"
	if err := os.WriteFile(tmp, data, 0o644); err != nil {
		return err
	}
	return os.Rename(tmp, path)
}

// Installed reports which apps in the catalogue are already there.
func Installed(d Distro) map[string]bool {
	out := map[string]bool{}
	for _, a := range Catalog {
		if a.Command != "" {
			if _, err := exec.LookPath(a.Command); err == nil {
				out[a.ID] = true
				continue
			}
		}
		if src := a.Sources[d]; src.Kind == Flatpak {
			out[a.ID] = exec.Command("flatpak", "info", src.Package).Run() == nil
		}
	}
	return out
}
