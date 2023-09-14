
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

// 导出处理鼠标事件和键盘事件的函数，以便其他模块可以使用它们
export { handleMouseEvent, handleKeyboardEvent };
