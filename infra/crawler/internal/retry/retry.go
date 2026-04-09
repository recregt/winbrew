package retry

import (
	"context"
	"crypto/rand"
	"errors"
	"fmt"
	"math/big"
	"runtime/debug"
	"strings"
	"time"
)

const (
	maxBackoffDelay   = 30 * time.Second
	maxTrackedErrors  = 10
	maxPanicStackSize = 4 * 1024
)

type RetryError struct {
	Attempts int
	LastErr  error
	AllErrs  []error
}

func (e *RetryError) Error() string {
	if e == nil {
		return "<nil>"
	}

	var b strings.Builder
	fmt.Fprintf(&b, "failed after %d attempts: %v", e.Attempts, e.LastErr)

	if len(e.AllErrs) > 1 {
		b.WriteString("\nattempt history:")
		limit := len(e.AllErrs)
		if limit > 3 {
			limit = 3
		}
		for i := 0; i < limit; i++ {
			fmt.Fprintf(&b, "\n  [%d] %v", i+1, e.AllErrs[i])
		}
		if len(e.AllErrs) > limit {
			fmt.Fprintf(&b, "\n  ... and %d more", len(e.AllErrs)-limit)
		}
	}

	return b.String()
}

func (e *RetryError) Unwrap() []error {
	if e == nil {
		return nil
	}

	unwrapped := make([]error, 0, len(e.AllErrs)+1)
	if e.LastErr != nil {
		unwrapped = append(unwrapped, e.LastErr)
	}
	unwrapped = append(unwrapped, e.AllErrs...)
	return unwrapped
}

type NonRetryableError interface {
	error
	NonRetryable() bool
}

type Config struct {
	MaxAttempts int
	BaseDelay   time.Duration
	RandInt64N  func(n int64) (int64, error)
	OnRetry     func(attempt int, err error, nextDelay time.Duration)
	OnSuccess   func(attempt int, duration time.Duration)
}

func Do(ctx context.Context, maxAttempts int, baseDelay time.Duration, fn func() error) error {
	return DoConfig(ctx, Config{MaxAttempts: maxAttempts, BaseDelay: baseDelay}, fn)
}

func DoConfig(ctx context.Context, cfg Config, fn func() error) error {
	if cfg.MaxAttempts < 1 {
		cfg.MaxAttempts = 1
	}
	if cfg.BaseDelay <= 0 {
		return fmt.Errorf("retry: baseDelay must be positive, got %v", cfg.BaseDelay)
	}
	if err := ctx.Err(); err != nil {
		return err
	}

	start := time.Now()
	errs := make([]error, 0, minInt(cfg.MaxAttempts, maxTrackedErrors))

	for attempt := 1; attempt <= cfg.MaxAttempts; attempt++ {
		if err := ctx.Err(); err != nil {
			if len(errs) == 0 {
				return err
			}

			return &RetryError{
				Attempts: len(errs),
				LastErr:  err,
				AllErrs:  append([]error(nil), errs...),
			}
		}

		rawErr := safeFn(fn)
		if rawErr == nil {
			if cfg.OnSuccess != nil {
				cfg.OnSuccess(attempt, time.Since(start))
			}
			return nil
		}

		attemptErr := fmt.Errorf("attempt %d: %w", attempt, rawErr)
		if len(errs) < maxTrackedErrors {
			errs = append(errs, attemptErr)
		}

		if !IsRetryable(rawErr) || attempt == cfg.MaxAttempts {
			return &RetryError{
				Attempts: attempt,
				LastErr:  rawErr,
				AllErrs:  append([]error(nil), errs...),
			}
		}

		delay := calculateDelay(attempt, cfg.BaseDelay, cfg.RandInt64N)
		if deadline, ok := ctx.Deadline(); ok {
			remaining := time.Until(deadline)
			if remaining <= 0 || remaining < delay {
				deadlineErr := ctx.Err()
				if deadlineErr == nil {
					deadlineErr = context.DeadlineExceeded
				}

				return &RetryError{
					Attempts: attempt,
					LastErr:  fmt.Errorf("context deadline exceeded before next retry: %w", deadlineErr),
					AllErrs:  append([]error(nil), errs...),
				}
			}
		}

		if cfg.OnRetry != nil {
			cfg.OnRetry(attempt, rawErr, delay)
		}

		if err := sleepWithContext(ctx, delay); err != nil {
			return &RetryError{
				Attempts: attempt,
				LastErr:  err,
				AllErrs:  append([]error(nil), errs...),
			}
		}
	}

	return nil
}

func IsRetryable(err error) bool {
	if err == nil {
		return false
	}

	var nonRetryable NonRetryableError
	if errors.As(err, &nonRetryable) && nonRetryable.NonRetryable() {
		return false
	}

	if errors.Is(err, context.Canceled) || errors.Is(err, context.DeadlineExceeded) {
		return false
	}

	return true
}

func calculateDelay(attempt int, baseDelay time.Duration, randInt64N func(n int64) (int64, error)) time.Duration {
	if attempt < 1 || baseDelay <= 0 {
		return 0
	}

	shift := uint(attempt - 1)
	if shift > 62 {
		shift = 62
	}

	multiplier := time.Duration(1) << shift
	if baseDelay > 0 && multiplier > maxBackoffDelay/baseDelay {
		return withJitter(maxBackoffDelay, randInt64N)
	}

	delay := baseDelay * multiplier
	if delay > maxBackoffDelay {
		delay = maxBackoffDelay
	}

	return withJitter(delay, randInt64N)
}

func safeFn(fn func() error) (err error) {
	defer func() {
		if r := recover(); r != nil {
			stack := debug.Stack()
			if len(stack) > maxPanicStackSize {
				stack = stack[:maxPanicStackSize]
			}
			err = fmt.Errorf("panic recovered: %v\n%s", r, stack)
		}
	}()

	return fn()
}

func sleepWithContext(ctx context.Context, d time.Duration) error {
	if d <= 0 {
		return nil
	}

	timer := time.NewTimer(d)
	defer timer.Stop()

	select {
	case <-ctx.Done():
		if !timer.Stop() {
			select {
			case <-timer.C:
			default:
			}
		}
		return ctx.Err()
	case <-timer.C:
		return nil
	}
}

func withJitter(delay time.Duration, randInt64N func(n int64) (int64, error)) time.Duration {
	if delay <= 0 {
		return 0
	}

	half := delay / 2
	if randInt64N != nil {
		if n, err := randInt64N(int64(delay)); err == nil {
			return half + time.Duration(n)
		}
	}

	n, err := cryptoInt64N(int64(delay))
	if err != nil {
		return half
	}

	return half + time.Duration(n)
}

func cryptoInt64N(n int64) (int64, error) {
	if n <= 0 {
		return 0, nil
	}

	value, err := rand.Int(rand.Reader, big.NewInt(n))
	if err != nil {
		return 0, err
	}

	return value.Int64(), nil
}

func minInt(a, b int) int {
	if a < b {
		return a
	}
	return b
}
