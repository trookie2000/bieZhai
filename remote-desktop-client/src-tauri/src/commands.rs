use serde::Serialize;
use tauri::command;
use enigo::{Enigo, MouseButton, MouseControllable};
use enigo::{Key, KeyboardControllable};
use uuid::Uuid;
use mac_address::MacAddress;
use rand::Rng;
use crate::KEYMAP;


#[derive(Serialize)]
pub struct Account {
    id: String,
    password: String,
}


#[command]
pub fn generate_account() -> Account {
    let mac_result = mac_address::get_mac_address();
    let id = if let Ok(Some(mac)) = mac_result {
        mac.to_string()
    } else {
        String::from("Unknown")
    };
    let mac_result = mac_address::get_mac_address();
    let mut password = String::new();

    if let Ok(Some(mac)) = mac_result{
        println!("MAC address: {:?}", mac.to_string());
    }
    let mut random = rand::thread_rng();
    let mut i = 0;
    while i < 2 {
        let num = random.gen_range(0..10);
        password.push_str(&num.to_string());
        i += 1;
    }

    // 创建并返回 Account 结构体
    Account { id, password }
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

    println!("type {}", event_type);
    println!("key {}", key);

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

