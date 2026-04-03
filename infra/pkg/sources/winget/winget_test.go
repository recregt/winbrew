package winget

import (
	"context"
	"net/http"
	"net/http/httptest"
	"os"
	"path/filepath"
	"sync"
	"sync/atomic"
	"testing"
)

func TestDownloadUsesETagCache(t *testing.T) {
	t.Parallel()

	dir := t.TempDir()
	var calls int32
	var mu sync.Mutex
	var ifNoneMatch []string
	var unexpectedCalls int32
	const etagValue = `"abc123"`

	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		call := atomic.AddInt32(&calls, 1)

		mu.Lock()
		ifNoneMatch = append(ifNoneMatch, r.Header.Get("If-None-Match"))
		mu.Unlock()

		switch call {
		case 1:
			w.Header().Set("ETag", etagValue)
			w.WriteHeader(http.StatusOK)
			_, _ = w.Write([]byte("msix-bytes"))
		case 2:
			w.WriteHeader(http.StatusNotModified)
		default:
			atomic.StoreInt32(&unexpectedCalls, 1)
		}
	}))
	defer server.Close()

	src, err := New(server.Client(), dir)
	if err != nil {
		t.Fatalf("New() error = %v", err)
	}

	dst := filepath.Join(dir, "winget-source.msix")
	if err := src.download(context.Background(), server.URL, dst); err != nil {
		t.Fatalf("download(first) error = %v", err)
	}

	data, err := os.ReadFile(dst)
	if err != nil {
		t.Fatalf("ReadFile(dst) error = %v", err)
	}
	if got, want := string(data), "msix-bytes"; got != want {
		t.Fatalf("downloaded content = %q, want %q", got, want)
	}

	etagData, err := os.ReadFile(dst + ".etag")
	if err != nil {
		t.Fatalf("ReadFile(etag) error = %v", err)
	}
	if got, want := string(etagData), etagValue; got != want {
		t.Fatalf("etag file = %q, want %q", got, want)
	}

	if err := src.download(context.Background(), server.URL, dst); err != nil {
		t.Fatalf("download(second) error = %v", err)
	}

	data, err = os.ReadFile(dst)
	if err != nil {
		t.Fatalf("ReadFile(dst after 304) error = %v", err)
	}
	if got, want := string(data), "msix-bytes"; got != want {
		t.Fatalf("cached content = %q, want %q", got, want)
	}

	if atomic.LoadInt32(&unexpectedCalls) != 0 {
		t.Fatal("unexpected request count")
	}

	mu.Lock()
	gotHeaders := append([]string(nil), ifNoneMatch...)
	mu.Unlock()
	if len(gotHeaders) != 2 {
		t.Fatalf("request count = %d, want 2", len(gotHeaders))
	}
	if got, want := gotHeaders[0], ""; got != want {
		t.Fatalf("first request If-None-Match = %q, want %q", got, want)
	}
	if got, want := gotHeaders[1], etagValue; got != want {
		t.Fatalf("second request If-None-Match = %q, want %q", got, want)
	}
}
