package scoop

import (
	"context"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
)

func syncRepo(ctx context.Context, url, dir string) error {
	if _, err := os.Stat(filepath.Join(dir, ".git")); os.IsNotExist(err) {
		return cloneRepo(ctx, url, dir)
	}
	return fetchRepo(ctx, dir)
}

func cloneRepo(ctx context.Context, url, dir string) error {
	cmd := exec.CommandContext(ctx, "git", "clone", "--depth=1", url, dir)
	if err := cmd.Run(); err != nil {
		return fmt.Errorf("git clone failed: %w", err)
	}
	return nil
}

func fetchRepo(ctx context.Context, dir string) error {
	cmd := exec.CommandContext(ctx, "git", "-C", dir, "fetch", "--depth=1", "origin", "HEAD")
	if err := cmd.Run(); err != nil {
		return fmt.Errorf("git fetch failed: %w", err)
	}

	cmd = exec.CommandContext(ctx, "git", "-C", dir, "reset", "--hard", "FETCH_HEAD")
	if err := cmd.Run(); err != nil {
		return fmt.Errorf("git reset failed: %w", err)
	}

	return nil
}
