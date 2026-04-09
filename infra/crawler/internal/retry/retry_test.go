package retry

import (
	"context"
	"errors"
	"fmt"
	"strings"
	"testing"
	"time"
)

func TestDoReturnsOnFirstSuccess(t *testing.T) {
	t.Parallel()

	var attempts int
	err := Do(context.Background(), 3, time.Millisecond, func() error {
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
	err := Do(context.Background(), 3, time.Millisecond, func() error {
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
	err := Do(context.Background(), 2, time.Millisecond, func() error {
		return wantErr
	})
	if err == nil {
		t.Fatal("Do() error = nil, want non-nil")
	}
	if !errors.Is(err, wantErr) {
		t.Fatalf("Do() error = %v, want wrapped %v", err, wantErr)
	}
	var retryErr *RetryError
	if !errors.As(err, &retryErr) {
		t.Fatalf("Do() error = %T, want RetryError", err)
	}
	if got, want := retryErr.Attempts, 2; got != want {
		t.Fatalf("RetryError.Attempts = %d, want %d", got, want)
	}
	if got, want := len(retryErr.AllErrs), 2; got != want {
		t.Fatalf("RetryError.AllErrs len = %d, want %d", got, want)
	}
	if !strings.Contains(err.Error(), "failed after 2 attempts") {
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

func TestDoSkipsNonRetryableErrors(t *testing.T) {
	t.Parallel()

	var attempts int
	err := Do(context.Background(), 3, 10*time.Millisecond, func() error {
		attempts++
		return nonRetryableTestErr{msg: "bad request"}
	})
	if err == nil {
		t.Fatal("Do() error = nil, want non-nil")
	}
	if attempts != 1 {
		t.Fatalf("Do() attempts = %d, want 1", attempts)
	}
	var retryErr *RetryError
	if !errors.As(err, &retryErr) {
		t.Fatalf("Do() error = %T, want RetryError", err)
	}
	if got, want := retryErr.Attempts, 1; got != want {
		t.Fatalf("RetryError.Attempts = %d, want %d", got, want)
	}
	if !strings.Contains(retryErr.Error(), "failed after 1 attempts") {
		t.Fatalf("RetryError.Error() = %q, want attempt count", retryErr.Error())
	}
}

func TestDoRejectsNonPositiveBaseDelay(t *testing.T) {
	t.Parallel()

	err := Do(context.Background(), 1, 0, func() error {
		return errors.New("fail")
	})
	if err == nil {
		t.Fatal("Do() error = nil, want non-nil")
	}
	if !strings.Contains(err.Error(), "baseDelay must be positive") {
		t.Fatalf("Do() error = %q, want baseDelay validation", err.Error())
	}
}

func TestDoRecoversFromPanic(t *testing.T) {
	t.Parallel()

	var attempts int
	err := Do(context.Background(), 2, time.Millisecond, func() error {
		attempts++
		if attempts == 1 {
			panic("boom")
		}
		return nil
	})
	if err != nil {
		t.Fatalf("Do() error = %v, want nil", err)
	}
	if attempts != 2 {
		t.Fatalf("Do() attempts = %d, want 2", attempts)
	}
}

func TestDoConfigHooks(t *testing.T) {
	t.Parallel()

	var retryCalls int
	var successCalls int
	var observedDelay time.Duration
	var observedDuration time.Duration

	err := DoConfig(context.Background(), Config{
		MaxAttempts: 2,
		BaseDelay:   time.Millisecond,
		OnRetry: func(attempt int, err error, nextDelay time.Duration) {
			retryCalls++
			if attempt != 1 {
				t.Fatalf("OnRetry attempt = %d, want 1", attempt)
			}
			if err == nil {
				t.Fatal("OnRetry err = nil, want non-nil")
			}
			observedDelay = nextDelay
		},
		OnSuccess: func(attempt int, duration time.Duration) {
			successCalls++
			if attempt != 2 {
				t.Fatalf("OnSuccess attempt = %d, want 2", attempt)
			}
			observedDuration = duration
		},
	}, func() error {
		if retryCalls == 0 {
			return fmt.Errorf("retry me")
		}
		return nil
	})
	if err != nil {
		t.Fatalf("DoConfig() error = %v, want nil", err)
	}
	if retryCalls != 1 {
		t.Fatalf("OnRetry calls = %d, want 1", retryCalls)
	}
	if successCalls != 1 {
		t.Fatalf("OnSuccess calls = %d, want 1", successCalls)
	}
	if observedDelay <= 0 {
		t.Fatalf("OnRetry delay = %v, want positive", observedDelay)
	}
	if observedDuration <= 0 {
		t.Fatalf("OnSuccess duration = %v, want positive", observedDuration)
	}
}

func TestDoContextAlreadyCancelled(t *testing.T) {
	t.Parallel()

	ctx, cancel := context.WithCancel(context.Background())
	cancel()

	var attempts int
	err := Do(ctx, 3, time.Millisecond, func() error {
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
	err := Do(context.Background(), 0, time.Millisecond, func() error {
		attempts++
		return errors.New("fail")
	})
	if err == nil {
		t.Fatal("Do() error = nil, want non-nil")
	}
	if attempts != 1 {
		t.Fatalf("Do() attempts = %d, want 1", attempts)
	}
	if !strings.Contains(err.Error(), "failed after 1 attempts") {
		t.Fatalf("Do() error = %q, want attempt count in message", err.Error())
	}
}

func TestCalculateDelayUsesInjectedJitterFunc(t *testing.T) {
	t.Parallel()

	randFunc := func(n int64) (int64, error) {
		wantMax := int64(100 * time.Millisecond)
		if n != wantMax {
			t.Fatalf("randFunc max = %d, want %d", n, wantMax)
		}

		return int64(25 * time.Millisecond), nil
	}

	got1 := calculateDelay(1, 100*time.Millisecond, randFunc)
	got2 := calculateDelay(1, 100*time.Millisecond, randFunc)
	if got1 != got2 {
		t.Fatalf("calculateDelay() = %v and %v, want deterministic output", got1, got2)
	}
	if got1 != 75*time.Millisecond {
		t.Fatalf("calculateDelay() = %v, want 75ms", got1)
	}
}

func TestRetryErrorUnwrapsAllErrors(t *testing.T) {
	t.Parallel()

	wantErr := errors.New("boom")
	retryErr := &RetryError{
		Attempts: 2,
		LastErr:  wantErr,
		AllErrs: []error{
			fmt.Errorf("attempt 1: %w", errors.New("first")),
			fmt.Errorf("attempt 2: %w", errors.New("second")),
		},
	}

	if !errors.Is(retryErr, wantErr) {
		t.Fatalf("errors.Is(RetryError, wantErr) = false, want true")
	}
	if !strings.Contains(retryErr.Error(), "attempt history:") {
		t.Fatalf("RetryError.Error() = %q, want attempt history", retryErr.Error())
	}
}

type nonRetryableTestErr struct {
	msg string
}

func (e nonRetryableTestErr) Error() string {
	return e.msg
}

func (e nonRetryableTestErr) NonRetryable() bool {
	return true
}
