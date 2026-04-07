package crawler

import (
	"bytes"
	"encoding/json"
	"strings"
	"testing"

	"winbrew/infra/pkg/normalize"
)

func TestEmitPackagesJSONL(t *testing.T) {
	t.Parallel()

	var out bytes.Buffer
	err := emitPackagesJSONL(&out, []normalize.Package{{
		ID:          "scoop/main/example",
		Name:        "example",
		Version:     "1.2.3",
		Description: "example package",
		Installers: []normalize.Installer{{
			URL:  "https://example.invalid/app.zip",
			Hash: "sha256:deadbeef",
			Type: "portable",
		}},
	}})
	if err != nil {
		t.Fatalf("emitPackagesJSONL() error = %v", err)
	}

	lines := strings.Split(strings.TrimSpace(out.String()), "\n")
	if got, want := len(lines), 1; got != want {
		t.Fatalf("line count = %d, want %d", got, want)
	}

	var decoded map[string]any
	if err := json.Unmarshal([]byte(lines[0]), &decoded); err != nil {
		t.Fatalf("json.Unmarshal() error = %v", err)
	}
	if got, ok := decoded["id"].(string); !ok || got != "scoop/main/example" {
		t.Fatalf("id = %#v, want %q", decoded["id"], "scoop/main/example")
	}
	installers, ok := decoded["installers"].([]any)
	if !ok || len(installers) != 1 {
		t.Fatalf("installers = %#v, want 1 item", decoded["installers"])
	}
}
