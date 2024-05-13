// 针对不是调试断言并且目标操作系统是 Windows 的情况，设置 Windows 子系统
#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
  )]
  
  // 导入标准库和其他依赖
  use std::collections::HashMap;
  use tauri::{utils::config::AppUrl, window::WindowBuilder, WindowUrl};
  use commands::{generate_account, key_event, mouse_event};
  use enigo::Key;
  
  // 导入命令模块
  mod commands; 
  
  // 导入 lazy_static 宏，用于创建静态的 KEYMAP 映射
  #[macro_use]
  extern crate lazy_static;
  
  // 创建静态的 KEYMAP 映射，将字符串键映射到 enigo::Key 枚举
  lazy_static! {
    static ref KEYMAP: HashMap<&'static str, Key> = {
        let mut m = HashMap::new();
        m.insert("Control", Key::Control);
        m.insert("Alt", Key::Alt);
        // 添加更多键映射...
        m.insert("Delete", Key::Delete);
        m.insert("Escape", Key::Escape);
        m.insert("End", Key::End);
        m.insert("Home", Key::Home);
        m.insert("Space", Key::Space);
        m.insert("Enter", Key::Return);
        m.insert("Shift", Key::Shift);
        m.insert("Tab", Key::Tab);
        m.insert("PageUp", Key::PageUp);
        m.insert("PageDown", Key::PageDown);
        m.insert("Backspace", Key::Backspace);
        m.insert("CapsLock", Key::CapsLock);
        m.insert("Meta", Key::Meta);
        m.insert("F1", Key::F1);
        m.insert("F2", Key::F2);
        m.insert("F3", Key::F3);
        m.insert("F4", Key::F4);
        m.insert("F5", Key::F5);
        m.insert("F6", Key::F6);
        m.insert("F7", Key::F7);
        m.insert("F8", Key::F8);
        m.insert("F9", Key::F9);
        m.insert("F10", Key::F10);
        m.insert("F11", Key::F11);
        m.insert("F12", Key::F12);

        m
    };
  }
  
  
  fn main() {
      
      
    // tauri::Builder::default()
    // .invoke_handler(tauri::generate_handler![
    // key_event,
    // mouse_event,
    // generate_account
    // ])
    
    //     .run(tauri::generate_context!())
    //     .expect("error while running tauri application");
  
    // 使用 portpicker 库选择一个未使用的端口
    let port = portpicker::pick_unused_port().expect("failed to find unused port");
  
    // 生成 Tauri 上下文
    let mut context = tauri::generate_context!();
  
    // 构建应用程序的 URL
    let url = format!("http://localhost:{}", port).parse().unwrap();
    let window_url = WindowUrl::External(url);
  
    // 重新配置应用程序，以便在此 URL 上启用 IPC
    context.config_mut().build.dist_dir = AppUrl::Url(window_url.clone());
  
    // 创建 Tauri 应用程序
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            key_event,
            mouse_event,
            generate_account
        ])
        .plugin(tauri_plugin_localhost::Builder::new(port).build())
        .setup(move |app| {
            // 创建主窗口
            WindowBuilder::new(
                app,
                "main".to_string(),
                if cfg!(dev) {
                    Default::default()
                } else {
                    window_url
                }
            )
            .title("多端远控")
            .build()?;
            Ok(())
        })
        .run(context)
        .expect("error while running tauri application");
  }
  