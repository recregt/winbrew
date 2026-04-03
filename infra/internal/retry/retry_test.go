package retry

import (
	"context"
	"errors"
	"strings"
	"testing"
	"time"
)

func TestDoReturnsOnFirstSuccess(t *testing.T) {
	t.Parallel()

	var attempts int
	err := Do(context.Background(), 3, 0, func() error {
		attempts++
		return nil
	})
	if err != nil {
		t.Fatalf("Do() error = %v, want nil", err)
	}
	if attempts != 1 {
		t.Fatalf("Do() attempts = %d, want 1", attempts)
	}
}

func TestDoRetriesUntilSuccess(t *testing.T) {
	t.Parallel()

	var attempts int
	err := Do(context.Background(), 3, 0, func() error {
		attempts++
		if attempts < 3 {
			return errors.New("try again")
		}
		return nil
	})
	if err != nil {
		t.Fatalf("Do() error = %v, want nil", err)
	}
	if attempts != 3 {
		t.Fatalf("Do() attempts = %d, want 3", attempts)
	}
}

func TestDoWrapsFinalError(t *testing.T) {
	t.Parallel()

	wantErr := errors.New("boom")
	err := Do(context.Background(), 2, 0, func() error {
		return wantErr
	})
	if err == nil {
		t.Fatal("Do() error = nil, want non-nil")
	}
	if !errors.Is(err, wantErr) {
		t.Fatalf("Do() error = %v, want wrapped %v", err, wantErr)
	}
	if !strings.Contains(err.Error(), "attempt 2/2 failed") {
		t.Fatalf("Do() error = %q, want attempt count in message", err.Error())
	}
}

func TestDoStopsWhenContextIsCancelled(t *testing.T) {
	t.Parallel()

	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()

	var attempts int
	err := Do(ctx, 3, 10*time.Millisecond, func() error {
		attempts++
		cancel()
		return errors.New("stop")
	})
	if !errors.Is(err, context.Canceled) {
		t.Fatalf("Do() error = %v, want context.Canceled", err)
	}
	if attempts != 1 {
		t.Fatalf("Do() attempts = %d, want 1", attempts)
	}
}

func TestDoRespectsBackoff(t *testing.T) {
	t.Parallel()

	start := time.Now()
	err := Do(context.Background(), 2, 25*time.Millisecond, func() error {
		return errors.New("fail")
	})
	if err == nil {
		t.Fatal("Do() error = nil, want non-nil")
	}

	elapsed := time.Since(start)
	if elapsed < 10*time.Millisecond {
		t.Fatalf("Do() elapsed = %v, want backoff delay to be observed", elapsed)
	}
}

func TestDoContextAlreadyCancelled(t *testing.T) {
	t.Parallel()

	ctx, cancel := context.WithCancel(context.Background())
	cancel()

	var attempts int
	err := Do(ctx, 3, 0, func() error {
		attempts++
		return errors.New("fail")
	})
	if !errors.Is(err, context.Canceled) {
		t.Fatalf("Do() error = %v, want context.Canceled", err)
	}
	if attempts != 0 {
		t.Fatalf("Do() attempts = %d, want 0", attempts)
	}
}

func TestDoTreatsNonPositiveMaxAttemptsAsOne(t *testing.T) {
	t.Parallel()

	var attempts int
	err := Do(context.Background(), 0, 0, func() error {
		attempts++
		return errors.New("fail")
	})
	if err == nil {
		t.Fatal("Do() error = nil, want non-nil")
	}
	if attempts != 1 {
		t.Fatalf("Do() attempts = %d, want 1", attempts)
	}
	if !strings.Contains(err.Error(), "attempt 1/1 failed") {
		t.Fatalf("Do() error = %q, want attempt count in message", err.Error())
	}
}
