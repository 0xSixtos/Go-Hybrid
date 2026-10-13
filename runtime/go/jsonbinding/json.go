package jsonbinding

import (
	"encoding/json"
	"errors"
	"runtime"
	"unsafe"

	"hybrid.program/hybridge"
)

// The binding's error surface (Spike 5 sentinel shape; wrap-on-demand, E5).
var (
	ErrJSON = errors.New("serde_json: operation failed")
	ErrABI  = errors.New("serde_json: hyrt ABI handshake mismatch")
)

// Value is a Rust-owned JSON value handle. Zero id = released/moved.
type Value struct{ h *handle }

type handle struct{ id uint64 }

func wrap(id uint64) Value {
	h := &handle{id: id}
	// Late release is safe (hyrt's release is idempotent + generation-
	// checked); a MOVED value zeroes its id first, so this releases nothing.
	runtime.AddCleanup(h, func(id uint64) {
		if id != 0 {
			hybridge.Cross1(FnRelease, uintptr(id))
		}
	}, id)
	return Value{h: h}
}

// pinCrossKey: the copy-small pinned borrow for a key/string argument.
// Returns the byte copy's pointer + len and a release func.
func pinCrossKey(s string) (uintptr, uintptr, *runtime.Pinner, []byte) {
	b := []byte(s)
	var p runtime.Pinner
	if len(b) > 0 {
		p.Pin(&b[0])
		return uintptr(unsafe.Pointer(&b[0])), uintptr(len(b)), &p, b
	}
	// Empty key: pass (0, 0) — hyrt's HyStr::as_str maps len==0 to "".
	return 0, 0, &p, b
}

// Parse parses JSON text into a fresh value handle.
func Parse(data []byte) (Value, error) {
	var id uint64
	var p runtime.Pinner
	var dptr uintptr
	if len(data) > 0 {
		p.Pin(&data[0])
		dptr = uintptr(unsafe.Pointer(&data[0]))
	}
	p.Pin(&id)
	st := hybridge.Cross3(FnParse, dptr, uintptr(len(data)), uintptr(unsafe.Pointer(&id)))
	p.Unpin()
	runtime.KeepAlive(&p)
	runtime.KeepAlive(data)
	if st != statusOK {
		return Value{}, ErrJSON
	}
	return wrap(id), nil
}

// NewObject / NewArray: fresh containers.
func NewObject() Value {
	var id uint64
	hybridge.Cross1(FnNewObject, uintptr(unsafe.Pointer(&id)))
	return wrap(id)
}

func NewArray() Value {
	var id uint64
	hybridge.Cross1(FnNewArray, uintptr(unsafe.Pointer(&id)))
	return wrap(id)
}

// Array returns the array at v[key], MATERIALIZED as a Go slice of handles
// (len + N element crossings inside the binding) — the .hy surface iterates
// `for u in v.array(k)?` as a plain slice range.
func (v Value) Array(key string) ([]Value, error) {
	if v.h == nil || v.h.id == 0 {
		return nil, ErrJSON
	}
	var arrID uint64
	kptr, klen, pin, kb := pinCrossKey(key)
	pin.Pin(&arrID)
	st := hybridge.Cross4(FnGetArray, uintptr(v.h.id), kptr, klen, uintptr(unsafe.Pointer(&arrID)))
	pin.Unpin()
	runtime.KeepAlive(pin)
	runtime.KeepAlive(kb)
	if st != statusOK {
		return nil, ErrJSON
	}
	var n uint64
	hybridge.Cross2(FnArrayLen, uintptr(arrID), uintptr(unsafe.Pointer(&n)))
	out := make([]Value, n)
	for i := range out {
		var eid uint64
		hybridge.Cross3(FnArrayGet, uintptr(arrID), uintptr(i), uintptr(unsafe.Pointer(&eid)))
		out[i] = wrap(eid)
	}
	// The temporary array handle dies with the materialized slice.
	hybridge.Cross1(FnRelease, uintptr(arrID))
	return out, nil
}

// Int returns the i64 at v[key].
func (v Value) Int(key string) (int64, error) {
	if v.h == nil || v.h.id == 0 {
		return 0, ErrJSON
	}
	var n int64
	kptr, klen, pin, kb := pinCrossKey(key)
	pin.Pin(&n)
	st := hybridge.Cross4(FnGetInt, uintptr(v.h.id), kptr, klen, uintptr(unsafe.Pointer(&n)))
	pin.Unpin()
	runtime.KeepAlive(pin)
	runtime.KeepAlive(kb)
	if st != statusOK {
		return 0, ErrJSON
	}
	return n, nil
}

// String returns the string at v[key] (one extra release crossing for the
// temporary string handle; the text itself copies out through the handle).
func (v Value) String(key string) (string, error) {
	if v.h == nil || v.h.id == 0 {
		return "", ErrJSON
	}
	var sid uint64
	kptr, klen, pin, kb := pinCrossKey(key)
	pin.Pin(&sid)
	st := hybridge.Cross4(FnGetString, uintptr(v.h.id), kptr, klen, uintptr(unsafe.Pointer(&sid)))
	pin.Unpin()
	runtime.KeepAlive(pin)
	runtime.KeepAlive(kb)
	if st != statusOK {
		return "", ErrJSON
	}
	s := valueText(sid)
	hybridge.Cross1(FnRelease, uintptr(sid))
	return s, nil
}

// PushString appends s to an array value (example 05's arr.push_string).
func (v Value) PushString(s string) error {
	if v.h == nil || v.h.id == 0 {
		return ErrJSON
	}
	sptr, slen, pin, sb := pinCrossKey(s)
	st := hybridge.Cross3(FnPushString, uintptr(v.h.id), sptr, slen)
	pin.Unpin()
	runtime.KeepAlive(pin)
	runtime.KeepAlive(sb)
	if st != statusOK {
		return ErrJSON
	}
	return nil
}

// Set moves val into v[key]. The scalar surface (differential corpus 12/13)
// takes any Go scalar: a Value handle moves directly; bool/int/float/string
// box through a JSON round-trip (Marshal produces the exact JSON text, Parse
// allocates the Rust-side handle, set moves it in — the temp's late cleanup
// releases nothing because the move zeroes its id). Set reports the Rust
// status (hy_json_set's i32) as an error — `doc.set(k, v)?` propagates it.
func (v Value) Set(key string, val any) error {
	if hv, ok := val.(Value); ok {
		return v.setHandle(key, hv)
	}
	b, err := json.Marshal(val)
	if err != nil {
		return err
	}
	bv, err := Parse(b)
	if err != nil {
		return err
	}
	return v.setHandle(key, bv)
}

func (v Value) setHandle(key string, val Value) error {
	if v.h == nil || v.h.id == 0 || val.h == nil || val.h.id == 0 {
		return ErrJSON
	}
	kptr, klen, pin, kb := pinCrossKey(key)
	st := hybridge.Cross4(FnSet, uintptr(v.h.id), kptr, klen, uintptr(val.h.id))
	pin.Unpin()
	runtime.KeepAlive(pin)
	runtime.KeepAlive(kb)
	if st != statusOK {
		return ErrJSON
	}
	val.h.id = 0 // moved: the handle's payload now lives inside v
	return nil
}

// ToStringPretty serializes v (two-leg buffer protocol: the first crossing
// reports the needed length via status-3; the second copies).
func ToStringPretty(v Value) (string, error) {
	if v.h == nil || v.h.id == 0 {
		return "", ErrJSON
	}
	var need uint64
	st := hybridge.Cross4(FnToStringPretty, uintptr(v.h.id), 0, 0, uintptr(unsafe.Pointer(&need)))
	if st == statusTryAgain && need > 0 {
		buf := make([]byte, need)
		var p runtime.Pinner
		p.Pin(&buf[0])
		st = hybridge.Cross4(FnToStringPretty, uintptr(v.h.id),
			uintptr(unsafe.Pointer(&buf[0])), uintptr(len(buf)), uintptr(unsafe.Pointer(&need)))
		p.Unpin()
		runtime.KeepAlive(&p)
		runtime.KeepAlive(buf)
		if st == statusOK {
			return string(buf[:need]), nil
		}
	}
	if st != statusOK {
		return "", ErrJSON
	}
	return "", ErrJSON
}

// valueText reads a string-value's text (compact to_string of a Json::String
// is the quoted form; hyrt's get_string already stores the raw text as a
// Json::String, so to_string would QUOTE it — hy_json_value_text on a bare
// string value returns the quoted JSON. The binding therefore reads via the
// compact form and strips the quotes it added — one documented copy).
func valueText(id uint64) string {
	var need uint64
	st := hybridge.Cross4(FnToStringPretty, uintptr(id), 0, 0, uintptr(unsafe.Pointer(&need)))
	if st != statusTryAgain || need < 2 {
		return ""
	}
	buf := make([]byte, need)
	var p runtime.Pinner
	p.Pin(&buf[0])
	hybridge.Cross4(FnToStringPretty, uintptr(id), uintptr(unsafe.Pointer(&buf[0])), uintptr(len(buf)), uintptr(unsafe.Pointer(&need)))
	p.Unpin()
	runtime.KeepAlive(&p)
	runtime.KeepAlive(buf)
	// Json::String's to_string is `"text"` — strip the 2-quote wrapper.
	s := string(buf[:need])
	if len(s) >= 2 && s[0] == '"' && s[len(s)-1] == '"' {
		return s[1 : len(s)-1]
	}
	return s
}
