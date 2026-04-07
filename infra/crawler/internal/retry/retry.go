package retry

import (
	"context"
	"fmt"
	"math/rand/v2"
	"time"
)

const (
	maxBackoffDelay     = 30 * time.Second
	jitterWindowDivisor = 5 // +/-20% jitter window
)

func Do(ctx context.Context, maxAttempts int, baseDelay time.Duration, fn func() error) error {
	if maxAttempts < 1 {
		maxAttempts = 1
	}

	for attempt := 1; ; attempt++ {
		if err := ctx.Err(); err != nil {
			return err
		}

		if err := fn(); err != nil {
			if attempt == maxAttempts {
				return fmt.Errorf("attempt %d/%d failed: %w", attempt, maxAttempts, err)
			}

			shift := uint(attempt - 1)
			if shift > 62 {
				shift = 62
			}
			delay := min(baseDelay*(time.Duration(1)<<shift), maxBackoffDelay)
			delay = withJitter(delay)

			timer := time.NewTimer(delay)
			select {
			case <-ctx.Done():
				if !timer.Stop() {
					<-timer.C
				}
				return ctx.Err()
			case <-timer.C:
			}
			continue
		}

		return nil
	}
}

func withJitter(delay time.Duration) time.Duration {
	if delay <= 0 {
		return 0
	}

	jitterRange := delay / jitterWindowDivisor
	if jitterRange <= 0 {
		return delay
	}

	offset := time.Duration(rand.Int64N(int64(jitterRange)*2+1)) - jitterRange
	jittered := delay + offset
	if jittered < 0 {
		return 0
	}

	return jittered
}
