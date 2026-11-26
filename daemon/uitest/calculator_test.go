//go:build uitest

package uitest_test

import (
	"encoding/json"
	"image/png"
	"os"
	"path/filepath"
	"testing"

	"whiterose/daemon/internal/uitest"
)

func calculatorBinary(s *uitest.Session) string {
	if p := os.Getenv("WR_UITEST_CALCULATOR"); p != "" {
		return p
	}
	return filepath.Join(s.Repo, "apps/target/debug/whiterose-calculator")
}

func calculatorExpression(w *uitest.Window, value string) {
	w.Click(uitest.Named("Expression").Role("text"))
	w.Press("ctrl+a")
	w.Type(value)
}

func calculatorShot(t *testing.T, s *uitest.Session, w *uitest.Window, name string) {
	t.Helper()
	if out := os.Getenv("WHITEROSE_CALCULATOR_SHOTS"); out != "" {
		if err := os.MkdirAll(out, 0o755); err != nil {
			t.Fatal(err)
		}
		g := w.Geometry()
		image := s.Capture(g[0], g[1], g[2], g[3])
		f, err := os.Create(filepath.Join(out, name+".png"))
		if err != nil {
			t.Fatal(err)
		}
		defer f.Close()
		if err := png.Encode(f, image); err != nil {
			t.Fatal(err)
		}
	}
}

func TestCalculatorArithmeticScientificHistoryAndClipboard(t *testing.T) {
	s := uitest.Start(t, uitest.Options{})
	s.Launch("whiterose-calculator", calculatorBinary(s))
	app := s.WaitWindow("Calculator")
	result := uitest.Named("Result")
	expression := uitest.Named("Expression").Role("text")
	app.Gone(uitest.Named("Type an expression · Enter = · Esc clears"))
	calculatorExpression(app, "(125+75)*4")
	app.WaitFor(result.Described("800"))
	app.Press("Return")
	calculatorShot(t, s, app, "standard")
	app.Click(uitest.Named("Copy").Role("push button"))
	app.Click(expression)
	app.Press("ctrl+a")
	app.Press("ctrl+v")
	if got := app.Text(expression); got != "800" {
		t.Fatalf("copy/paste result = %q, want 800", got)
	}
	app.Click(uitest.Named("C").Role("push button"))
	for _, key := range []string{"1", "2", "+", "3", "Equals"} {
		app.Click(uitest.Named(key).Role("push button"))
	}
	app.WaitFor(result.Described("15"))
	app.Click(uitest.Named("7").Role("push button"))
	if got := app.Text(expression); got != "7" {
		t.Fatalf("a digit after Equals didn't start a new calculation: %q", got)
	}
	app.Click(uitest.Named("Scientific").Role("radio button"))
	calculatorExpression(app, "sin(30)+2^3")
	app.Press("Return")
	app.WaitFor(result.Described("8.5"))
	calculatorShot(t, s, app, "scientific")
	app.Click(uitest.Named("RAD").Role("radio button"))
	app.WaitFor(result.Described("7.01196837591"))
	app.Click(uitest.Named("DEG").Role("radio button"))
	app.WaitFor(result.Described("8.5"))

	calculatorExpression(app, "1/0")
	app.Press("Return")
	app.WaitFor(uitest.Named("Cannot divide by zero"))
	app.WaitFor(result.Described("Error"))
	app.Click(uitest.Named("History").Role("push button"))
	app.WaitFor(uitest.Named("sin(30)+2^3 = 8.5").Role("list item"))
	app.Click(uitest.Named("RAD").Role("radio button"))
	app.Click(uitest.Named("Reuse expression").Role("push button"))
	app.WaitFor(result.Described("8.5"))
	if !app.WaitFor(uitest.Named("DEG").Role("radio button")).Checked() {
		t.Error("history didn't restore its angle mode")
	}
	calculatorShot(t, s, app, "history")
	var state struct {
		History []struct {
			Expression string `json:"expression"`
		} `json:"history"`
	}
	data, err := os.ReadFile(filepath.Join(s.Home, ".local/share/whiterose/calculator/session.json"))
	if err != nil || json.Unmarshal(data, &state) != nil {
		t.Fatalf("history wasn't saved: %v (%s)", err, data)
	}
	if len(state.History) != 3 {
		t.Fatalf("history has %d calculations, want 3 (invalid input must not be saved)", len(state.History))
	}
	app.Click(uitest.Named("Close").Role("push button"))
	app.Closed()
	s.Launch("whiterose-calculator", calculatorBinary(s))
	app = s.WaitWindow("Calculator")
	app.WaitFor(uitest.Named("sin(30)+2^3 = 8.5").Role("list item"))
	app.Click(uitest.Named("Clear").Role("push button"))
	app.WaitFor(uitest.Named("No calculations yet"))
	app.Click(uitest.Named("Close").Role("push button"))
	app.Closed()
	s.Launch("whiterose-calculator", calculatorBinary(s))
	app = s.WaitWindow("Calculator")
	app.WaitFor(uitest.Named("No calculations yet"))
	if !app.WaitFor(uitest.Named("Scientific").Role("radio button")).Checked() {
		t.Error("scientific mode wasn't remembered")
	}
}

func TestCalculatorButtonsSelectionAndLightTheme(t *testing.T) {
	s := uitest.Start(t, uitest.Options{Settings: map[string]any{"appearance.theme": "light"}})
	s.Launch("whiterose-calculator", calculatorBinary(s))
	app := s.WaitWindow("Calculator")
	expression := uitest.Named("Expression").Role("text")
	result := uitest.Named("Result")
	calculatorExpression(app, "25")
	app.Click(uitest.Named("%").Role("push button"))
	app.WaitFor(result.Described("0.25"))
	app.Click(uitest.Named("C").Role("push button"))
	calculatorExpression(app, "12+3")
	app.Click(uitest.Named("x²").Role("push button"))
	app.WaitFor(result.Described("21"))
	app.Click(uitest.Named("CE").Role("push button"))
	if got := app.Text(expression); got != "12+" {
		t.Fatalf("CE cleared more than the current operand: %q", got)
	}
	app.Click(uitest.Named("4").Role("push button"))
	app.WaitFor(result.Described("16"))
	app.Click(expression)
	app.Press("ctrl+a")
	app.Click(uitest.Named("√x").Role("push button"))
	app.WaitFor(result.Described("4"))
	app.Press("Return")
	app.Type("9")
	app.WaitFor(result.Described("9"))
	if got := app.Text(expression); got != "9" {
		t.Fatalf("typing after Equals didn't start fresh: %q", got)
	}
	calculatorShot(t, s, app, "light")
}

func TestCalculatorNarrowHistoryAndShortWindow(t *testing.T) {
	s := uitest.Start(t, uitest.Options{})
	s.Launch("whiterose-calculator", calculatorBinary(s))
	app := s.WaitWindow("Calculator")
	app.Click(uitest.Named("Scientific").Role("radio button"))
	calculatorExpression(app, "6*7")
	app.Press("Return")
	app.Click(uitest.Named("History").Role("push button"))
	g := app.Geometry()
	x, y := float64(g[0]+g[2]-4), float64(g[1]+g[3]-4)
	s.Drag(x, y, x-410, y-240)
	if !uitest.WaitUntil(func() bool { now := app.Geometry(); return now[2] < 700 && now[3] < 500 }) {
		t.Fatalf("Calculator didn't shrink: %v", app.Geometry())
	}
	app.WaitFor(uitest.Named("Back").Role("push button"))
	app.Click(uitest.Named("Reuse expression").Role("push button"))
	app.WaitFor(uitest.Named("Result").Described("42"))
	app.Gone(uitest.Named("Back").Role("push button"))
	// Closing the history overlay restores the preferred scientific size.
	// Shrink again and use the last keypad row through the scroll container.
	g = app.Geometry()
	x, y = float64(g[0]+g[2]-4), float64(g[1]+g[3]-4)
	s.Drag(x, y, x-140, y-240)
	app.Click(uitest.Named("Equals").Role("push button"))
	app.Click(uitest.Named("Copy").Role("push button"))
	app.WaitFor(uitest.Named("Result").Described("42"))
	app.WaitFor(uitest.Named("Copied").Role("push button"))
	calculatorShot(t, s, app, "short")
}

func TestCalculatorDisplayLongNumbersAndExponents(t *testing.T) {
	s := uitest.Start(t, uitest.Options{})
	s.Launch("whiterose-calculator", calculatorBinary(s))
	app := s.WaitWindow("Calculator")
	expression := uitest.Named("Expression").Role("text")
	result := uitest.Named("Result")
	for _, example := range []struct{ input, result string }{
		{"12500-154.33", "12345.67"},
		{"-1/1000", "-0.001"},
		{"6.02*10^23", "6.02e+23"},
		{"1/7", "0.142857142857"},
	} {
		calculatorExpression(app, example.input)
		app.Press("Return")
		app.WaitFor(result.Described(example.result))
		app.Click(uitest.Named("Copy").Role("push button"))
		app.Click(expression)
		app.Press("ctrl+a")
		app.Press("ctrl+v")
		if got := app.Text(expression); got != example.result {
			t.Fatalf("%s copied as %q, want %q", example.input, got, example.result)
		}
	}
	calculatorExpression(app, "12500-154.33")
	app.Press("Return")
	app.WaitFor(result.Described("12345.67"))
	calculatorShot(t, s, app, "doto-decimal")
	calculatorExpression(app, "6.02*10^23")
	app.Press("Return")
	app.WaitFor(result.Described("6.02e+23"))
	calculatorShot(t, s, app, "doto-exponent")
}
