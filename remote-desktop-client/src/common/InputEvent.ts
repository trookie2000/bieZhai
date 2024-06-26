
import { invoke } from "@tauri-apps/api/tauri";

import { Command } from "./Constans";


// 执行与鼠标事件相关的命令
const handleMouseEvent = async (data: Record<string, any>) => {
    await invoke(Command.MOUSE_EVENT, data);
};


// 执行与键盘事件相关的命令
const handleKeyboardEvent = async (data: Record<string, any>) => {
    await invoke(Command.KEY_EVENT, data);
};

// 获取当前处于最前面的窗口的信息
const handleWindowTop = async (data: Record<string, any>) => {
    await invoke(Command.WINDOW_EVENT, data);
};

//
const handleGetTopWindowInfo = async () => {
    return await invoke(Command.TOP_WINDOW_EVENT);
};


export { handleMouseEvent, handleKeyboardEvent, handleWindowTop, handleGetTopWindowInfo };
