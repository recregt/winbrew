package models

import (
	"testing"

	"gopkg.in/yaml.v3"
)

func TestFlexibleStringSliceScalar(t *testing.T) {
	t.Parallel()

	var values FlexibleStringSlice
	if err := yaml.Unmarshal([]byte(`" Windows.Desktop "`), &values); err != nil {
		t.Fatalf("yaml.Unmarshal() error = %v", err)
	}

	if got, want := len(values), 1; got != want {
		t.Fatalf("len(values) = %d, want %d", got, want)
	}
	if got, want := values[0], "Windows.Desktop"; got != want {
		t.Fatalf("values[0] = %q, want %q", got, want)
	}
}

func TestFlexibleStringSliceSequence(t *testing.T) {
	t.Parallel()

	var values FlexibleStringSlice
	if err := yaml.Unmarshal([]byte("- \" Windows.Desktop \"\n- Windows.Server\n"), &values); err != nil {
		t.Fatalf("yaml.Unmarshal() error = %v", err)
	}

	if got, want := len(values), 2; got != want {
		t.Fatalf("len(values) = %d, want %d", got, want)
	}
	if got, want := values[0], "Windows.Desktop"; got != want {
		t.Fatalf("values[0] = %q, want %q", got, want)
	}
	if got, want := values[1], "Windows.Server"; got != want {
		t.Fatalf("values[1] = %q, want %q", got, want)
	}
}
