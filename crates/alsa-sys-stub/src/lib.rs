//! Stub replacement for `alsa-sys` that provides the same public API surface
//! used by `tinyaudio` but without requiring `libasound2-dev` to be installed.
//!
//! Every PCM function returns an error code (`-1`), which causes `tinyaudio`
//! (and therefore Fyrox) to gracefully skip audio initialisation.  This lets
//! the game build and run on platforms where ALSA headers/libraries are
//! unavailable — the only trade-off is that there will be no sound output.

#![allow(non_camel_case_types, non_upper_case_globals)]

use std::os::raw::{c_char, c_int, c_long, c_uint, c_ulong, c_void};

// ---------------------------------------------------------------------------
// Opaque types (pointers are always null in practice; open always fails)
// ---------------------------------------------------------------------------

/// Opaque PCM handle.
pub enum snd_pcm_t {}

/// Opaque hardware-params handle.
pub enum snd_pcm_hw_params_t {}

/// Opaque software-params handle.
pub enum snd_pcm_sw_params_t {}

// ---------------------------------------------------------------------------
// Type aliases
// ---------------------------------------------------------------------------

pub type snd_pcm_uframes_t = c_ulong;
pub type snd_pcm_sframes_t = c_long;
pub type snd_pcm_stream_t = c_uint;
pub type snd_pcm_access_t = c_uint;
pub type snd_pcm_format_t = c_int;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

pub const SND_PCM_STREAM_PLAYBACK: snd_pcm_stream_t = 0;
pub const SND_PCM_STREAM_CAPTURE: snd_pcm_stream_t = 1;

pub const SND_PCM_ACCESS_RW_INTERLEAVED: snd_pcm_access_t = 3;

pub const SND_PCM_FORMAT_S16_LE: snd_pcm_format_t = 2;

pub const SND_PCM_NONBLOCK: c_int = 0x1;
pub const SND_PCM_ASYNC: c_int = 0x2;

// ---------------------------------------------------------------------------
// Stub error code
// ---------------------------------------------------------------------------

const STUB_ERR: c_int = -1;

static STUB_MSG: &[u8] = b"ALSA not available (stub build)\0";

// ---------------------------------------------------------------------------
// Functions used by tinyaudio
// ---------------------------------------------------------------------------

/// Return a human-readable error string.
#[no_mangle]
pub unsafe extern "C" fn snd_strerror(_errnum: c_int) -> *const c_char {
    STUB_MSG.as_ptr() as *const c_char
}

/// Open a PCM device — always fails in the stub.
#[no_mangle]
pub unsafe extern "C" fn snd_pcm_open(
    _pcm: *mut *mut snd_pcm_t,
    _name: *const c_char,
    _stream: snd_pcm_stream_t,
    _mode: c_int,
) -> c_int {
    STUB_ERR
}

/// Close a PCM device.
#[no_mangle]
pub unsafe extern "C" fn snd_pcm_close(_pcm: *mut snd_pcm_t) -> c_int {
    STUB_ERR
}

/// Prepare a PCM device.
#[no_mangle]
pub unsafe extern "C" fn snd_pcm_prepare(_pcm: *mut snd_pcm_t) -> c_int {
    STUB_ERR
}

/// Write interleaved frames.
#[no_mangle]
pub unsafe extern "C" fn snd_pcm_writei(
    _pcm: *mut snd_pcm_t,
    _buffer: *const c_void,
    _size: snd_pcm_uframes_t,
) -> snd_pcm_sframes_t {
    STUB_ERR as snd_pcm_sframes_t
}

/// Recover from an error.
#[no_mangle]
pub unsafe extern "C" fn snd_pcm_recover(
    _pcm: *mut snd_pcm_t,
    _err: c_int,
    _silent: c_int,
) -> c_int {
    STUB_ERR
}

// -- Hardware parameters -----------------------------------------------------

#[no_mangle]
pub unsafe extern "C" fn snd_pcm_hw_params_malloc(
    _ptr: *mut *mut snd_pcm_hw_params_t,
) -> c_int {
    STUB_ERR
}

#[no_mangle]
pub unsafe extern "C" fn snd_pcm_hw_params_free(_params: *mut snd_pcm_hw_params_t) {
}

#[no_mangle]
pub unsafe extern "C" fn snd_pcm_hw_params_any(
    _pcm: *mut snd_pcm_t,
    _params: *mut snd_pcm_hw_params_t,
) -> c_int {
    STUB_ERR
}

#[no_mangle]
pub unsafe extern "C" fn snd_pcm_hw_params(
    _pcm: *mut snd_pcm_t,
    _params: *mut snd_pcm_hw_params_t,
) -> c_int {
    STUB_ERR
}

#[no_mangle]
pub unsafe extern "C" fn snd_pcm_hw_params_set_access(
    _pcm: *mut snd_pcm_t,
    _params: *mut snd_pcm_hw_params_t,
    _access: snd_pcm_access_t,
) -> c_int {
    STUB_ERR
}

#[no_mangle]
pub unsafe extern "C" fn snd_pcm_hw_params_set_format(
    _pcm: *mut snd_pcm_t,
    _params: *mut snd_pcm_hw_params_t,
    _format: snd_pcm_format_t,
) -> c_int {
    STUB_ERR
}

#[no_mangle]
pub unsafe extern "C" fn snd_pcm_hw_params_set_rate_near(
    _pcm: *mut snd_pcm_t,
    _params: *mut snd_pcm_hw_params_t,
    _val: *mut c_uint,
    _dir: *mut c_int,
) -> c_int {
    STUB_ERR
}

#[no_mangle]
pub unsafe extern "C" fn snd_pcm_hw_params_set_channels(
    _pcm: *mut snd_pcm_t,
    _params: *mut snd_pcm_hw_params_t,
    _val: c_uint,
) -> c_int {
    STUB_ERR
}

#[no_mangle]
pub unsafe extern "C" fn snd_pcm_hw_params_set_period_size_near(
    _pcm: *mut snd_pcm_t,
    _params: *mut snd_pcm_hw_params_t,
    _val: *mut snd_pcm_uframes_t,
    _dir: *mut c_int,
) -> c_int {
    STUB_ERR
}

#[no_mangle]
pub unsafe extern "C" fn snd_pcm_hw_params_set_buffer_size_near(
    _pcm: *mut snd_pcm_t,
    _params: *mut snd_pcm_hw_params_t,
    _val: *mut snd_pcm_uframes_t,
) -> c_int {
    STUB_ERR
}

// -- Software parameters -----------------------------------------------------

#[no_mangle]
pub unsafe extern "C" fn snd_pcm_sw_params_malloc(
    _ptr: *mut *mut snd_pcm_sw_params_t,
) -> c_int {
    STUB_ERR
}

#[no_mangle]
pub unsafe extern "C" fn snd_pcm_sw_params_current(
    _pcm: *mut snd_pcm_t,
    _params: *mut snd_pcm_sw_params_t,
) -> c_int {
    STUB_ERR
}

#[no_mangle]
pub unsafe extern "C" fn snd_pcm_sw_params(
    _pcm: *mut snd_pcm_t,
    _params: *mut snd_pcm_sw_params_t,
) -> c_int {
    STUB_ERR
}

#[no_mangle]
pub unsafe extern "C" fn snd_pcm_sw_params_set_avail_min(
    _pcm: *mut snd_pcm_t,
    _params: *mut snd_pcm_sw_params_t,
    _val: snd_pcm_uframes_t,
) -> c_int {
    STUB_ERR
}

#[no_mangle]
pub unsafe extern "C" fn snd_pcm_sw_params_set_start_threshold(
    _pcm: *mut snd_pcm_t,
    _params: *mut snd_pcm_sw_params_t,
    _val: snd_pcm_uframes_t,
) -> c_int {
    STUB_ERR
}
