mod openblt;

use std::ffi::CString;
use std::os::raw::c_void;
use std::ptr;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::Emitter;

/// 验证 FFI 是否打通：返回 LibOpenBLT 版本字符串。
#[tauri::command]
fn version() -> String {
    openblt::version_string()
}

/// 用户主动取消：烧录线程在重试/写入循环里轮询该标志。
// ponytail: 全局标志，一次只跑一个烧录会话，够用；并发会话时再改成 per-session 句柄。
static CANCEL: AtomicBool = AtomicBool::new(false);

#[tauri::command]
fn cancel() {
    CANCEL.store(true, Ordering::SeqCst);
}

/// 扫描 /dev 下的 USB 串口设备（ttyUSB* / ttyACM*），返回绝对路径列表。
#[tauri::command]
fn list_serial_ports() -> Vec<String> {
    let mut ports = Vec::new();
    if let Ok(entries) = std::fs::read_dir("/dev") {
        for entry in entries.flatten() {
            let name = entry.file_name();
            if let Some(name) = name.to_str() {
                if name.starts_with("ttyUSB") || name.starts_with("ttyACM") {
                    ports.push(format!("/dev/{name}"));
                }
            }
        }
    }
    ports.sort();
    ports
}

/// 烧录命令：在后台线程跑完整 LibOpenBLT 流程，进度/日志通过事件回传前端。
/// 无论成功失败都发 `done` 事件，前端据此解除按钮禁用，避免重复点击并发烧录。
#[tauri::command]
fn program(app: tauri::AppHandle, transport: String, port: String, baudrate: u32, file: String) {
    CANCEL.store(false, Ordering::SeqCst);
    std::thread::spawn(move || {
        match run_program(&app, &transport, &port, baudrate, &file) {
            Ok(()) => {
                let _ = app.emit("done", true);
            }
            Err(e) => {
                let _ = app.emit("log", format!("错误: {}", e));
                let _ = app.emit("progress", 0u8);
                let _ = app.emit("done", false);
            }
        }
    });
}

/// 解析固件文件并回传概览信息（段数、总字节、地址范围），不建立会话、不烧录。
/// 前端在选完文件后调用，用于在主界面展示固件信息、填充界面。
#[derive(serde::Serialize)]
struct FirmwareInfo {
    valid: bool,
    error: String,
    segment_count: u32,
    total_bytes: u32,
    start_address: u32,
    end_address: u32,
}

impl FirmwareInfo {
    fn err(msg: &str) -> Self {
        Self {
            valid: false,
            error: msg.to_string(),
            segment_count: 0,
            total_bytes: 0,
            start_address: 0,
            end_address: 0,
        }
    }
}

#[tauri::command]
fn firmware_info(file: String) -> FirmwareInfo {
    let c_file = match CString::new(file) {
        Ok(c) => c,
        Err(_) => return FirmwareInfo::err("文件名包含非法字符(NUL)"),
    };
    unsafe {
        openblt::BltFirmwareInit(openblt::BLT_FIRMWARE_PARSER_SRECORD);
        if openblt::BltFirmwareLoadFromFile(c_file.as_ptr(), 0) != openblt::BLT_RESULT_OK {
            openblt::BltFirmwareTerminate();
            return FirmwareInfo::err("固件文件加载失败（可能不是合法的 S-record）");
        }
        let seg_count = openblt::BltFirmwareGetSegmentCount();
        let mut total: u32 = 0;
        let mut start: u32 = u32::MAX;
        let mut end: u32 = 0;
        for i in 0..seg_count {
            let mut addr: u32 = 0;
            let mut len: u32 = 0;
            let data = openblt::BltFirmwareGetSegment(i, &mut addr, &mut len);
            if !data.is_null() && len > 0 {
                total += len;
                if addr < start {
                    start = addr;
                }
                if addr + len > end {
                    end = addr + len;
                }
            }
        }
        openblt::BltFirmwareTerminate();
        FirmwareInfo {
            valid: true,
            error: String::new(),
            segment_count: seg_count,
            total_bytes: total,
            start_address: if start == u32::MAX { 0 } else { start },
            end_address: end,
        }
    }
}

fn run_program(
    app: &tauri::AppHandle,
    transport: &str,
    port: &str,
    baudrate: u32,
    file: &str,
) -> Result<(), String> {
    // CString 必须活到整个会话结束（BltSessionStart 连接时仍会读取指针）
    let c_port = CString::new(port).map_err(|_| "串口名包含非法字符(NUL)".to_string())?;
    let c_file = CString::new(file).map_err(|_| "文件名包含非法字符(NUL)".to_string())?;

    let session_settings = openblt::BltSessionSettingsXcpV10 {
        timeout_t1: 1000,
        timeout_t3: 2000,
        timeout_t4: 10000,
        // 擦除/写入超时：目标板 flash 擦写偶尔超过 1s，导致间歇性「擦除失败/写入失败」，
        // 加大到 5s 留足余量（如仍超时再上调）。
        timeout_t5: 5000,
        timeout_t6: 5000,
        timeout_t7: 2000,
        seed_key_file: ptr::null(),
        connect_mode: 0, // BLT_CONNECT_MODE_NORMAL
        // 1 = BLT_BYPASS_FIRMWARE_START_DISABLED：烧录完成后启动新固件（0 会让 bootloader 停留在 bootloader）
        bypass_firmware_start: 1,
    };

    // rs232_settings 必须在函数作用域内保持存活（BltSessionInit 取指针）
    let mut rs232_settings = openblt::BltTransportSettingsXcpV10Rs232 {
        port_name: ptr::null(),
        baudrate: 0,
        cs_type: 0,
    };

    let (transport_type, transport_settings_ptr): (u32, *const c_void) = if transport == "usb" {
        // USB 在 openblt.h 中没有设置结构体，传 null
        (openblt::BLT_TRANSPORT_XCP_V10_USB, ptr::null())
    } else {
        rs232_settings.port_name = c_port.as_ptr();
        rs232_settings.baudrate = baudrate;
        rs232_settings.cs_type = 0;
        (
            openblt::BLT_TRANSPORT_XCP_V10_RS232,
            &rs232_settings as *const _ as *const c_void,
        )
    };

    unsafe {
        openblt::BltFirmwareInit(openblt::BLT_FIRMWARE_PARSER_SRECORD);
        if openblt::BltFirmwareLoadFromFile(c_file.as_ptr(), 0) != openblt::BLT_RESULT_OK {
            openblt::BltFirmwareTerminate();
            return Err("固件文件加载失败（可能不是合法的 S-record）".to_string());
        }

        // ---- 自动重试连接 ----
        // 下位机 OpenBLT 复位后只有约 500ms 的 backdoor 窗口等待 CONNECT。
        // 连不上就关闭会话、等一小会儿再重新初始化（重开串口）并重试，
        // 直到命中窗口或达到最大重试次数。这样用户重新上电电路板时，
        // 某一次重试会正好落进窗口从而连上，无需手动反复点“烧录”。
        const RETRY_INTERVAL_MS: u64 = 300;
        const MAX_RETRIES: u32 = 100; // ~30s，给用户足够时间重新上电
        let mut connected = false;
        for attempt in 1..=MAX_RETRIES {
            if CANCEL.load(Ordering::SeqCst) {
                openblt::BltFirmwareTerminate();
                return Err("已取消".to_string());
            }
            let _ = app.emit(
                "log",
                format!("正在连接目标... (第 {}/{} 次)", attempt, MAX_RETRIES),
            );
            openblt::BltSessionInit(
                openblt::BLT_SESSION_XCP_V10,
                &session_settings as *const _ as *const c_void,
                transport_type,
                transport_settings_ptr,
            );
            if openblt::BltSessionStart() == openblt::BLT_RESULT_OK {
                connected = true;
                break;
            }
            openblt::BltSessionTerminate();
            std::thread::sleep(std::time::Duration::from_millis(RETRY_INTERVAL_MS));
        }
        if !connected {
            openblt::BltFirmwareTerminate();
            return Err(format!(
                "连接目标失败（已重试 {} 次，请确认已重新上电或检查串口/固件）",
                MAX_RETRIES
            ));
        }
        let _ = app.emit("log", "已连接，开始烧录");

        // 先算出总字节数，用于进度百分比
        let seg_count = openblt::BltFirmwareGetSegmentCount();
        let mut total: u32 = 0;
        for i in 0..seg_count {
            let mut addr: u32 = 0;
            let mut len: u32 = 0;
            let data = openblt::BltFirmwareGetSegment(i, &mut addr, &mut len);
            if !data.is_null() {
                total += len;
            }
        }

        let mut done: u32 = 0;
        for i in 0..seg_count {
            let mut addr: u32 = 0;
            let mut len: u32 = 0;
            let data = openblt::BltFirmwareGetSegment(i, &mut addr, &mut len);
            if data.is_null() || len == 0 {
                continue;
            }

            if openblt::BltSessionClearMemory(addr, len) != openblt::BLT_RESULT_OK {
                openblt::BltSessionStop();
                openblt::BltSessionTerminate();
                openblt::BltFirmwareTerminate();
                return Err(format!("擦除失败 @ {:#x}", addr));
            }

            // 256 字节分块写入（匹配 XCP 编程块大小）
            let slice = std::slice::from_raw_parts(data, len as usize);
            let mut offset: u32 = 0;
            while offset < len {
                if CANCEL.load(Ordering::SeqCst) {
                    openblt::BltSessionStop();
                    openblt::BltSessionTerminate();
                    openblt::BltFirmwareTerminate();
                    return Err("已取消".to_string());
                }
                let chunk = std::cmp::min(256, len - offset);
                let r = openblt::BltSessionWriteData(
                    addr + offset,
                    chunk,
                    slice[offset as usize..(offset + chunk) as usize].as_ptr(),
                );
                if r != openblt::BLT_RESULT_OK {
                    openblt::BltSessionStop();
                    openblt::BltSessionTerminate();
                    openblt::BltFirmwareTerminate();
                    return Err(format!("写入失败 @ {:#x}", addr + offset));
                }
                offset += chunk;
                done += chunk;
                let pct = if total > 0 {
                    (done * 100 / total) as u8
                } else {
                    100u8
                };
                let _ = app.emit("progress", pct);
            }
            let _ = app.emit("log", format!("段 {}/{} 完成 ({:#x}, {} 字节)", i + 1, seg_count, addr, len));
        }

        openblt::BltSessionStop();
        openblt::BltSessionTerminate();
        openblt::BltFirmwareTerminate();
    }

    let _ = app.emit("log", "烧录完成 ✓");
    let _ = app.emit("progress", 100u8);
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![version, program, cancel, list_serial_ports, firmware_info])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
