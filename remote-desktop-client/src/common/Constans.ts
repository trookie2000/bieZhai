
enum MouseStatus {
    MOUSE_DOWN = "mouse-down",     // 鼠标按下
    MOUSE_UP = "mouse-up",         // 鼠标松开
    MOUSE_MOVE = "mouse-move",     // 鼠标移动
    RIGHT_CLICK = "right-click",   // 鼠标右键单击
}

enum WheelStatus {
    WHEEL_UP = "wheel-up",         // 滚轮向上滚动
    WHEEL_DOWN = "wheel-down",     // 滚轮向下滚动
}

enum KeyboardStatus {
    MOUSE_DOWN = "key-down",       // 键盘按键按下
    MOUSE_UP = "key-up",           // 键盘按键松开
}

enum MessageType {
    VIDEO_OFFER = "video-offer",                 // 视频通话邀请
    VIDEO_ANSWER = "video-answer",               // 视频通话回应
    NEW_ICE_CANDIDATE = "new-ice-candidate",     // 新的 ICE 候选项
    REMOTE_DESKTOP = "remote-desktop",           // 远程桌面请求
    CLOSE_REMOTE_DESKTOP = "close-remote-desktop", // 关闭远程桌面
    STOP_SHARING = "stop-sharing",
    WINDOW_INFO = "window-info"
}

enum InputEventType {
    MOUSE_EVENT = "mouse-event",   // 鼠标事件
    KEY_EVENT = "key-event",       // 键盘事件
    WINDOW_EVENT = "window-event"
}

enum Command {
    MOUSE_EVENT = "mouse_event",   // 鼠标事件命令
    KEY_EVENT = "key_event",       // 键盘事件命令
    WINDOW_EVENT = "set_window_topmost",//通过窗口标题将指定窗口置于最前面，并将其激活
    TOP_WINDOW_EVENT = "get_top_window_info"//获取当前处于最前面的窗口的信息
}

// 导出枚举和常量，使其在其他地方可以使用
export { MouseStatus, WheelStatus, KeyboardStatus, MessageType, InputEventType, Command }
