package sources

import (
	"context"

	"github.com/recregt/winbrew-infra/pkg/normalize"
)

type Source interface {
	Name() string
	Fetch(ctx context.Context) ([]normalize.Package, error)
}
