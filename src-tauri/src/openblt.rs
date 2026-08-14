// FFI bindings for libopenblt (OpenBLT host library).
// Field layouts and signatures are copied 1:1 from Source/LibOpenBLT/openblt.h.
// uint32_t -> u32, uint16_t -> u16, uint8_t -> u8, char const* -> *const c_char,
// void const* -> *const c_void.

use std::ffi::c_void;
use std::os::raw::c_char;

// ---------------------------------------------------------------------------
// Result / constant values (mirror of openblt.h macros)
// ---------------------------------------------------------------------------
pub const BLT_RESULT_OK: u32 = 0;

pub const BLT_SESSION_XCP_V10: u32 = 0;

pub const BLT_TRANSPORT_XCP_V10_RS232: u32 = 0;
pub const BLT_TRANSPORT_XCP_V10_USB: u32 = 2;

pub const BLT_FIRMWARE_PARSER_SRECORD: u32 = 0;

// ---------------------------------------------------------------------------
// Settings structures (must be #[repr(C)] and match openblt.h field-for-field)
// ---------------------------------------------------------------------------

/// XCP v1.0 session settings (tBltSessionSettingsXcpV10).
#[repr(C)]
pub struct BltSessionSettingsXcpV10 {
    pub timeout_t1: u16,
    pub timeout_t3: u16,
    pub timeout_t4: u16,
    pub timeout_t5: u16,
    pub timeout_t6: u16,
    pub timeout_t7: u16,
    /// Seed/key algorithm library filename. Pass a CString that stays alive for the
    /// whole session. Set to std::ptr::null() if no seed/key protection is used.
    pub seed_key_file: *const c_char,
    pub connect_mode: u8,
    pub bypass_firmware_start: u8,
}

/// XCP v1.0 RS232 transport settings (tBltTransportSettingsXcpV10Rs232).
#[repr(C)]
pub struct BltTransportSettingsXcpV10Rs232 {
    /// e.g. "/dev/ttyUSB0". CString must outlive the session.
    pub port_name: *const c_char,
    pub baudrate: u32,
    pub cs_type: u8, // 0=none, 1=byte checksum
}

// NOTE: openblt.h defines NO settings struct for the USB transport (xcp_usb).
// It uses a fixed VID/PID (0x1D50 / 0x60AC). When calling BltSessionInit with
// BLT_TRANSPORT_XCP_V10_USB, pass std::ptr::null() as transportSettings.

// ---------------------------------------------------------------------------
// Raw C function declarations (unsafe to call)
// ---------------------------------------------------------------------------
extern "C" {
    pub fn BltVersionGetString() -> *const c_char;

    pub fn BltSessionInit(
        session_type: u32,
        session_settings: *const c_void,
        transport_type: u32,
        transport_settings: *const c_void,
    );
    pub fn BltSessionTerminate();
    pub fn BltSessionStart() -> u32;
    pub fn BltSessionStop();
    pub fn BltSessionClearMemory(address: u32, len: u32) -> u32;
    pub fn BltSessionWriteData(address: u32, len: u32, data: *const u8) -> u32;

    pub fn BltFirmwareInit(parser_type: u32);
    pub fn BltFirmwareTerminate();
    pub fn BltFirmwareLoadFromFile(firmware_file: *const c_char, address_offset: u32) -> u32;
    pub fn BltFirmwareGetSegmentCount() -> u32;
    pub fn BltFirmwareGetSegment(idx: u32, address: *mut u32, len: *mut u32) -> *mut u8;
}

// ---------------------------------------------------------------------------
// Small safe helpers (optional conveniences)
// ---------------------------------------------------------------------------

/// Returns the library version as a Rust String.
/// Safe because BltVersionGetString returns a static C string owned by the library.
pub fn version_string() -> String {
    unsafe {
        let ptr = BltVersionGetString();
        if ptr.is_null() {
            return String::new();
        }
        std::ffi::CStr::from_ptr(ptr).to_string_lossy().into_owned()
    }
}
