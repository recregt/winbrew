package scoop

import (
	"strings"
	"testing"
)

func TestTruncateGitOutput(t *testing.T) {
	t.Parallel()

	output := strings.Repeat("a", 5000)
	truncated := truncateGitOutput(output)

	if len(truncated) > 4099 {
		t.Fatalf("len(truncated) = %d, want <= 4099", len(truncated))
	}
	if !strings.HasPrefix(truncated, "...") {
		t.Fatalf("truncated output does not have expected prefix: %q", truncated[:3])
	}
	if !strings.HasSuffix(truncated, strings.Repeat("a", 10)) {
		t.Fatalf("truncated output does not preserve tail: %q", truncated)
	}
}

func TestIsRetryableGitOutput(t *testing.T) {
	t.Parallel()

	tests := []struct {
		name   string
		output string
		want   bool
	}{
		{name: "network", output: "fatal: unable to access 'https://example.invalid': Could not resolve host: example.invalid", want: true},
		{name: "remote hang up", output: "error: RPC failed; the remote end hung up unexpectedly", want: true},
		{name: "auth", output: "fatal: Authentication failed for 'https://example.invalid/'", want: false},
		{name: "empty", output: "", want: true},
	}

	for _, tt := range tests {
		tt := tt
		t.Run(tt.name, func(t *testing.T) {
			t.Parallel()

			if got := isRetryableGitOutput(tt.output); got != tt.want {
				t.Fatalf("isRetryableGitOutput() = %v, want %v", got, tt.want)
			}
		})
	}
}
