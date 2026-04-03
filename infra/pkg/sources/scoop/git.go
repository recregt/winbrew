package scoop

import (
	"context"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
)

func syncRepo(ctx context.Context, url, dir string) error {
	_, err := os.Stat(filepath.Join(dir, ".git"))
	if os.IsNotExist(err) {
		return cloneRepo(ctx, url, dir)
	}
	if err != nil {
		return fmt.Errorf("failed to stat repo directory: %w", err)
	}
	return fetchRepo(ctx, dir)
}

func cloneRepo(ctx context.Context, url, dir string) error {
	cmd := exec.CommandContext(ctx, "git", "clone", "--depth=1", url, dir)
	out, err := cmd.CombinedOutput()
	if err != nil {
		return fmt.Errorf("git clone failed: %w\n%s", err, out)
	}
	return nil
}

func fetchRepo(ctx context.Context, dir string) error {
	cmd := exec.CommandContext(ctx, "git", "-C", dir, "fetch", "--depth=1", "origin", "HEAD")
	out, err := cmd.CombinedOutput()
	if err != nil {
		return fmt.Errorf("git fetch failed: %w\n%s", err, out)
	}

	cmd = exec.CommandContext(ctx, "git", "-C", dir, "reset", "--hard", "FETCH_HEAD")
	out, err = cmd.CombinedOutput()
	if err != nil {
		return fmt.Errorf("git reset failed: %w\n%s", err, out)
	}

	return nil
}
