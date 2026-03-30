package r2

import (
	"context"
	"fmt"
	"net/url"
	"os"

	"github.com/aws/aws-sdk-go-v2/aws"
	"github.com/aws/aws-sdk-go-v2/config"
	"github.com/aws/aws-sdk-go-v2/credentials"
	"github.com/aws/aws-sdk-go-v2/service/s3"
)

type Client struct {
	s3       *s3.Client
	bucket   string
	endpoint string
}

func New(endpoint, bucket, accessKeyID, secretAccessKey string) (*Client, error) {
	if endpoint == "" {
		return nil, fmt.Errorf("endpoint cannot be empty")
	}
	if bucket == "" {
		return nil, fmt.Errorf("bucket cannot be empty")
	}
	if accessKeyID == "" {
		return nil, fmt.Errorf("access key id cannot be empty")
	}
	if secretAccessKey == "" {
		return nil, fmt.Errorf("secret access key cannot be empty")
	}

	cfg, err := config.LoadDefaultConfig(context.Background(),
		config.WithRegion("auto"),
		config.WithCredentialsProvider(
			credentials.NewStaticCredentialsProvider(accessKeyID, secretAccessKey, ""),
		),
	)
	if err != nil {
		return nil, fmt.Errorf("failed to load aws config: %w", err)
	}

	client := s3.NewFromConfig(cfg, func(o *s3.Options) {
		o.BaseEndpoint = aws.String(endpoint)
	})

	return &Client{
		s3:       client,
		bucket:   bucket,
		endpoint: endpoint,
	}, nil
}

func (c *Client) Upload(ctx context.Context, key, path string) error {
	f, err := os.Open(path)
	if err != nil {
		return fmt.Errorf("failed to open file: %w", err)
	}
	defer f.Close()

	_, err = c.s3.PutObject(ctx, &s3.PutObjectInput{
		Bucket: aws.String(c.bucket),
		Key:    aws.String(key),
		Body:   f,
	})
	if err != nil {
		return fmt.Errorf("failed to upload %s: %w", key, err)
	}
	return nil
}

func (c *Client) PublicURL(key string) string {
	publicURL, err := url.JoinPath(c.endpoint, c.bucket, key)
	if err != nil {
		return fmt.Sprintf("%s/%s/%s", c.endpoint, c.bucket, key)
	}
	return publicURL
}
