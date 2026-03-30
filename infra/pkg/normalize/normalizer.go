package normalize

type Normalizer[T any] interface {
	Normalize(raw T) (Package, error)
}
