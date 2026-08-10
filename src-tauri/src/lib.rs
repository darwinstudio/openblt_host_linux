mod openblt;

use std::ffi::CString;
use std::os::raw::c_void;
use std::ptr;
use tauri::Emitter;

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

/// 验证 FFI 是否打通：返回 LibOpenBLT 版本字符串。
#[tauri::command]
fn version() -> String {
    openblt::version_string()
}

/// 烧录命令：在后台线程跑完整 LibOpenBLT 流程，进度/日志通过事件回传前端。
#[tauri::command]
fn program(app: tauri::AppHandle, transport: String, port: String, baudrate: u32, file: String) {
    std::thread::spawn(move || {
        if let Err(e) = run_program(&app, &transport, &port, baudrate, &file) {
            let _ = app.emit("log", format!("错误: {}", e));
            let _ = app.emit("progress", 0u8);
        }
    });
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
        timeout_t5: 1000,
        timeout_t6: 1000,
        timeout_t7: 2000,
        seed_key_file: ptr::null(),
        connect_mode: 0,
        bypass_firmware_start: 0,
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

        openblt::BltSessionInit(
            openblt::BLT_SESSION_XCP_V10,
            &session_settings as *const _ as *const c_void,
            transport_type,
            transport_settings_ptr,
        );

        let _ = app.emit("log", "正在连接目标...");
        if openblt::BltSessionStart() != openblt::BLT_RESULT_OK {
            openblt::BltSessionTerminate();
            openblt::BltFirmwareTerminate();
            return Err("连接目标失败".to_string());
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
        .invoke_handler(tauri::generate_handler![greet, version, program])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
