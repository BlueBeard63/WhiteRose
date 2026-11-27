package main

import (
	"bytes"
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"os/exec"
	"sync"
	"time"

	"whiterose/daemon/internal/clocks"
	"whiterose/daemon/internal/server"
)

func clockSound(ctx context.Context, options clocks.AlarmOptions, elapsed int64) error {
	pcm, err := clocks.SoundPCM(options.Sound, clocks.SoundDirectory())
	if err != nil {
		return err
	}
	pcm = clocks.Gain(pcm, options, elapsed)
	for _, player := range []struct {
		name string
		args []string
	}{
		{"pw-play", []string{"--raw", "--format", "s16", "--rate", "24000", "--channels", "1", "--media-role", "Alarm", "-"}},
		{"paplay", []string{"--raw", "--format=s16le", "--rate=24000", "--channels=1"}},
	} {
		if bin, e := exec.LookPath(player.name); e == nil {
			cmd := exec.CommandContext(ctx, bin, player.args...)
			cmd.Stdin = bytes.NewReader(pcm)
			if err = cmd.Run(); err == nil {
				return nil
			}
			if ctx.Err() != nil {
				return ctx.Err()
			}
		}
	}
	return fmt.Errorf("Couldn't play alarm audio; check the audio output and PipeWire/PulseAudio player")
}

func clockRing(ctx context.Context, alert clocks.Alert, play func(context.Context, clocks.AlarmOptions, int64) error, log io.Writer) {
	if alert.Options.RingDuration > 0 {
		var cancel context.CancelFunc
		ctx, cancel = context.WithTimeout(ctx, time.Duration(max(int64(0), alert.Options.RingDuration-alert.Elapsed))*time.Millisecond)
		defer cancel()
	}
	start := clocks.Now().Boot
	for ctx.Err() == nil {
		elapsed := alert.Elapsed + max(int64(0), clocks.Now().Boot-start)
		if alert.Options.RingDuration > 0 && elapsed >= alert.Options.RingDuration {
			return
		}
		clip, cancel := context.WithTimeout(ctx, 61*time.Second)
		err := play(clip, alert.Options, elapsed)
		cancel()
		if err != nil && ctx.Err() == nil {
			fmt.Fprintln(log, "whiterosed: alarm sound:", err)
			// Never silently lose an alarm because imported audio was removed
			// or corrupted. Fall back to Classic for this playback attempt.
			fallback := alert.Options
			fallback.Sound = "chime"
			fallbackContext, fallbackCancel := context.WithTimeout(ctx, 2*time.Second)
			_ = play(fallbackContext, fallback, elapsed)
			fallbackCancel()
		}
		pause := time.NewTimer(3 * time.Second)
		select {
		case <-ctx.Done():
			pause.Stop()
			return
		case <-pause.C:
		}
	}
}

type clockPreview struct {
	mu     sync.Mutex
	cancel context.CancelFunc
	owner  string
	latest map[string]uint64
}

func (p *clockPreview) request(r server.Request, validate func(clocks.AlarmOptions) error, play func(context.Context, clocks.AlarmOptions, int64) error) (any, error) {
	var x struct {
		Client   string              `json:"client"`
		Sequence uint64              `json:"sequence"`
		Stop     bool                `json:"stop"`
		Options  clocks.AlarmOptions `json:"options"`
	}
	if err := json.Unmarshal(r.Args, &x); err != nil {
		return nil, err
	}
	if x.Client == "" || len(x.Client) > 100 {
		return nil, errors.New("Invalid preview client")
	}
	if !x.Stop {
		if err := validate(x.Options); err != nil {
			return nil, err
		}
	}
	p.mu.Lock()
	if p.latest == nil {
		p.latest = map[string]uint64{}
	}
	if x.Sequence <= p.latest[x.Client] {
		p.mu.Unlock()
		return nil, nil
	}
	if len(p.latest) > 128 {
		p.latest = map[string]uint64{}
	}
	p.latest[x.Client] = x.Sequence
	if x.Stop {
		if p.owner == x.Client && p.cancel != nil {
			p.cancel()
		}
		p.mu.Unlock()
		return nil, nil
	}
	if p.cancel != nil {
		p.cancel()
	}
	ctx, cancel := context.WithTimeout(context.Background(), 8*time.Second)
	p.cancel = cancel
	p.owner = x.Client
	p.mu.Unlock()
	defer cancel()
	if x.Options.Volume == 0 {
		return nil, nil
	}
	err := play(ctx, x.Options, 0)
	if ctx.Err() != nil {
		return nil, nil
	}
	return nil, err
}

func (p *clockPreview) stop() {
	p.mu.Lock()
	defer p.mu.Unlock()
	if p.cancel != nil {
		p.cancel()
	}
}
