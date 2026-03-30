package config

import (
	"fmt"
	"os"
	"strings"
	"time"

	"gopkg.in/yaml.v3"
)

const (
	defaultLogLevel     = "info"
	defaultFetchTimeout  = 5 * time.Minute
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
	f, err := os.Open(path)
	if err != nil {
		return nil, fmt.Errorf("failed to open config file: %w", err)
	}
	defer f.Close()

	var cfg Config
	if err := yaml.NewDecoder(f).Decode(&cfg); err != nil {
		return nil, fmt.Errorf("failed to parse config: %w", err)
	}

	cfg.setDefaults()

	if err := cfg.validate(); err != nil {
		return nil, err
	}

	return &cfg, nil
}

func (c *Config) validate() error {
	if len(c.Sources) == 0 {
		return fmt.Errorf("at least one source must be configured")
	}
	if !isValidLogLevel(c.LogLevel) {
		return fmt.Errorf("invalid log level %q: expected debug, info, warn, or error", c.LogLevel)
	}
	if c.Timeout.Fetch < 0 {
		return fmt.Errorf("timeout.fetch cannot be negative")
	}
	if c.Retry.Max < 0 {
		return fmt.Errorf("retry.max cannot be negative")
	}
	if c.Retry.Backoff < 0 {
		return fmt.Errorf("retry.backoff cannot be negative")
	}
	return nil
}

func (c *Config) setDefaults() {
	if c.LogLevel == "" {
		c.LogLevel = defaultLogLevel
	}
	if c.Timeout.Fetch == 0 {
		c.Timeout.Fetch = defaultFetchTimeout
	}
	if c.Retry.Max == 0 {
		c.Retry.Max = defaultRetryMax
	}
	if c.Retry.Backoff == 0 {
		c.Retry.Backoff = defaultRetryBackoff
	}
}

func isValidLogLevel(level string) bool {
	switch strings.ToLower(level) {
	case "debug", "info", "warn", "error":
		return true
	default:
		return false
	}
}
