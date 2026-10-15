package main

import (
	"fmt"
	"os"
	"path/filepath"
)

func main() {
	hyErr := func() (hyErr error) {
		enc, hyErr0 := flate2.new_writer(6)
		if hyErr0 != nil {
			hyErr = hyErr0
			return
		}
		fmt.Println(enc)
		return
	}()
	if hyErr != nil {
		fmt.Fprintln(os.Stderr, filepath.Base(os.Args[0])+":", hyErr)
		os.Exit(1)
	}
}
