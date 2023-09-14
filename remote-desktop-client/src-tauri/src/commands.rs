// 导入必要的库和模块
use serde::Serialize;
use tauri::command;

// 导入 Enigo 库中的鼠标和键盘控制相关内容
use enigo::{Enigo, MouseButton, MouseControllable};
use enigo::{Key, KeyboardControllable};
use uuid::Uuid;

// 导入随机数生成库
use rand::Rng;

// 导入包含键映射的 KEYMAP
use crate::KEYMAP;

// 定义一个序列化结构体 Account，用于生成账号信息
#[derive(Serialize)]
pub struct Account {
    id: String,
    password: String,
}

// 生成账号信息的命令函数
#[command]
pub fn generate_account() -> Account {
    // 生成唯一标识符 UUID
    let uuid = Uuid::new_v4();
    let id = uuid.to_string().replace("-", "");

    let mut password = String::new();

    let mut random = rand::thread_rng();

    let mut i = 0;
    while i < 8 {
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
        _ => {}
    }
}
