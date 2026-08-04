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

func TestValidateRepoInputs(t *testing.T) {
	t.Parallel()

	tests := []struct {
		name    string
		url     string
		dir     string
		wantErr bool
	}{
		{name: "https", url: "https://github.com/ScoopInstaller/Main", dir: "/tmp/main", wantErr: false},
		{name: "http", url: "http://github.com/ScoopInstaller/Main", dir: "/tmp/main", wantErr: false},
		{name: "ssh shorthand", url: "git@github.com:ScoopInstaller/Main.git", dir: "/tmp/main", wantErr: false},
		{name: "empty url", url: "", dir: "/tmp/main", wantErr: true},
		{name: "empty dir", url: "https://github.com/ScoopInstaller/Main", dir: "", wantErr: true},
		{name: "ext transport", url: "ext::sh -c touch% /tmp/pwned", dir: "/tmp/main", wantErr: true},
		{name: "scheme confusion", url: "httpevil://github.com/ScoopInstaller/Main", dir: "/tmp/main", wantErr: true},
		{name: "schemeless http-prefixed", url: "http:evil", dir: "/tmp/main", wantErr: true},
		{name: "no host", url: "https:///ScoopInstaller/Main", dir: "/tmp/main", wantErr: true},
	}

	for _, tt := range tests {
		tt := tt
		t.Run(tt.name, func(t *testing.T) {
			t.Parallel()

			err := validateRepoInputs(tt.url, tt.dir)
			if (err != nil) != tt.wantErr {
				t.Fatalf("validateRepoInputs(%q, %q) error = %v, wantErr %v", tt.url, tt.dir, err, tt.wantErr)
			}
		})
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
