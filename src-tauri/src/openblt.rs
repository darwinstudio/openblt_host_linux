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
pub const BLT_RESULT_ERROR_GENERIC: u32 = 1;
pub const BLT_RESULT_ERROR_SESSION_INFO_TABLE_NOT_SUPPORTED: u32 = 33;
pub const BLT_RESULT_ERROR_SESSION_INFO_TABLE: u32 = 34;

pub const BLT_SESSION_XCP_V10: u32 = 0;

pub const BLT_TRANSPORT_XCP_V10_RS232: u32 = 0;
pub const BLT_TRANSPORT_XCP_V10_CAN: u32 = 1;
pub const BLT_TRANSPORT_XCP_V10_USB: u32 = 2;
pub const BLT_TRANSPORT_XCP_V10_NET: u32 = 3;
pub const BLT_TRANSPORT_XCP_V10_MBRTU: u32 = 4;

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

/// XCP v1.0 CAN transport settings (tBltTransportSettingsXcpV10Can).
#[repr(C)]
pub struct BltTransportSettingsXcpV10Can {
    /// e.g. "can0" (Linux) or "peak_pcanusb" (Windows). CString must outlive session.
    pub device_name: *const c_char,
    pub device_channel: u32,
    pub baudrate: u32,
    pub transmit_id: u32,
    pub receive_id: u32,
    pub use_extended: u32,
    pub brs_baudrate: u32, // 0 = CAN classic; >0 enables CAN FD (+ bitrate switch)
}

/// XCP v1.0 NET (TCP/IP) transport settings (tBltTransportSettingsXcpV10Net).
#[repr(C)]
pub struct BltTransportSettingsXcpV10Net {
    /// IP or hostname, e.g. "192.168.178.23". CString must outlive session.
    pub address: *const c_char,
    pub port: u16,
}

/// XCP v1.0 Modbus RTU transport settings (tBltTransportSettingsXcpV10MbRtu).
#[repr(C)]
pub struct BltTransportSettingsXcpV10MbRtu {
    pub port_name: *const c_char,
    pub baudrate: u32,
    pub parity: u8,    // 0 none, 1 odd, 2 even
    pub stopbits: u8,  // 1 or 2
    pub destination_addr: u8,
}

// NOTE: openblt.h defines NO settings struct for the USB transport (xcp_usb).
// It uses a fixed VID/PID (0x1D50 / 0x60AC). When calling BltSessionInit with
// BLT_TRANSPORT_XCP_V10_USB, pass std::ptr::null() as transportSettings.

// ---------------------------------------------------------------------------
// Raw C function declarations (unsafe to call)
// ---------------------------------------------------------------------------
extern "C" {
    pub fn BltVersionGetNumber() -> u32;
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
    pub fn BltSessionReadData(address: u32, len: u32, data: *mut u8) -> u32;
    pub fn BltSessionCheckInfoTable() -> u32;

    pub fn BltFirmwareInit(parser_type: u32);
    pub fn BltFirmwareTerminate();
    pub fn BltFirmwareLoadFromFile(firmware_file: *const c_char, address_offset: u32) -> u32;
    pub fn BltFirmwareSaveToFile(firmware_file: *const c_char) -> u32;
    pub fn BltFirmwareGetSegmentCount() -> u32;
    pub fn BltFirmwareGetSegment(idx: u32, address: *mut u32, len: *mut u32) -> *mut u8;
    pub fn BltFirmwareAddData(address: u32, len: u32, data: *const u8) -> u32;
    pub fn BltFirmwareRemoveData(address: u32, len: u32) -> u32;
    pub fn BltFirmwareClearData();

    pub fn BltUtilCrc16Calculate(data: *const u8, len: u32) -> u16;
    pub fn BltUtilCrc32Calculate(data: *const u8, len: u32) -> u32;
    pub fn BltUtilTimeGetSystemTime() -> u32;
    pub fn BltUtilTimeDelayMs(delay: u16);
    pub fn BltUtilCryptoAes256Encrypt(data: *mut u8, len: u32, key: *const u8) -> u32;
    pub fn BltUtilCryptoAes256Decrypt(data: *mut u8, len: u32, key: *const u8) -> u32;
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

/// Returns the library version number (e.g. 0x010200 for v1.2.0).
pub fn version_number() -> u32 {
    unsafe { BltVersionGetNumber() }
}
