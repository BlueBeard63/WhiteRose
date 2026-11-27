package clocks

import (
	"bytes"
	"context"
	"crypto/sha256"
	"encoding/binary"
	"errors"
	"fmt"
	"io"
	"math"
	"net/url"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"syscall"
)

const AudioRate = 24000
const MaxAudioBytes = 60 * AudioRate * 2

func SoundDirectory() string { return filepath.Join(filepath.Dir(Path()), "sounds") }

type audioBuffer struct{ bytes.Buffer }

func (b *audioBuffer) Write(p []byte) (int, error) {
	if b.Len()+len(p) > MaxAudioBytes {
		return 0, errors.New("Audio is too large")
	}
	return b.Buffer.Write(p)
}

// Imported audio is normalized once, bounded and copied into private storage.
// Future alarms don't depend on the original file remaining in place.
func ImportSound(ctx context.Context, path, directory string) (Sound, error) {
	if strings.HasPrefix(path, "file:") {
		u, err := url.Parse(path)
		if err != nil || u.Host != "" && u.Host != "localhost" {
			return Sound{}, errors.New("Choose a local audio file")
		}
		path = u.Path
	}
	if !filepath.IsAbs(path) {
		return Sound{}, errors.New("Choose a local audio file")
	}
	f, err := os.OpenFile(path, os.O_RDONLY|syscall.O_NONBLOCK, 0)
	if err != nil {
		return Sound{}, errors.New("Couldn't open the audio file")
	}
	defer f.Close()
	info, err := f.Stat()
	if err != nil || !info.Mode().IsRegular() || info.Size() > 20<<20 {
		return Sound{}, errors.New("Choose an audio file no larger than 20 MB")
	}
	var pcm audioBuffer
	// Pass the already-open regular file, not a URL, playlist or shell command.
	cmd := exec.CommandContext(ctx, "ffmpeg", "-nostdin", "-v", "error", "-protocol_whitelist", "file,pipe", "-format_whitelist", "wav,flac,ogg,mp3,mov,matroska,webm", "-i", "/proc/self/fd/3", "-t", "60", "-map", "0:a:0", "-vn", "-ac", "1", "-ar", "24000", "-f", "s16le", "pipe:1")
	cmd.ExtraFiles = []*os.File{f}
	cmd.Stdout = &pcm
	if err = cmd.Run(); err != nil {
		if ctx.Err() != nil {
			return Sound{}, errors.New("Audio import took too long")
		}
		return Sound{}, errors.New("Couldn't read this audio file; install FFmpeg and choose WAV, FLAC, Ogg, MP3 or M4A audio")
	}
	if pcm.Len() < 2 || pcm.Len()%2 != 0 {
		return Sound{}, errors.New("The file contains no usable audio")
	}
	sum := sha256.Sum256(pcm.Bytes())
	id := fmt.Sprintf("custom:%x", sum)
	if err = os.MkdirAll(directory, 0700); err != nil {
		return Sound{}, err
	}
	tmp, err := os.CreateTemp(directory, ".sound-*")
	if err != nil {
		return Sound{}, err
	}
	defer os.Remove(tmp.Name())
	_, err = tmp.Write(WAV(pcm.Bytes()))
	if err == nil {
		err = tmp.Sync()
	}
	closeErr := tmp.Close()
	if err == nil {
		err = closeErr
	}
	if err != nil {
		return Sound{}, err
	}
	if err = os.Rename(tmp.Name(), filepath.Join(directory, id[len("custom:"):]+".wav")); err != nil {
		return Sound{}, err
	}
	name := strings.TrimSpace(strings.TrimSuffix(filepath.Base(path), filepath.Ext(path)))
	if !validName(name) {
		name = "Custom audio"
	}
	return Sound{ID: id, Name: name, Custom: true}, nil
}

func SoundPCM(sound, directory string) ([]byte, error) {
	if IsCustomSound(sound) {
		path := filepath.Join(directory, strings.TrimPrefix(sound, "custom:")+".wav")
		f, err := os.OpenFile(path, os.O_RDONLY|syscall.O_NONBLOCK, 0)
		if err != nil {
			return nil, errors.New("Imported audio is missing; choose or import it again")
		}
		defer f.Close()
		info, err := f.Stat()
		if err != nil || !info.Mode().IsRegular() || info.Size() > MaxAudioBytes+44 {
			return nil, errors.New("Imported audio couldn't be read; import it again")
		}
		data, err := io.ReadAll(io.LimitReader(f, MaxAudioBytes+45))
		if err != nil || len(data) < 46 || len(data) > MaxAudioBytes+44 || (len(data)-44)%2 != 0 || string(data[:4]) != "RIFF" || string(data[8:16]) != "WAVEfmt " || string(data[36:40]) != "data" || binary.LittleEndian.Uint32(data[24:]) != AudioRate || binary.LittleEndian.Uint16(data[22:]) != 1 || binary.LittleEndian.Uint16(data[34:]) != 16 {
			return nil, errors.New("Imported audio couldn't be read; import it again")
		}
		if binary.LittleEndian.Uint16(data[20:]) != 1 || binary.LittleEndian.Uint16(data[32:]) != 2 || binary.LittleEndian.Uint32(data[40:]) != uint32(len(data)-44) {
			return nil, errors.New("Imported audio couldn't be read; import it again")
		}
		return data[44:], nil
	}
	if sound != "chime" && sound != "bells" && sound != "dawn" && sound != "pulse" {
		return nil, errors.New("Unknown alarm chime")
	}
	pcm := make([]byte, AudioRate*2)
	for i := 0; i < AudioRate; i++ {
		t := float64(i) / AudioRate
		local := math.Mod(t, 0.5)
		frequency := 660.0
		if t >= 0.5 {
			frequency = 880
		}
		switch sound {
		case "bells":
			frequency = 523.25
			if t >= 0.5 {
				frequency = 783.99
			}
		case "dawn":
			frequency = 392 + 196*t
		case "pulse":
			local = math.Mod(t, 0.25)
			frequency = 990
		}
		if local > 0.30 {
			continue
		}
		envelope := math.Min(1, local/0.02) * math.Min(1, (0.30-local)/0.08)
		wave := math.Sin(2 * math.Pi * frequency * t)
		if sound == "bells" {
			wave = (wave + 0.3*math.Sin(2*math.Pi*frequency*2.76*t)) / 1.3
		}
		binary.LittleEndian.PutUint16(pcm[i*2:], uint16(int16(9000*envelope*wave)))
	}
	return pcm, nil
}

// Gain applies per-alarm volume and a sample-smooth fade, without changing the
// session's output volume. It always returns an independent PCM buffer.
func Gain(pcm []byte, options AlarmOptions, elapsed int64) []byte {
	out := make([]byte, len(pcm))
	for i := 0; i+1 < len(pcm); i += 2 {
		volume := float64(options.Volume) / 100
		if options.FadeDuration > 0 {
			volume *= math.Min(1, math.Max(0, float64(elapsed)+float64(i/2)*1000/AudioRate)/float64(options.FadeDuration))
		}
		v := int16(binary.LittleEndian.Uint16(pcm[i:]))
		binary.LittleEndian.PutUint16(out[i:], uint16(int16(float64(v)*volume)))
	}
	return out
}

func WAV(pcm []byte) []byte {
	b := make([]byte, 44+len(pcm))
	copy(b, "RIFF")
	binary.LittleEndian.PutUint32(b[4:], uint32(len(b)-8))
	copy(b[8:], "WAVEfmt ")
	binary.LittleEndian.PutUint32(b[16:], 16)
	binary.LittleEndian.PutUint16(b[20:], 1)
	binary.LittleEndian.PutUint16(b[22:], 1)
	binary.LittleEndian.PutUint32(b[24:], AudioRate)
	binary.LittleEndian.PutUint32(b[28:], AudioRate*2)
	binary.LittleEndian.PutUint16(b[32:], 2)
	binary.LittleEndian.PutUint16(b[34:], 16)
	copy(b[36:], "data")
	binary.LittleEndian.PutUint32(b[40:], uint32(len(pcm)))
	copy(b[44:], pcm)
	return b
}
