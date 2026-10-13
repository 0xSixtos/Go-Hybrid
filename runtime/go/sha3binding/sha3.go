package sha3binding

import (
        "errors"
        "runtime"
        "unsafe"

        "hybrid.program/hybridge"
)

// The binding's error surface (Spike 5's sentinel shape): Go-side constants,
// mapped from the C-ABI status word — wrap-on-demand, ≤1 allocation (E5).
var (
        ErrDigest = errors.New("sha3: digest256 failed")
        ErrABI    = errors.New("sha3: hyrt ABI handshake mismatch (rebuild the runtime)")
)

func init() {
        if w := hybridge.Cross1(FnAbiWord, 0); w != abiWordExpected {
                panic(ErrABI.Error())
        }
}

// Digest256 computes SHA3-256(data) — the pinned-borrow protocol in full:
//
//      pin data's backing array → cross (ptr, len, out) → unpin + KeepAlive.
//
// The 32-byte digest buffer is GO-owned (allocated here, written by Rust
// through the pinned pointer during the call; the pointer is never
// retained — abi-types.md §2). Empty input is legal (SHA3 of the empty
// string); the nil-data leg passes 0/0.
func Digest256(data []byte) ([]byte, error) {
        out := make([]byte, 32)
        var p runtime.Pinner
        var dataPtr uintptr
        if len(data) > 0 {
                dataPtr = uintptr(unsafe.Pointer(&data[0]))
                p.Pin(&data[0])
        }
        p.Pin(&out[0])
        st := hybridge.Cross3(FnDigest256, dataPtr, uintptr(len(data)),
                uintptr(unsafe.Pointer(&out[0])))
        p.Unpin()
        runtime.KeepAlive(&p)
        runtime.KeepAlive(data)
        runtime.KeepAlive(out)
        if st != 0 {
                return nil, ErrDigest
        }
        return out, nil
}

// StreamChecksum runs `iters` SHA3-256 permutations INSIDE Rust (one
// boundary crossing — the hot-loop shape example 10 demonstrates) and
// returns the deterministic fold. No pointers cross; the deep-path retry
// is entirely inside Cross2 (bounded-stack callee).
func StreamChecksum(iters, seed uint64) (uint64, error) {
        v := hybridge.Cross2(FnStreamChecksum, uintptr(iters), uintptr(seed))
        return v, nil
}
