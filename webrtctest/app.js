const programList = document.getElementById("programList");
const startShareButton = document.getElementById("startShareButton");
const stopShareButton = document.getElementById("stopShareButton");
const sharedScreen = document.getElementById("sharedScreen");

let selectedPrograms = [];

// 监听Shift键的按下和释放事件
document.addEventListener("keydown", (e) => {
    if (e.key === "Shift") {
        programList.addEventListener("click", handleProgramSelection);
    }
});

document.addEventListener("keyup", (e) => {
    if (e.key === "Shift") {
        programList.removeEventListener("click", handleProgramSelection);
    }
});

function handleProgramSelection(e) {
    if (e.target.type === "checkbox") {
        const programName = e.target.value;
        if (e.target.checked && !selectedPrograms.includes(programName)) {
            selectedPrograms.push(programName);
        } else if (!e.target.checked && selectedPrograms.includes(programName)) {
            selectedPrograms = selectedPrograms.filter((name) => name !== programName);
        }
    }
}

// WebRTC屏幕共享示例
let screenStream = null;

startShareButton.addEventListener("click", () => {
    if (selectedPrograms.length > 0) {
        navigator.mediaDevices.getDisplayMedia({ video: true }).then((stream) => {
            sharedScreen.srcObject = stream;
            screenStream = stream;
        });
    }
});

stopShareButton.addEventListener("click", () => {
    if (screenStream) {
        screenStream.getTracks().forEach((track) => track.stop());
        sharedScreen.srcObject = null;
    }
});
