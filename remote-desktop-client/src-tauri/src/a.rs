use rusty_duplication::{FrameInfoExt, Scanner, VecCapturer};
use std::{fs::File, io::Write, thread, time::Duration};

// 创建 Scanner 来枚举系统中的所有显示器（监视器）
let mut scanner = Scanner::new().unwrap();

// scanner 实现了 Iterator，所以可以通过 next() 获取下一个监视器
let monitor = scanner.next().unwrap();

// 获取监视器的详细信息
monitor.dxgi_output_desc().unwrap();
monitor.dxgi_outdupl_desc();

// 创建一个 VecCapturer 来捕获屏幕图像
// 这个过程会分配一块内存，用于存储屏幕像素数据
let mut capturer: VecCapturer = monitor.try_into().unwrap();

// 也可以通过 capturer 来获取监视器信息
let dxgi_outdupl_desc = capturer.monitor().dxgi_outdupl_desc();
let dxgi_output_desc = capturer.monitor().dxgi_output_desc().unwrap();

// 打印分辨率（宽度和高度）
println!(
  "size: {}x{}",
  dxgi_outdupl_desc.ModeDesc.Width, dxgi_outdupl_desc.ModeDesc.Height
);

// 打印监视器在桌面中的坐标区域
println!(
  "left: {}, top: {}, right: {}, bottom: {}",
  dxgi_output_desc.DesktopCoordinates.left,
  dxgi_output_desc.DesktopCoordinates.top,
  dxgi_output_desc.DesktopCoordinates.right,
  dxgi_output_desc.DesktopCoordinates.bottom
);

// 先等待一小段时间，让系统有机会更新屏幕，然后再进行捕获
thread::sleep(Duration::from_millis(100));

// 捕获当前桌面画面，并获取此次捕获的帧信息
let info = capturer.capture().unwrap();

// 通过 FrameInfoExt，我们可以查看这次捕获中哪些信息被更新
if info.desktop_updated() {
  println!("桌面画面已更新！");
}
if info.mouse_updated() {
  println!("鼠标已更新！");
}
if info.pointer_shape_updated() {
  println!("指针形状已更新！");
}

// 将捕获到的图像数据写入文件
// capturer.buffer 中的像素数据使用 BGRA32 格式
let mut file = File::create("capture.bin").unwrap();
file.write_all(&capturer.buffer).unwrap();
