const state = {
    mode: "idle",
    view: "share",
    selectedFiles: [],
    shareCode: null,
    pendingReceive: null,
    eventSource: null,
    expireInterval: null,
    expiresAt: null,
};

const elements = {
    btnShare: document.getElementById("btn-share"),
    btnReceive: document.getElementById("btn-receive"),

    sharePanel: document.getElementById("share-panel"),
    dropZone: document.getElementById("drop-zone"),
    fileInput: document.getElementById("file-input"),
    selectedFiles: document.getElementById("selected-files"),
    fileList: document.getElementById("file-list"),
    totalSize: document.getElementById("total-size"),
    btnStartShare: document.getElementById("btn-start-share"),
    btnClearFiles: document.getElementById("btn-clear-files"),
    shareCodeDisplay: document.getElementById("share-code-display"),
    shareCode: document.getElementById("share-code"),
    expireTime: document.getElementById("expire-time"),
    qrCodeContainer: document.getElementById("qr-code-container"),
    qrCode: document.getElementById("qr-code"),
    shareStatus: document.getElementById("share-status"),
    btnCancelShare: document.getElementById("btn-cancel-share"),

    receivePanel: document.getElementById("receive-panel"),
    codeInputSection: document.getElementById("code-input-section"),
    codeInput: document.getElementById("code-input"),
    btnConnect: document.getElementById("btn-connect"),
    incomingTransfer: document.getElementById("incoming-transfer"),
    senderName: document.getElementById("sender-name"),
    incomingFiles: document.getElementById("incoming-files"),
    incomingSize: document.getElementById("incoming-size"),
    btnAccept: document.getElementById("btn-accept"),
    btnDecline: document.getElementById("btn-decline"),

    progressPanel: document.getElementById("progress-panel"),
    progressTitle: document.getElementById("progress-title"),
    progressFile: document.getElementById("progress-file"),
    progressFill: document.getElementById("progress-fill"),
    progressPercent: document.getElementById("progress-percent"),
    progressSpeed: document.getElementById("progress-speed"),
    progressEta: document.getElementById("progress-eta"),

    completePanel: document.getElementById("complete-panel"),
    completeSummary: document.getElementById("complete-summary"),
    btnDownload: document.getElementById("btn-download"),
    btnNewTransfer: document.getElementById("btn-new-transfer"),

    errorPanel: document.getElementById("error-panel"),
    errorMessage: document.getElementById("error-message"),
    btnDismissError: document.getElementById("btn-dismiss-error"),

    deviceName: document.getElementById("device-name"),
};

const api = {
    async getStatus() {
        const res = await fetch("/api/status");
        if (!res.ok) throw new Error("Failed to get status");
        return res.json();
    },

    async getNetwork() {
        const res = await fetch("/api/network");
        if (!res.ok) throw new Error("Failed to get network info");
        return res.json();
    },

    async createShare(files) {
        const formData = new FormData();
        files.forEach((f) => formData.append("files", f));
        const res = await fetch("/api/share", {
            method: "POST",
            body: formData,
        });
        if (!res.ok) {
            const err = await res.json();
            throw new Error(err.message || "Failed to create share");
        }
        return res.json();
    },

    async cancelShare() {
        const res = await fetch("/api/share", { method: "DELETE" });
        if (!res.ok) throw new Error("Failed to cancel share");
    },

    async startReceive(code) {
        const res = await fetch("/api/receive", {
            method: "POST",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify({ code: code.toUpperCase() }),
        });
        if (!res.ok) {
            const err = await res.json();
            throw new Error(err.message || "Failed to connect");
        }
        return res.json();
    },

    async acceptReceive() {
        const res = await fetch("/api/receive/accept", { method: "POST" });
        if (!res.ok) {
            const err = await res.json();
            throw new Error(err.message || "Failed to accept");
        }
        return res.json();
    },

    async declineReceive() {
        await fetch("/api/receive/decline", { method: "POST" });
    },
};

function formatBytes(bytes) {
    if (bytes === 0) return "0 B";
    const k = 1024;
    const sizes = ["B", "KB", "MB", "GB", "TB"];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(1)) + " " + sizes[i];
}

function formatDuration(seconds) {
    if (seconds === null || seconds === undefined) return "--";
    if (seconds < 60) return `${Math.round(seconds)}s`;
    if (seconds < 3600)
        return `${Math.floor(seconds / 60)}m ${Math.round(seconds % 60)}s`;
    return `${Math.floor(seconds / 3600)}h ${Math.floor(
        (seconds % 3600) / 60
    )}m`;
}

function formatSpeed(bytesPerSec) {
    return formatBytes(bytesPerSec) + "/s";
}

function showPanel(panelId) {
    [
        "share-panel",
        "receive-panel",
        "progress-panel",
        "complete-panel",
        "error-panel",
    ].forEach((id) => {
        const panel = document.getElementById(id);
        if (panel) panel.hidden = id !== panelId;
    });
}

function switchView(view) {
    state.view = view;
    elements.btnShare.classList.toggle("active", view === "share");
    elements.btnReceive.classList.toggle("active", view === "receive");
    elements.sharePanel.hidden = view !== "share";
    elements.receivePanel.hidden = view !== "receive";

    if (view === "share") {
        elements.dropZone.hidden = false;
        elements.selectedFiles.hidden = state.selectedFiles.length === 0;
        elements.shareCodeDisplay.hidden = true;
    } else {
        elements.codeInputSection.hidden = false;
        elements.incomingTransfer.hidden = true;
    }
}

function updateFileList() {
    elements.fileList.innerHTML = "";
    let total = 0;

    state.selectedFiles.forEach((file, index) => {
        const li = document.createElement("li");
        li.innerHTML = `
            <span class="file-name">${file.name}</span>
            <span class="file-size">${formatBytes(file.size)}</span>
            <button class="remove-file" data-index="${index}">&times;</button>
        `;
        elements.fileList.appendChild(li);
        total += file.size;
    });

    elements.totalSize.textContent = `Total: ${formatBytes(total)} (${
        state.selectedFiles.length
    } files)`;
    elements.selectedFiles.hidden = state.selectedFiles.length === 0;
    elements.dropZone.hidden = state.selectedFiles.length > 0;
}

function updateExpireTime() {
    if (!state.expiresAt) return;
    const remaining = Math.max(
        0,
        Math.floor((state.expiresAt - Date.now()) / 1000)
    );
    const minutes = Math.floor(remaining / 60);
    const seconds = remaining % 60;
    elements.expireTime.textContent = `${minutes}:${seconds
        .toString()
        .padStart(2, "0")}`;

    if (remaining <= 0) {
        clearInterval(state.expireInterval);
        showError("Share code expired");
        resetShareUI();
    }
}

function resetShareUI() {
    state.mode = "idle";
    state.shareCode = null;
    state.expiresAt = null;
    if (state.expireInterval) {
        clearInterval(state.expireInterval);
        state.expireInterval = null;
    }
    elements.dropZone.hidden = false;
    elements.selectedFiles.hidden = state.selectedFiles.length === 0;
    elements.shareCodeDisplay.hidden = true;
    elements.qrCodeContainer.hidden = true;
    elements.qrCode.innerHTML = "";
}

function resetReceiveUI() {
    state.mode = "idle";
    state.pendingReceive = null;
    elements.codeInputSection.hidden = false;
    elements.incomingTransfer.hidden = true;
    elements.codeInput.value = "";
}

function showError(message) {
    elements.errorMessage.textContent = message;
    showPanel("error-panel");
}

function updateProgressUI(progress) {
    const percent = progress.percentage || 0;
    elements.progressFill.style.width = `${percent}%`;
    elements.progressPercent.textContent = `${Math.round(percent)}%`;
    elements.progressFile.textContent = `${progress.file || ""} (${
        progress.file_index + 1
    }/${progress.total_files})`;
    elements.progressSpeed.textContent = formatSpeed(progress.speed_bps || 0);
    elements.progressEta.textContent = `ETA: ${formatDuration(
        progress.eta_seconds
    )}`;
}

function connectProgressSSE() {
    if (state.eventSource) {
        state.eventSource.close();
    }

    state.eventSource = new EventSource("/api/transfer/progress");

    state.eventSource.onmessage = (event) => {
        try {
            const data = JSON.parse(event.data);
            if (data.type === "progress") {
                updateProgressUI(data);
            } else if (data.type === "complete") {
                showComplete(data);
            } else if (data.type === "error") {
                showError(data.message);
            }
        } catch (e) {
            console.error("SSE parse error:", e);
        }
    };

    state.eventSource.addEventListener("complete", (event) => {
        const data = JSON.parse(event.data);
        showComplete(data);
    });

    state.eventSource.addEventListener("error", (event) => {
        console.log("SSE connection closed");
    });
}

function showComplete(data) {
    state.mode = "idle";
    if (state.eventSource) {
        state.eventSource.close();
        state.eventSource = null;
    }

    const speed =
        data.total_bytes && data.duration_secs
            ? formatSpeed(data.total_bytes / data.duration_secs)
            : "";

    elements.completeSummary.textContent =
        `${data.files || 0} files, ${formatBytes(data.total_bytes || 0)}` +
        (speed ? ` at ${speed}` : "");

    elements.btnDownload.hidden = state.view !== "receive";

    showPanel("complete-panel");
}

function handleFiles(files) {
    state.selectedFiles = [...state.selectedFiles, ...Array.from(files)];
    updateFileList();
}

async function startShare() {
    if (state.selectedFiles.length === 0) return;

    try {
        elements.btnStartShare.disabled = true;
        elements.btnStartShare.textContent = "Creating share...";

        const result = await api.createShare(state.selectedFiles);

        state.mode = "sharing";
        state.shareCode = result.code;
        state.expiresAt = Date.now() + result.expires_in * 1000;

        elements.shareCode.textContent = result.code.split("").join(" ");
        elements.shareStatus.textContent = "Waiting for receiver...";
        elements.dropZone.hidden = true;
        elements.selectedFiles.hidden = true;
        elements.shareCodeDisplay.hidden = false;

        if (result.qr_svg) {
            elements.qrCode.innerHTML = result.qr_svg;
            elements.qrCodeContainer.hidden = false;
        } else {
            elements.qrCodeContainer.hidden = true;
        }

        updateExpireTime();
        state.expireInterval = setInterval(updateExpireTime, 1000);

        connectProgressSSE();
    } catch (error) {
        showError(error.message);
    } finally {
        elements.btnStartShare.disabled = false;
        elements.btnStartShare.textContent = "Share Files";
    }
}

async function cancelShare() {
    try {
        await api.cancelShare();
    } catch (e) {
        console.error("Cancel share error:", e);
    }
    resetShareUI();
}

async function connectToCode() {
    const code = elements.codeInput.value.trim().toUpperCase();
    if (code.length !== 4) {
        showError("Please enter a 4-character code");
        return;
    }

    try {
        elements.btnConnect.disabled = true;
        elements.btnConnect.textContent = "Connecting...";

        const result = await api.startReceive(code);

        state.mode = "receiving";
        state.pendingReceive = result;

        elements.senderName.textContent = result.sender_name;
        elements.incomingFiles.innerHTML = "";
        result.files.forEach((file) => {
            const li = document.createElement("li");
            li.className = "file-item";

            let previewHtml = "";
            if (file.preview) {
                if (
                    file.preview.preview_type === "thumbnail" &&
                    file.preview.data
                ) {
                    previewHtml = `<img class="file-preview-img" src="data:${file.preview.mime_type};base64,${file.preview.data}" alt="Preview">`;
                } else if (
                    file.preview.preview_type === "text" &&
                    file.preview.data
                ) {
                    const snippet = file.preview.data
                        .substring(0, 60)
                        .replace(/\n/g, " ");
                    previewHtml = `<span class="file-preview-text">"${snippet}${
                        file.preview.data.length > 60 ? "..." : ""
                    }"</span>`;
                } else if (
                    file.preview.preview_type === "archive" &&
                    file.preview.file_count
                ) {
                    previewHtml = `<span class="file-preview-meta">(${file.preview.file_count} files)</span>`;
                }
            }

            let dimensionsHtml = "";
            if (file.preview?.dimensions) {
                dimensionsHtml = `<span class="file-dimensions">${file.preview.dimensions[0]}×${file.preview.dimensions[1]}</span>`;
            }

            li.innerHTML = `
                ${previewHtml}
                <div class="file-info">
                    <span class="file-name">${file.name}</span>
                    <span class="file-meta">
                        <span class="file-size">${formatBytes(file.size)}</span>
                        ${dimensionsHtml}
                    </span>
                </div>
            `;
            elements.incomingFiles.appendChild(li);
        });
        elements.incomingSize.textContent = formatBytes(result.total_size);

        elements.codeInputSection.hidden = true;
        elements.incomingTransfer.hidden = false;
    } catch (error) {
        showError(error.message);
    } finally {
        elements.btnConnect.disabled = false;
        elements.btnConnect.textContent = "Connect";
    }
}

async function acceptTransfer() {
    try {
        elements.btnAccept.disabled = true;
        elements.btnDecline.disabled = true;

        await api.acceptReceive();

        state.mode = "transferring";
        elements.progressTitle.textContent = "Receiving...";
        showPanel("progress-panel");

        connectProgressSSE();
    } catch (error) {
        showError(error.message);
        resetReceiveUI();
    }
}

async function declineTransfer() {
    try {
        await api.declineReceive();
    } catch (e) {
        console.error("Decline error:", e);
    }
    resetReceiveUI();
}

function downloadFiles() {
    window.location.href = "/api/receive/download";
}

function newTransfer() {
    state.selectedFiles = [];
    updateFileList();
    resetShareUI();
    resetReceiveUI();
    switchView(state.view);
    showPanel(state.view === "share" ? "share-panel" : "receive-panel");
}

async function init() {
    try {
        const [status, network] = await Promise.all([
            api.getStatus(),
            api.getNetwork(),
        ]);

        elements.deviceName.textContent = network.device_name;

        if (status.mode === "sharing" && status.share_code) {
            state.mode = "sharing";
            state.shareCode = status.share_code;
            elements.shareCode.textContent = status.share_code
                .split("")
                .join(" ");
            elements.dropZone.hidden = true;
            elements.selectedFiles.hidden = true;
            elements.shareCodeDisplay.hidden = false;
            connectProgressSSE();
        } else if (status.mode === "transferring") {
            showPanel("progress-panel");
            connectProgressSSE();
        }
    } catch (error) {
        console.error("Init error:", error);
    }

    elements.btnShare.addEventListener("click", () => switchView("share"));
    elements.btnReceive.addEventListener("click", () => switchView("receive"));

    elements.dropZone.addEventListener("click", () =>
        elements.fileInput.click()
    );
    elements.fileInput.addEventListener("change", (e) =>
        handleFiles(e.target.files)
    );

    elements.dropZone.addEventListener("dragover", (e) => {
        e.preventDefault();
        elements.dropZone.classList.add("dragover");
    });
    elements.dropZone.addEventListener("dragleave", () => {
        elements.dropZone.classList.remove("dragover");
    });
    elements.dropZone.addEventListener("drop", (e) => {
        e.preventDefault();
        elements.dropZone.classList.remove("dragover");
        handleFiles(e.dataTransfer.files);
    });

    elements.fileList.addEventListener("click", (e) => {
        if (e.target.classList.contains("remove-file")) {
            const index = parseInt(e.target.dataset.index, 10);
            state.selectedFiles.splice(index, 1);
            updateFileList();
        }
    });

    elements.btnStartShare.addEventListener("click", startShare);
    elements.btnClearFiles.addEventListener("click", () => {
        state.selectedFiles = [];
        updateFileList();
    });
    elements.btnCancelShare.addEventListener("click", cancelShare);

    elements.btnConnect.addEventListener("click", connectToCode);
    elements.codeInput.addEventListener("keypress", (e) => {
        if (e.key === "Enter") connectToCode();
    });
    elements.codeInput.addEventListener("input", (e) => {
        e.target.value = e.target.value.toUpperCase().replace(/[^A-Z0-9]/g, "");
    });
    elements.btnAccept.addEventListener("click", acceptTransfer);
    elements.btnDecline.addEventListener("click", declineTransfer);

    elements.btnDownload.addEventListener("click", downloadFiles);
    elements.btnNewTransfer.addEventListener("click", newTransfer);
    elements.btnDismissError.addEventListener("click", () => {
        showPanel(state.view === "share" ? "share-panel" : "receive-panel");
    });
}

document.addEventListener("DOMContentLoaded", init);
