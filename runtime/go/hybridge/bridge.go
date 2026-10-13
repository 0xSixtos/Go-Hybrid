// Package hybridge — the Hybrid runtime's boundary core (Phase 2).
//
// Lifted from Spike 3's working code (spikes/spike3-memory/work/go/hybridge),
// which passed its pre-registered thresholds both tiers (sandbox + Kaggle
// worker). The spike's handle-table / arena / probe machinery is NOT part of
// the runtime — per-crate bindings live in their own packages and use only:
//
//   - Cross1/2/3: the guarded crossing (fast path on the caller's goroutine
//     stack after an IN-TRAMPOLINE stackguard check; deep path on a pooled
//     1 MiB guarded mmap stack when headroom is short);
//   - the pinned-borrow protocol (runtime.Pinner pin → cross → unpin +
//     KeepAlive) that every binding emits around pointer-typed arguments.
//
// Trampoline discipline (both paths — the rules the bridge generator emits):
//   1. all Go-frame inputs loaded BEFORE any SP adjustment;
//   2. old SP saved in a SysV callee-saved register (RBX), restored BEFORE
//      writing results;
//   3. 16-byte SysV alignment at callee entry (rsp%16==8);
//   4. NOSPLIT: no Go stack growth inside the trampoline;
//   5. no callbacks into Go while on a deep stack; panics abort at the Rust
//      edge (E9) — they never unwind across.
package hybridge

import "unsafe"

//go:noescape
func spHeadroom() uintptr

//go:noescape
func callFast1(fn unsafe.Pointer, a1 uintptr) uint64

//go:noescape
func callFast2(fn unsafe.Pointer, a1, a2 uintptr) uint64

//go:noescape
func callFast3(fn unsafe.Pointer, a1, a2, a3 uintptr) uint64

//go:noescape
func callDeep1(fn unsafe.Pointer, a1 uintptr, stack uintptr) uint64

//go:noescape
func callDeep2(fn unsafe.Pointer, a1, a2 uintptr, stack uintptr) uint64

//go:noescape
func callDeep3(fn unsafe.Pointer, a1, a2, a3 uintptr, stack uintptr) uint64

// fastStackNeed: headroom required to run a callee on the caller's own
// goroutine stack. Callees must have BOUNDED stack use (the descriptor
// carries a per-fn stack budget; unbounded callees take the deep path).
const fastStackNeed = 4096

// needsDeep: sentinel returned by the fast trampoline when headroom is short.
const needsDeep = ^uint64(1)

// Cross1/2/3: the generated crossing. Fast path = caller's stack (guard
// checked IN the trampoline); deep path = pooled guarded mmap stack.
func Cross1(fn unsafe.Pointer, a1 uintptr) uint64 {
	v := callFast1(fn, a1)
	if v == needsDeep {
		i := acquireDeep()
		v = callDeep1(fn, a1, deepTops[i])
		releaseDeep(i)
	}
	return v
}

func Cross2(fn unsafe.Pointer, a1, a2 uintptr) uint64 {
	v := callFast2(fn, a1, a2)
	if v == needsDeep {
		i := acquireDeep()
		v = callDeep2(fn, a1, a2, deepTops[i])
		releaseDeep(i)
	}
	return v
}

func Cross3(fn unsafe.Pointer, a1, a2, a3 uintptr) uint64 {
	v := callFast3(fn, a1, a2, a3)
	if v == needsDeep {
		i := acquireDeep()
		v = callDeep3(fn, a1, a2, a3, deepTops[i])
		releaseDeep(i)
	}
	return v
}

// CrossDeep1: ALWAYS the pooled stack — for callees with unbounded stack use.
func CrossDeep1(fn unsafe.Pointer, a1 uintptr) uint64 {
	i := acquireDeep()
	v := callDeep1(fn, a1, deepTops[i])
	releaseDeep(i)
	return v
}

// SPHeadroomForTest exposes the stackguard headroom for the offsets canary.
func SPHeadroomForTest() uintptr { return spHeadroom() }
