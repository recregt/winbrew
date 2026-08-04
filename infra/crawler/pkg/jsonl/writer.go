// Package jsonl provides a small buffered-writer helper shared by the
// crawler's JSONL-emitting sources.
package jsonl

import (
	"bufio"
	"io"
)

// BufferedWriter wraps w in a buffered writer, unless it already is one, and
// returns the writer alongside a flush func that must be called once writing
// is done.
func BufferedWriter(w io.Writer) (io.Writer, func() error) {
	if bw, ok := w.(*bufio.Writer); ok {
		return bw, bw.Flush
	}

	bw := bufio.NewWriterSize(w, 64*1024)
	return bw, bw.Flush
}
