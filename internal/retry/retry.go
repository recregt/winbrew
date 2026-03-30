package retry

import (
	"context"
	"fmt"
	"math/rand/v2"
	"time"
)

const jitterDivisor = 5

func Do(ctx context.Context, maxAttempts int, baseDelay time.Duration, fn func() error) error {
	if maxAttempts < 1 {
		maxAttempts = 1
	}

	var lastErr error

	for attempt := 1; attempt <= maxAttempts; attempt++ {
		if err := ctx.Err(); err != nil {
			return err
		}

		if err := fn(); err != nil {
			lastErr = err
			if attempt == maxAttempts {
				return fmt.Errorf("attempt %d/%d failed: %w", attempt, maxAttempts, err)
			}

			delay := baseDelay
			for i := 1; i < attempt; i++ {
				delay *= 2
			}
			delay = withJitter(delay)
			if delay <= 0 {
				continue
			}

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

	return lastErr
}

func withJitter(delay time.Duration) time.Duration {
	if delay <= 0 {
		return 0
	}

	jitterRange := delay / jitterDivisor
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
