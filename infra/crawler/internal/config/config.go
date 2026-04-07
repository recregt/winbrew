package config

import (
	"bytes"
	"context"
	"errors"
	"fmt"
	"io"
	"os"
	"regexp"
	"strings"
	"time"

	"gopkg.in/yaml.v3"
)

var (
	envVarPattern = regexp.MustCompile(`\$\{([^}]+)\}`)
	validSources  = map[string]struct{}{
		"scoop":  {},
		"winget": {},
	}
)

const (
	defaultLogLevel     = "info"
	defaultFetchTimeout = 5 * time.Minute
	defaultRetryMax     = 3
	defaultRetryBackoff = 2 * time.Second
)

type Config struct {
	Sources  []string    `yaml:"sources"`
	LogLevel string      `yaml:"logLevel"` // debug, info, warn, error
	Timeout  Timeout     `yaml:"timeout"`
	Retry    RetryConfig `yaml:"retry"`
}

type Timeout struct {
	Fetch time.Duration `yaml:"fetch"` // source fetching
}

type RetryConfig struct {
	Max     int           `yaml:"max"`
	Backoff time.Duration `yaml:"backoff"`
}

func Load(path string) (*Config, error) {
	return LoadContext(context.Background(), path)
}

// LoadContext loads and validates a config file with cancellation support.
func LoadContext(ctx context.Context, path string) (*Config, error) {
	if err := ctx.Err(); err != nil {
		return nil, err
	}

	f, err := os.Open(path)
	if err != nil {
		return nil, fmt.Errorf("failed to open config file: %w", err)
	}
	defer func() {
		_ = f.Close()
	}()

	if err := ctx.Err(); err != nil {
		return nil, err
	}

	return Parse(&contextReader{ctx: ctx, reader: f})
}

// NewDefaultConfig constructs a config populated with the package defaults.
func NewDefaultConfig() *Config {
	return &Config{
		LogLevel: defaultLogLevel,
		Timeout: Timeout{
			Fetch: defaultFetchTimeout,
		},
		Retry: RetryConfig{
			Max:     defaultRetryMax,
			Backoff: defaultRetryBackoff,
		},
	}
}

// Parse decodes the configuration from any reader.
func Parse(r io.Reader) (*Config, error) {
	data, err := io.ReadAll(r)
	if err != nil {
		return nil, fmt.Errorf("failed to read config: %w", err)
	}

	data = expandEnv(data)

	cfg := NewDefaultConfig()
	dec := yaml.NewDecoder(bytes.NewReader(data))
	dec.KnownFields(true)
	if err := dec.Decode(cfg); err != nil {
		return nil, fmt.Errorf("failed to parse config: %w", err)
	}

	cfg.normalize()

	if err := cfg.Validate(); err != nil {
		return nil, err
	}

	return cfg, nil
}

func (c *Config) Validate() error {
	c.normalize()
	return c.validate()
}

func (c *Config) IsDebug() bool {
	return c.LogLevel == "debug"
}

func (c *Config) GetSources() []string {
	return append([]string(nil), c.Sources...)
}

func (c *Config) validate() error {
	var errs []error

	if len(c.Sources) == 0 {
		errs = append(errs, fmt.Errorf("at least one source must be configured"))
	}

	seen := make(map[string]struct{}, len(c.Sources))
	for i, source := range c.Sources {
		if source == "" {
			errs = append(errs, fmt.Errorf("sources[%d]: empty source name", i))
			continue
		}

		if _, ok := seen[source]; ok {
			errs = append(errs, fmt.Errorf("duplicate source: %s", source))
		} else {
			seen[source] = struct{}{}
		}

		if _, ok := validSources[source]; !ok {
			errs = append(errs, fmt.Errorf("unknown source: %s (valid: scoop, winget)", source))
		}
	}

	if !isValidLogLevel(c.LogLevel) {
		errs = append(errs, fmt.Errorf("invalid log level %q", c.LogLevel))
	}
	if c.Timeout.Fetch < 0 {
		errs = append(errs, fmt.Errorf("timeout.fetch cannot be negative: %v", c.Timeout.Fetch))
	}
	if c.Retry.Max < 0 {
		errs = append(errs, fmt.Errorf("retry.max cannot be negative: %d", c.Retry.Max))
	}
	if c.Retry.Backoff <= 0 {
		errs = append(errs, fmt.Errorf("retry.backoff must be positive: %v", c.Retry.Backoff))
	}

	if len(errs) == 0 {
		return nil
	}

	return fmt.Errorf("config validation failed with %d error(s): %w", len(errs), errors.Join(errs...))
}

func (c *Config) normalize() {
	for i, source := range c.Sources {
		c.Sources[i] = strings.ToLower(strings.TrimSpace(source))
	}
	c.LogLevel = strings.ToLower(strings.TrimSpace(c.LogLevel))
}

func isValidLogLevel(level string) bool {
	switch level {
	case "debug", "info", "warn", "error":
		return true
	default:
		return false
	}
}

type contextReader struct {
	ctx    context.Context
	reader io.Reader
}

func (r *contextReader) Read(p []byte) (int, error) {
	if err := r.ctx.Err(); err != nil {
		return 0, err
	}

	n, err := r.reader.Read(p)
	if err != nil {
		return n, err
	}

	if err := r.ctx.Err(); err != nil {
		return n, err
	}

	return n, nil
}

func expandEnv(data []byte) []byte {
	expanded := envVarPattern.ReplaceAllStringFunc(string(data), func(match string) string {
		parts := envVarPattern.FindStringSubmatch(match)
		if len(parts) != 2 {
			return match
		}

		if value, ok := os.LookupEnv(parts[1]); ok {
			return value
		}

		return match
	})

	return []byte(expanded)
}
