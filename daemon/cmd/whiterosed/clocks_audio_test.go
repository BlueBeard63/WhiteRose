package main

import (
	"context"
	"encoding/json"
	"errors"
	"io"
	"os"
	"path/filepath"
	"testing"
	"time"

	"whiterose/daemon/internal/clocks"
	"whiterose/daemon/internal/server"
)

func previewRequest(sequence int, stop bool) server.Request {
	args, _ := json.Marshal(map[string]any{"client": "test", "sequence": sequence, "stop": stop, "options": clocks.DefaultAlarmOptions()})
	return server.Request{Args: args}
}
func TestClockPreviewStopsAndRejectsLateStarts(t *testing.T) {
	p := &clockPreview{}
	validate := func(clocks.AlarmOptions) error { return nil }
	started := make(chan struct{})
	finished := make(chan struct{})
	play := func(ctx context.Context, _ clocks.AlarmOptions, _ int64) error {
		close(started)
		<-ctx.Done()
		return ctx.Err()
	}
	go func() { defer close(finished); p.request(previewRequest(1, false), validate, play) }()
	select {
	case <-started:
	case <-time.After(time.Second):
		t.Fatal("preview didn't start")
	}
	if _, err := p.request(previewRequest(2, true), validate, play); err != nil {
		t.Fatal(err)
	}
	select {
	case <-finished:
	case <-time.After(time.Second):
		t.Fatal("preview didn't stop")
	}
	if _, err := p.request(previewRequest(1, false), validate, func(context.Context, clocks.AlarmOptions, int64) error { t.Error("stale preview played"); return nil }); err != nil {
		t.Fatal(err)
	}
}
func TestClockRingHonorsDurationAndPreservesPerAlarmOptions(t *testing.T) {
	o := clocks.DefaultAlarmOptions()
	o.Sound = "bells"
	o.Volume = 25
	o.RingDuration = 1000
	o.FadeDuration = 500
	played := make(chan clocks.AlarmOptions, 1)
	start := time.Now()
	clockRing(context.Background(), clocks.Alert{Options: o, Elapsed: 900}, func(ctx context.Context, options clocks.AlarmOptions, elapsed int64) error {
		played <- options
		if elapsed < 900 {
			t.Error(elapsed)
		}
		<-ctx.Done()
		return ctx.Err()
	}, io.Discard)
	if time.Since(start) > time.Second {
		t.Fatal("ring limit ignored")
	}
	if <-played != o {
		t.Fatal("wrong alarm settings")
	}
}
func TestClockSoundUsesPrivateStreamGainNotSystemVolume(t *testing.T) {
	dir := t.TempDir()
	capture := filepath.Join(dir, "pcm")
	player := filepath.Join(dir, "pw-play")
	if err := os.WriteFile(player, []byte("#!/bin/sh\nexec /bin/cat > \"$CLOCK_TEST_PCM\"\n"), 0700); err != nil {
		t.Fatal(err)
	}
	t.Setenv("PATH", dir)
	t.Setenv("CLOCK_TEST_PCM", capture)
	o := clocks.DefaultAlarmOptions()
	o.Volume = 0
	if err := clockSound(context.Background(), o, 0); err != nil {
		t.Fatal(err)
	}
	pcm, err := os.ReadFile(capture)
	if err != nil || len(pcm) != clocks.AudioRate*2 {
		t.Fatal(err)
	}
	for _, v := range pcm {
		if v != 0 {
			t.Fatal("volume zero wasn't silent")
		}
	}
}
func TestClockPreviewReportsPlaybackFailure(t *testing.T) {
	p := &clockPreview{}
	_, err := p.request(previewRequest(1, false), func(clocks.AlarmOptions) error { return nil }, func(context.Context, clocks.AlarmOptions, int64) error { return errors.New("no audio output") })
	if err == nil {
		t.Fatal("preview hid playback failure")
	}
}
