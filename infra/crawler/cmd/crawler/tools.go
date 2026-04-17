package main

import (
	"context"
	"errors"
	"flag"
	"fmt"
	"os"
	"strings"

	"infra/crawler/pkg/sources/winget"
)

func runTools(ctx context.Context, args []string) error {
	if len(args) == 0 {
		return fmt.Errorf("missing tools subcommand")
	}

	switch args[0] {
	case "generate-fixtures":
		return runGenerateFixtures(ctx, args[1:])
	default:
		return fmt.Errorf("unknown tools subcommand %q", args[0])
	}
}

func runGenerateFixtures(ctx context.Context, args []string) error {
	flags := flag.NewFlagSet("generate-fixtures", flag.ContinueOnError)
	flags.SetOutput(os.Stderr)

	count := flags.Int("count", 500, "number of Winget packages to write")
	outputPath := flags.String("output", "", "path to write the JSONL fixture")

	if err := flags.Parse(args); err != nil {
		if errors.Is(err, flag.ErrHelp) {
			return nil
		}
		return err
	}

	if *outputPath == "" {
		return fmt.Errorf("output path is required")
	}
	if *count <= 0 {
		return fmt.Errorf("count must be greater than zero")
	}
	if flags.NArg() > 0 {
		return fmt.Errorf("unexpected arguments: %s", strings.Join(flags.Args(), " "))
	}

	if err := winget.GenerateFixtures(ctx, *outputPath, *count); err != nil {
		return fmt.Errorf("generate fixtures: %w", err)
	}

	return nil
}
