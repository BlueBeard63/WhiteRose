package main

import (
	"encoding/json"
	"errors"
	"sync"
	"whiterose/daemon/internal/memwatch"
	"whiterose/daemon/internal/server"
	"whiterose/daemon/internal/tasks"
)

func taskOps(mem *memWatch) map[string]func(server.Request) (any, error) {
	sampler := tasks.New(mem.slice)
	var edit sync.Mutex
	return map[string]func(server.Request) (any, error){
		"tasks_state": func(server.Request) (any, error) {
			state := sampler.Sample()
			protection, _ := mem.ops()["memory_status"](server.Request{})
			var protected []string
			_ = mem.st.GetInto("memory.protected", &protected)
			return map[string]any{"system": state, "protection": protection, "protected": protected, "auto_close": mem.autoClose()}, nil
		},
		"tasks_end": func(r server.Request) (any, error) {
			var args struct {
				Unit  string `json:"unit"`
				Token string `json:"token"`
				Force bool   `json:"force"`
			}
			if err := json.Unmarshal(r.Args, &args); err != nil {
				return nil, err
			}
			return nil, sampler.End(args.Unit, args.Token, args.Force)
		},
		"tasks_protect": func(r server.Request) (any, error) {
			var args struct {
				ID string `json:"id"`
				On bool   `json:"on"`
			}
			if err := json.Unmarshal(r.Args, &args); err != nil {
				return nil, err
			}
			if args.ID == "" {
				return nil, errors.New("choose an app")
			}
			edit.Lock()
			defer edit.Unlock()
			var list []string
			if err := mem.st.GetInto("memory.protected", &list); err != nil {
				return nil, err
			}
			next := []string{}
			for _, id := range list {
				if !memwatch.Protected(args.ID, []string{id}, "") {
					next = append(next, id)
				}
			}
			if args.On {
				next = append(next, args.ID)
			}
			return nil, mem.st.SetValue("memory.protected", next)
		},
	}
}
