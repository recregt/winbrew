package config

import (
	"fmt"
	"os"
	"strings"
	"time"

	"gopkg.in/yaml.v3"
)

const (
	defaultLogLevel      = "info"
	defaultFetchTimeout  = 5 * time.Minute
	defaultUploadTimeout = 10 * time.Minute
	defaultRetryMax      = 3
	defaultRetryBackoff  = 2 * time.Second
)

type Config struct {
	Sources  []string    `yaml:"sources"`
	CDN      CDNConfig   `yaml:"cdn"`
	LogLevel string      `yaml:"logLevel"` // debug, info, warn, error
	Timeout  Timeout     `yaml:"timeout"`
	Retry    RetryConfig `yaml:"retry"`
}

type CDNConfig struct {
	Provider           string `yaml:"provider"` // r2, s3, minio
	Bucket             string `yaml:"bucket"`
	Endpoint           string `yaml:"endpoint"`
	AccessKeyIDEnv     string `yaml:"accessKeyIDEnv"`
	SecretAccessKeyEnv string `yaml:"secretAccessKeyEnv"`
}

type Timeout struct {
	Fetch  time.Duration `yaml:"fetch"`  // source fetching
	Upload time.Duration `yaml:"upload"` // cdn uploading
}

type RetryConfig struct {
	Max     int           `yaml:"max"`
	Backoff time.Duration `yaml:"backoff"`
}

func (c *CDNConfig) ResolveCredentials() (accessKeyID, secretAccessKey string, err error) {
	if c.AccessKeyIDEnv == "" {
		return "", "", fmt.Errorf("cdn.accessKeyIDEnv is empty; environment variable name must be provided")
	}
	accessKeyID = os.Getenv(c.AccessKeyIDEnv)
	if accessKeyID == "" {
		return "", "", fmt.Errorf("environment variable %q is not set or empty", c.AccessKeyIDEnv)
	}
	if c.SecretAccessKeyEnv == "" {
		return "", "", fmt.Errorf("cdn.secretAccessKeyEnv is empty; environment variable name must be provided")
	}
	secretAccessKey = os.Getenv(c.SecretAccessKeyEnv)
	if secretAccessKey == "" {
		return "", "", fmt.Errorf("environment variable %q is not set or empty", c.SecretAccessKeyEnv)
	}
	return accessKeyID, secretAccessKey, nil
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
	if c.CDN.Bucket == "" {
		return fmt.Errorf("cdn.bucket cannot be empty")
	}
	if c.CDN.Provider == "" {
		return fmt.Errorf("cdn.provider cannot be empty")
	}
	if c.Timeout.Fetch < 0 {
		return fmt.Errorf("timeout.fetch cannot be negative")
	}
	if c.Timeout.Upload < 0 {
		return fmt.Errorf("timeout.upload cannot be negative")
	}
	if c.Retry.Max < 0 {
		return fmt.Errorf("retry.max cannot be negative")
	}
	if c.Retry.Backoff < 0 {
		return fmt.Errorf("retry.backoff cannot be negative")
	}
	switch c.CDN.Provider {
	case "r2", "s3", "minio":
		if c.CDN.Endpoint == "" {
			return fmt.Errorf("cdn.endpoint cannot be empty for %s provider", c.CDN.Provider)
		}
		if c.CDN.AccessKeyIDEnv == "" {
			return fmt.Errorf("cdn.accessKeyIDEnv cannot be empty for %s provider", c.CDN.Provider)
		}
		if c.CDN.SecretAccessKeyEnv == "" {
			return fmt.Errorf("cdn.secretAccessKeyEnv cannot be empty for %s provider", c.CDN.Provider)
		}
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
	if c.Timeout.Upload == 0 {
		c.Timeout.Upload = defaultUploadTimeout
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
