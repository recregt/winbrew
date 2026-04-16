package publisher

import (
	"bytes"
	"context"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"io"
	"net/url"
	"os"
	"path"
	"path/filepath"
	"strings"

	"github.com/minio/minio-go/v7"
	"github.com/minio/minio-go/v7/pkg/credentials"
)

const defaultObjectKey = "catalog.db"

type Config struct {
	Endpoint        string
	BucketName      string
	AccessKeyID     string
	SecretAccessKey string
	Region          string
}

func Run(ctx context.Context, inputPath, metadataPath, objectKey, updatePlansPath, patchChainPath string) (bool, error) {
	inputPath, metadataPath, objectKey, err := resolveRunInputs(inputPath, metadataPath, objectKey)
	if err != nil {
		return false, err
	}

	cfg, err := LoadConfigFromEnv()
	if err != nil {
		return false, err
	}

	client, err := newClient(cfg)
	if err != nil {
		return false, err
	}

	localMetadata, err := loadVerifiedMetadata(inputPath, metadataPath)
	if err != nil {
		return false, err
	}

	published, err := publish(ctx, client, cfg.BucketName, inputPath, metadataPath, objectKey, localMetadata)
	if err != nil {
		return false, err
	}

	if published && strings.TrimSpace(updatePlansPath) != "" {
		if err := WriteUpdatePlansSQL(updatePlansPath, inputPath, metadataPath, objectKey, patchChainPath); err != nil {
			return false, err
		}
	}

	return published, nil
}

func resolveRunInputs(inputPath, metadataPath, objectKey string) (string, string, string, error) {
	inputPath = strings.TrimSpace(inputPath)
	metadataPath = strings.TrimSpace(metadataPath)
	objectKey = strings.TrimSpace(objectKey)

	if strings.TrimSpace(inputPath) == "" {
		inputPath = strings.TrimSpace(os.Getenv("WINBREW_DB_PATH"))
	}
	if inputPath == "" {
		return "", "", "", fmt.Errorf("input path cannot be empty")
	}
	if strings.TrimSpace(metadataPath) == "" {
		metadataPath = defaultMetadataPath(inputPath)
	}
	if strings.TrimSpace(objectKey) == "" {
		objectKey = defaultObjectKey
	}

	return inputPath, metadataPath, objectKey, nil
}

func loadVerifiedMetadata(inputPath, metadataPath string) (Metadata, error) {
	localMetadata, err := LoadMetadata(metadataPath)
	if err != nil {
		return Metadata{}, err
	}

	inputHash, err := hashFile(inputPath)
	if err != nil {
		return Metadata{}, err
	}
	if localMetadata.CurrentHash != inputHash {
		return Metadata{}, fmt.Errorf("metadata current hash mismatch: expected %s, got %s", localMetadata.CurrentHash, inputHash)
	}

	return localMetadata, nil
}

func publish(ctx context.Context, client *minio.Client, bucketName, inputPath, metadataPath, objectKey string, localMetadata Metadata) (bool, error) {
	remoteMetadata, err := loadRemoteMetadata(ctx, client, bucketName, metadataKeyForObjectKey(objectKey))
	if err != nil {
		return false, err
	}
	if remoteMetadata != nil && remoteMetadata.CurrentHash == localMetadata.CurrentHash {
		return false, nil
	}
	if remoteMetadata != nil && remoteMetadata.CurrentHash != "" {
		localMetadata.PreviousHash = remoteMetadata.CurrentHash
	}

	if err := uploadObjects(ctx, client, bucketName, inputPath, objectKey, localMetadata); err != nil {
		return false, err
	}

	if err := SaveMetadata(metadataPath, localMetadata); err != nil {
		return false, err
	}

	return true, nil
}

func LoadConfigFromEnv() (Config, error) {
	cfg := Config{
		Endpoint:        strings.TrimSpace(os.Getenv("R2_ENDPOINT")),
		BucketName:      strings.TrimSpace(os.Getenv("R2_BUCKET_NAME")),
		AccessKeyID:     firstNonEmpty(os.Getenv("R2_ACCESS_KEY_ID"), os.Getenv("AWS_ACCESS_KEY_ID")),
		SecretAccessKey: firstNonEmpty(os.Getenv("R2_SECRET_ACCESS_KEY"), os.Getenv("AWS_SECRET_ACCESS_KEY")),
		Region:          firstNonEmpty(os.Getenv("R2_REGION"), "auto"),
	}

	if cfg.Endpoint == "" {
		return Config{}, fmt.Errorf("R2_ENDPOINT cannot be empty")
	}
	if cfg.BucketName == "" {
		return Config{}, fmt.Errorf("R2_BUCKET_NAME cannot be empty")
	}
	if cfg.AccessKeyID == "" {
		return Config{}, fmt.Errorf("access key id cannot be empty")
	}
	if cfg.SecretAccessKey == "" {
		return Config{}, fmt.Errorf("secret access key cannot be empty")
	}

	return cfg, nil
}

func newClient(cfg Config) (*minio.Client, error) {
	host, secure, err := normalizeEndpoint(cfg.Endpoint)
	if err != nil {
		return nil, err
	}

	client, err := minio.New(host, &minio.Options{
		Creds:        credentials.NewStaticV4(cfg.AccessKeyID, cfg.SecretAccessKey, ""),
		Secure:       secure,
		Region:       cfg.Region,
		BucketLookup: minio.BucketLookupPath,
	})
	if err != nil {
		return nil, fmt.Errorf("failed to create R2 client: %w", err)
	}

	return client, nil
}

func normalizeEndpoint(raw string) (string, bool, error) {
	raw = strings.TrimSpace(raw)
	if raw == "" {
		return "", false, fmt.Errorf("invalid R2_ENDPOINT: empty")
	}

	if strings.Contains(raw, "://") {
		parsed, err := url.Parse(raw)
		if err != nil {
			return "", false, fmt.Errorf("invalid R2_ENDPOINT: %w", err)
		}
		if parsed.Host == "" {
			return "", false, fmt.Errorf("invalid R2_ENDPOINT: %q", raw)
		}
		if parsed.Path != "" && parsed.Path != "/" {
			return "", false, fmt.Errorf("invalid R2_ENDPOINT: path not allowed: %q", raw)
		}

		switch parsed.Scheme {
		case "http":
			return parsed.Host, false, nil
		case "https":
			return parsed.Host, true, nil
		default:
			return "", false, fmt.Errorf("unsupported R2_ENDPOINT scheme: %s", parsed.Scheme)
		}
	}

	if strings.ContainsAny(raw, " /?#") {
		return "", false, fmt.Errorf("invalid R2_ENDPOINT: path not allowed without scheme: %q", raw)
	}

	return raw, true, nil
}

func firstNonEmpty(values ...string) string {
	for _, value := range values {
		if trimmed := strings.TrimSpace(value); trimmed != "" {
			return trimmed
		}
	}
	return ""
}

func defaultMetadataPath(inputPath string) string {
	return filepath.Join(filepath.Dir(inputPath), "metadata.json")
}

func hashFile(path string) (string, error) {
	file, err := os.Open(path)
	if err != nil {
		return "", fmt.Errorf("failed to open file for hashing: %w", err)
	}
	defer file.Close()

	hasher := sha256.New()
	if _, err := io.Copy(hasher, file); err != nil {
		return "", fmt.Errorf("failed to hash file: %w", err)
	}

	return "sha256:" + hex.EncodeToString(hasher.Sum(nil)), nil
}

func loadRemoteMetadata(ctx context.Context, client *minio.Client, bucketName, metadataKey string) (*Metadata, error) {
	object, err := client.GetObject(ctx, bucketName, metadataKey, minio.GetObjectOptions{})
	if err != nil {
		if isMissingObject(err) {
			return nil, nil
		}
		return nil, fmt.Errorf("failed to open remote metadata: %w", err)
	}
	defer object.Close()

	var metadata Metadata
	if err := json.NewDecoder(object).Decode(&metadata); err != nil {
		if isMissingObject(err) {
			return nil, nil
		}
		return nil, fmt.Errorf("failed to decode remote metadata: %w", err)
	}

	return &metadata, nil
}

func isMissingObject(err error) bool {
	response := minio.ToErrorResponse(err)
	switch response.Code {
	case "NoSuchKey", "NoSuchObject":
		return true
	default:
		return response.StatusCode == 404
	}
}

func uploadObjects(ctx context.Context, client *minio.Client, bucketName, inputPath, objectKey string, metadata Metadata) error {
	tempObjectKey := objectTempKeyForObjectKey(objectKey)
	if _, err := client.FPutObject(ctx, bucketName, tempObjectKey, inputPath, minio.PutObjectOptions{
		ContentType: "application/octet-stream",
	}); err != nil {
		return fmt.Errorf("failed to upload %s to temporary object %s in bucket %s: %w", filepath.Base(inputPath), tempObjectKey, bucketName, err)
	}
	defer func() {
		_ = client.RemoveObject(ctx, bucketName, tempObjectKey, minio.RemoveObjectOptions{})
	}()

	if _, err := client.CopyObject(ctx, minio.CopyDestOptions{
		Bucket: bucketName,
		Object: objectKey,
	}, minio.CopySrcOptions{
		Bucket: bucketName,
		Object: tempObjectKey,
	}); err != nil {
		return fmt.Errorf("failed to publish object %s to bucket %s: %w", objectKey, bucketName, err)
	}

	metadataBytes, err := metadataBytes(metadata)
	if err != nil {
		return err
	}

	metadataKey := metadataKeyForObjectKey(objectKey)
	tempMetadataKey := metadataTempKeyForObjectKey(objectKey)
	if _, err := client.PutObject(ctx, bucketName, tempMetadataKey, bytes.NewReader(metadataBytes), int64(len(metadataBytes)), minio.PutObjectOptions{
		ContentType: "application/json",
	}); err != nil {
		return fmt.Errorf("failed to upload temporary metadata object %s to bucket %s: %w", tempMetadataKey, bucketName, err)
	}
	defer func() {
		_ = client.RemoveObject(ctx, bucketName, tempMetadataKey, minio.RemoveObjectOptions{})
	}()

	if _, err := client.CopyObject(ctx, minio.CopyDestOptions{
		Bucket: bucketName,
		Object: metadataKey,
	}, minio.CopySrcOptions{
		Bucket: bucketName,
		Object: tempMetadataKey,
	}); err != nil {
		return fmt.Errorf("failed to publish metadata object %s to bucket %s: %w", metadataKey, bucketName, err)
	}

	return nil
}

func objectTempKeyForObjectKey(objectKey string) string {
	return path.Join(path.Dir(objectKey), path.Base(objectKey)+".tmp")
}
