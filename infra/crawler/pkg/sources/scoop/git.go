package scoop

import (
	"bytes"
	"context"
	"errors"
	"fmt"
	"io"
	"log/slog"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"time"
)

const repoSyncTimeout = 5 * time.Minute

func syncRepo(ctx context.Context, url, dir string) error {
	if err := validateRepoInputs(url, dir); err != nil {
		return err
	}

	ctx, cancel := context.WithTimeout(ctx, repoSyncTimeout)
	defer cancel()

	start := time.Now()
	lock, err := acquireRepoLock(ctx, dir)
	if err != nil {
		return err
	}
	defer func() {
		if releaseErr := lock.Release(); releaseErr != nil {
			slog.Warn("failed to release repo lock", "dir", dir, "err", releaseErr)
		}
	}()

	slog.Debug("syncing scoop repo", "url", url, "dir", dir)

	_, statErr := os.Stat(filepath.Join(dir, ".git"))
	if os.IsNotExist(statErr) {
		slog.Info("cloning scoop repo", "url", url, "dir", dir)
		err = cloneRepo(ctx, url, dir)
	} else if statErr != nil {
		return fmt.Errorf("failed to stat repo directory: %w", statErr)
	} else {
		slog.Debug("fetching scoop repo", "dir", dir)
		err = fetchRepo(ctx, dir)
	}
	if err != nil {
		slog.Error("scoop repo sync failed", "url", url, "dir", dir, "elapsed", time.Since(start), "err", err)
		return err
	}

	slog.Info("scoop repo synced", "url", url, "dir", dir, "elapsed", time.Since(start))
	return nil
}

func cloneRepo(ctx context.Context, url, dir string) error {
	tmpDir, err := os.MkdirTemp(filepath.Dir(dir), filepath.Base(dir)+".tmp-*")
	if err != nil {
		return fmt.Errorf("failed to create temporary clone directory: %w", err)
	}
	defer func() {
		_ = os.RemoveAll(tmpDir)
	}()

	if err := runGit(ctx, "git clone failed", "clone", "--depth=1", url, tmpDir); err != nil {
		return err
	}

	if err := os.RemoveAll(dir); err != nil {
		return fmt.Errorf("failed to remove existing repo directory: %w", err)
	}
	if err := os.Rename(tmpDir, dir); err != nil {
		return fmt.Errorf("failed to move cloned repo into place: %w", err)
	}

	return nil
}

func fetchRepo(ctx context.Context, dir string) error {
	if err := runGit(ctx, "git remote validation failed", "-C", dir, "remote", "get-url", "origin"); err != nil {
		return err
	}

	if err := runGit(ctx, "git fetch failed", "-C", dir, "fetch", "--depth=1", "origin", "HEAD"); err != nil {
		return err
	}

	if err := runGit(ctx, "git reset failed", "-C", dir, "reset", "--hard", "FETCH_HEAD"); err != nil {
		return err
	}

	return nil
}

func runGit(ctx context.Context, errorPrefix string, args ...string) error {
	cmd := exec.CommandContext(ctx, "git", args...)
	var stderr bytes.Buffer
	cmd.Stdout = io.Discard
	cmd.Stderr = &stderr

	if err := cmd.Run(); err != nil {
		output := truncateGitOutput(stderr.String())
		return &gitCommandError{
			prefix:    errorPrefix,
			err:       err,
			output:    output,
			retryable: isRetryableGitOutput(output),
		}
	}

	return nil
}

type gitCommandError struct {
	prefix    string
	err       error
	output    string
	retryable bool
}

func (e *gitCommandError) Error() string {
	if e == nil {
		return "<nil>"
	}

	if e.output == "" {
		return fmt.Sprintf("%s: %v", e.prefix, e.err)
	}

	return fmt.Sprintf("%s: %v\n%s", e.prefix, e.err, e.output)
}

func (e *gitCommandError) Unwrap() error {
	if e == nil {
		return nil
	}

	return e.err
}

func (e *gitCommandError) NonRetryable() bool {
	if e == nil {
		return true
	}

	return !e.retryable
}

func validateRepoInputs(url, dir string) error {
	if strings.TrimSpace(url) == "" {
		return fmt.Errorf("empty repository URL")
	}
	if strings.TrimSpace(dir) == "" {
		return fmt.Errorf("empty repository directory")
	}
	if !strings.HasPrefix(url, "http") && !strings.HasPrefix(url, "git@") {
		return fmt.Errorf("invalid git URL: %s", url)
	}

	return nil
}

func acquireRepoLock(ctx context.Context, dir string) (*repoLock, error) {
	lockPath := dir + ".lock"
	const staleAfter = 30 * time.Minute
	for {
		file, err := os.OpenFile(lockPath, os.O_CREATE|os.O_EXCL|os.O_WRONLY, 0o600)
		if err == nil {
			return &repoLock{path: lockPath, file: file}, nil
		}
		if !errors.Is(err, os.ErrExist) {
			return nil, fmt.Errorf("failed to acquire repo lock: %w", err)
		}

		info, statErr := os.Stat(lockPath)
		if statErr == nil && time.Since(info.ModTime()) > staleAfter {
			removeErr := os.Remove(lockPath)
			if removeErr == nil || errors.Is(removeErr, os.ErrNotExist) {
				continue
			}
			return nil, fmt.Errorf("failed to remove stale repo lock: %w", removeErr)
		}

		timer := time.NewTimer(100 * time.Millisecond)
		select {
		case <-ctx.Done():
			timer.Stop()
			return nil, ctx.Err()
		case <-timer.C:
		}
	}
}

type repoLock struct {
	path string
	file *os.File
}

func (l *repoLock) Release() error {
	if l == nil {
		return nil
	}
	if l.file != nil {
		if err := l.file.Close(); err != nil {
			return err
		}
	}
	if err := os.Remove(l.path); err != nil && !errors.Is(err, os.ErrNotExist) {
		return err
	}
	return nil
}

func truncateGitOutput(output string) string {
	const maxOutput = 4 * 1024
	if len(output) <= maxOutput {
		return output
	}

	return "..." + output[len(output)-maxOutput:]
}

func isRetryableGitOutput(output string) bool {
	if output == "" {
		return true
	}

	retryablePatterns := []string{
		"Could not resolve host",
		"Connection refused",
		"Connection timed out",
		"Temporary failure",
		"RPC failed",
		"the remote end hung up unexpectedly",
	}

	for _, pattern := range retryablePatterns {
		if strings.Contains(output, pattern) {
			return true
		}
	}

	return false
}
