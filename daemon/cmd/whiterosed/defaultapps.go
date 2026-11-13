package main

// Settings › Apps › Default apps (internal/mimeapps): which app opens what
// in a WhiteRose session. Your choices are saved in
// ~/.config/whiterose-mimeapps.list, which only a WhiteRose session reads.

import (
	"errors"
	"strings"

	"whiterose/daemon/internal/mimeapps"
	"whiterose/daemon/internal/server"
)

type appView struct {
	ID   string `json:"id"`
	Name string `json:"name"`
	Icon string `json:"icon"`
}

func viewOf(s *mimeapps.State, id string) *appView {
	if a, ok := s.Apps[id]; ok {
		return &appView{ID: a.ID, Name: a.Name, Icon: a.Icon}
	}
	return nil
}

func viewsOf(apps []mimeapps.App) []appView {
	out := []appView{}
	for _, a := range apps {
		out = append(out, appView{ID: a.ID, Name: a.Name, Icon: a.Icon})
	}
	return out
}

// typesFor: a row's key as its types ("documents", or "type:text/csv").
func typesFor(key string) ([]string, error) {
	if m, ok := strings.CutPrefix(key, "type:"); ok && m != "" && !strings.ContainsAny(m, "=;\n[]") {
		return []string{m}, nil
	}
	if c, ok := mimeapps.CategoryByKey(key); ok {
		return c.Types, nil
	}
	return nil, errors.New("no such kind of file")
}

func defaultAppsOps() map[string]func(server.Request) (any, error) {
	dirs := mimeapps.FromEnv
	return map[string]func(server.Request) (any, error){
		// The categories (what opens each, and what could), and every type
		// you've set yourself.
		"default_apps": func(server.Request) (any, error) {
			s := mimeapps.Load(dirs())
			cats := []map[string]any{}
			for _, c := range mimeapps.Categories {
				capable := map[string]mimeapps.App{}
				for _, t := range c.Types {
					for _, a := range s.Capable(t) {
						capable[a.ID] = a
					}
				}
				list := make([]mimeapps.App, 0, len(capable))
				for _, a := range capable {
					list = append(list, a)
				}
				changed := false
				for _, t := range c.Types {
					changed = changed || s.Changed(t)
				}
				// What opens its main type, else the first of the others that
				// has something.
				current := s.Default(c.Show)
				for _, t := range c.Types {
					if current != "" {
						break
					}
					current = s.Default(t)
				}
				cats = append(cats, map[string]any{
					"key": c.Key, "name": c.Name, "current": viewOf(s, current),
					"apps": viewsOf(sortApps(list)), "changed": changed,
				})
			}
			globs := s.Dirs.Globs()
			exts := map[string][]string{}
			for e, m := range globs {
				exts[m] = append(exts[m], e)
			}
			mine := []map[string]any{}
			for _, m := range s.ChangedTypes() {
				mine = append(mine, map[string]any{"mime": m, "exts": exts[m], "current": viewOf(s, s.Default(m))})
			}
			return map[string]any{"categories": cats, "changed": mine}, nil
		},
		// A type (or extension) looked up: what opens it, and what could.
		"default_apps_search": func(r server.Request) (any, error) {
			s := mimeapps.Load(dirs())
			out := []map[string]any{}
			for _, tm := range mimeapps.Search(r.Name, s.Dirs.Globs(), s.Apps) {
				out = append(out, map[string]any{"mime": tm.Mime, "exts": tm.Exts, "current": viewOf(s, s.Default(tm.Mime)),
					"apps": viewsOf(s.Capable(tm.Mime)), "changed": s.Changed(tm.Mime)})
			}
			return out, nil
		},
		// Key: a category or "type:<mime>"; Name: the app's id.
		"default_apps_set": func(r server.Request) (any, error) {
			types, err := typesFor(r.Key)
			if err != nil {
				return nil, err
			}
			s := mimeapps.Load(dirs())
			if _, ok := s.Apps[r.Name]; !ok {
				return nil, errors.New("that app isn't installed")
			}
			return nil, s.Set(types, r.Name)
		},
		"default_apps_reset": func(r server.Request) (any, error) {
			types, err := typesFor(r.Key)
			if err != nil {
				return nil, err
			}
			return nil, mimeapps.Load(dirs()).Reset(types)
		},
	}
}

func sortApps(a []mimeapps.App) []mimeapps.App {
	for i := 1; i < len(a); i++ {
		for j := i; j > 0 && strings.ToLower(a[j].Name) < strings.ToLower(a[j-1].Name); j-- {
			a[j], a[j-1] = a[j-1], a[j]
		}
	}
	return a
}
