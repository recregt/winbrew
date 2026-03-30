package sources

import (
	"context"

	"winbrew/infra/pkg/normalize"
)

type Source interface {
	Name() string
	Fetch(ctx context.Context) ([]normalize.Package, error)
}
