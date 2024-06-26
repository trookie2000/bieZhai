use crate::KEYMAP;
use enigo::{Enigo, MouseButton, MouseControllable};
use enigo::{Key, KeyboardControllable};
use mac_address::MacAddress;
use rand::Rng;
use serde::Serialize;
use tauri::command;
use uuid::Uuid;
use std::net::UdpSocket;

extern crate winapi;
use winapi::shared::windef::HWND;

use winapi::um::winuser::{
    FindWindowW, SetForegroundWindow, SetWindowPos, ShowWindow, HWND_TOP, HWND_TOPMOST, SWP_NOMOVE,
    SWP_NOSIZE, SWP_SHOWWINDOW, SW_RESTORE,FindWindowExA,FindWindowExW
};
use std::os::windows::ffi::OsStrExt;
use std::ffi::OsString;
use std::os::windows::ffi::OsStringExt;
use std::ptr;
use winapi::shared::minwindef::{DWORD, MAX_PATH};
use winapi::shared::windef::RECT;
use winapi::um::winuser::{GetForegroundWindow, GetWindowRect, GetWindowTextW};

#[derive(Serialize)]
pub struct Account {
    id: String,
    password: String,
}
fn find_windows_by_title(window_title: &str) -> Vec<HWND> {
    let mut hwnds = Vec::new();
    let mut hwnd = ptr::null_mut();
    let window_title_wide: Vec<u16> = OsString::from(window_title).encode_wide().collect();

    loop {
        hwnd = unsafe { 
            FindWindowExW(ptr::null_mut(), hwnd, ptr::null(), window_title_wide.as_ptr())
        };
        if hwnd.is_null() {
            break;
        }
        hwnds.push(hwnd);
    }

    hwnds
}
#[command]
pub fn generate_account() -> Account {
    // 生成随机密码
    let mut password = String::new();
    let mut random = rand::thread_rng();
    for _ in 0..2 {
        let num = random.gen_range(0..10);
        password.push_str(&num.to_string());
    }

    // 获取本机 IP 地址
    let id = get_local_ip_address().unwrap_or_else(|_| String::from("Unknown"));

    // 创建并返回 Account 结构体
    Account { id, password }
}

fn get_local_ip_address() -> Result<String, std::io::Error> {
    let socket = UdpSocket::bind("0.0.0.0:0")?;
    socket.connect("8.8.8.8:80")?; // Google 的公共 DNS 服务器
    let local_addr = socket.local_addr()?;
    Ok(local_addr.ip().to_string())
}

// 鼠标事件命令函数
#[command]
pub fn mouse_event(x: i32, y: i32, event_type: &str) {
    let mut enigo = Enigo::new();

    println!("type {}", event_type);
    match event_type {
        "left-mouse-down" => {
            enigo.mouse_down(MouseButton::Left);
        }
        "left-mouse-up" => {
            enigo.mouse_up(MouseButton::Left);
        }
        "right-mouse-down" => {
            enigo.mouse_down(MouseButton::Right);
        }
        "right-mouse-up" => {
            enigo.mouse_down(MouseButton::Right);
        }
        "left-click" => {
            enigo.mouse_click(MouseButton::Left);
        }
        "right-click" => {
            enigo.mouse_click(MouseButton::Right);
        }
        "mouse-move" => {
            enigo.mouse_move_to(x, y);
        }
        "wheel-up" => {
            enigo.mouse_scroll_y(2);
        }
        "wheel-down" => {
            enigo.mouse_scroll_y(-2);
        }
        _ => {}
    }
}

// 键盘事件命令函数
#[command]
pub fn key_event(event_type: &str, key: &str) {
    let mut enigo = Enigo::new();

    let k: Key;

    // println!("type {}", event_type);
    // println!("key {}", key);

    if key.len() > 1 {
        // 根据键名获取键值，如果不存在则打印错误信息
        match KEYMAP.get(key) {
            Some(val) => k = *val,
            None => {
                println!("get key error by map");
                return;
            }
        }
    } else {
        // 如果键名长度为 1，则将其视为单字符键
        let c: Vec<char> = key.chars().collect();
        k = Key::Layout(c[0]);
    }

    match event_type {
        "key-down" => {
            enigo.key_down(k);
        }
        "key-up" => {
            enigo.key_up(k);
        }
        "key-click" => {
            enigo.key_click(k);
        }
        "enter" => {
            enigo.key_down(k);
        }
        _ => {}
    }
}

//将一个指定窗口置于最前面
#[command]
pub fn set_window_topmost(window_title: &str) {
    let hwnds = find_windows_by_title(window_title);
    let hwnds_clone = hwnds.clone(); // 克隆一份副本
    for hwnd in hwnds_clone {
        unsafe {
            ShowWindow(hwnd, SW_RESTORE);
            SetWindowPos(hwnd, HWND_TOP, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE);
            SetForegroundWindow(hwnd);
        }
    }
    if hwnds.is_empty() {
        println!("Window not found");
    } else {
        println!("Show window");
    }
}


#[derive(Serialize)]
pub struct WindowInfo {
    name: String,
    width: i32,
    height: i32,
    left: i32,
    right: i32,
    top: i32,
    bottom: i32,
}

//获取当前处于最前面的窗口的信息
#[command]
pub fn get_top_window_info() -> Option<WindowInfo> {
    unsafe {
        let mut title: Vec<u16> = Vec::with_capacity(MAX_PATH);
        let hWnd = GetForegroundWindow();
        if hWnd.is_null() {
            return None;
        }
        let len = GetWindowTextW(hWnd, title.as_mut_ptr(), MAX_PATH as i32);
        if len == 0 {
            return None;
        }
        title.set_len(len as usize);

        let mut rect: RECT = std::mem::zeroed();
        if GetWindowRect(hWnd, &mut rect as *mut RECT) == 0 {
            return None;
        }
        let width = rect.right - rect.left;
        let height = rect.bottom - rect.top;
        let left = rect.left;
        let right = rect.right;
        let top = rect.top;
        let bottom = rect.bottom;

        let title_string = OsString::from_wide(&title).to_string_lossy().into_owned();

        Some(WindowInfo {
            name: title_string,
            width,
            height,
            left,
            right,
            top,
            bottom,
        })
    }
}
