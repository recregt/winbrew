package models

import (
	"fmt"
	"strings"

	"gopkg.in/yaml.v3"
)

type FlexibleStringSlice []string

func (f *FlexibleStringSlice) UnmarshalYAML(node *yaml.Node) error {
	if node == nil {
		*f = nil
		return nil
	}

	if node.Kind == yaml.AliasNode && node.Alias != nil {
		node = node.Alias
	}

	switch node.Kind {
	case yaml.SequenceNode:
		var values []string
		if err := node.Decode(&values); err != nil {
			return err
		}

		result := make([]string, 0, len(values))
		for _, value := range values {
			if trimmed := strings.TrimSpace(value); trimmed != "" {
				result = append(result, trimmed)
			}
		}

		if len(result) == 0 {
			*f = nil
			return nil
		}

		*f = result
		return nil
	case yaml.ScalarNode:
		if node.Tag == "!!null" {
			*f = nil
			return nil
		}

		var single string
		if err := node.Decode(&single); err != nil {
			return err
		}

		if trimmed := strings.TrimSpace(single); trimmed != "" {
			*f = FlexibleStringSlice{trimmed}
			return nil
		}

		*f = nil
		return nil
	default:
		return fmt.Errorf("unsupported flexible string slice YAML kind: %d", node.Kind)
	}
}
