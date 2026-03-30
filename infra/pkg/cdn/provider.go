package cdn

import "context"

type Provider interface {
	Upload(ctx context.Context, key string, path string) error
	PublicURL(key string) string
}
