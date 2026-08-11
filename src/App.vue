<script setup lang="ts">
import { ref, computed, onMounted, watch, nextTick } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import { openUrl } from "@tauri-apps/plugin-opener";
import {
  NConfigProvider,
  NLayout,
  NLayoutHeader,
  NLayoutContent,
  NLayoutFooter,
  NButton,
  NProgress,
  NInput,
  NDrawer,
  NDrawerContent,
  NForm,
  NFormItem,
  NSelect,
  NInputNumber,
  NSpace,
  NCard,
  NDescriptions,
  NDescriptionsItem,
  NAlert,
  NLog,
} from "naive-ui";

// ---- 设置持久化（设置界面参数存到 localStorage，避免每次打开重填）----
const SETTINGS_KEY = "openblt.settings";

type Transport = "rs232" | "usb";
const transport = ref<Transport>("rs232");
const rs232Port = ref("/dev/ttyUSB0");
const rs232Baud = ref(115200);

function saveSettings() {
  localStorage.setItem(
    SETTINGS_KEY,
    JSON.stringify({
      transport: transport.value,
      rs232Port: rs232Port.value,
      rs232Baud: rs232Baud.value,
    })
  );
}
function loadSettings() {
  const raw = localStorage.getItem(SETTINGS_KEY);
  if (!raw) return;
  try {
    const s = JSON.parse(raw);
    if (s.transport === "rs232" || s.transport === "usb") transport.value = s.transport;
    if (typeof s.rs232Port === "string") rs232Port.value = s.rs232Port;
    if (typeof s.rs232Baud === "number") rs232Baud.value = s.rs232Baud;
  } catch {
    /* 忽略损坏的配置 */
  }
}

const transportOptions = [
  { label: "RS232 串口", value: "rs232" },
  { label: "USB", value: "usb" },
];
const usbNote = "USB 使用固定 VID/PID 0x1D50/0x60AC，无需额外设置";

// 主界面顶部展示当前配置概览，避免每次进设置查看
const configSummary = computed(() => {
  if (transport.value === "usb") return "USB（VID/PID 0x1D50/0x60AC）";
  return `RS232 | ${rs232Port.value} | ${rs232Baud.value} bps`;
});

// ---- 库版本（验证 FFI 打通）----
const libVersion = ref("");
async function loadVersion() {
  libVersion.value = await invoke("version");
}

// ---- 后端事件监听（进度 / 日志）----
const logLines = ref<string[]>([]);
const logText = computed(() => logLines.value.join("\n"));
function log(msg: string) {
  logLines.value.push(msg);
}
function clearLog() {
  logLines.value = [];
}

// 自动滚动日志到底部，避免手动下拉
const logRef = ref<{ scrollToBottom: (silent?: boolean) => void } | null>(null);
watch(logText, () => {
  nextTick(() => logRef.value?.scrollToBottom());
});

onMounted(async () => {
  loadSettings();
  await loadVersion();
  await listen<number>("progress", (e) => {
    progress.value = e.payload;
  });
  await listen<string>("log", (e) => {
    log(e.payload);
  });
  // 后端烧录结束（成功/失败）后解除按钮禁用，避免重复点击并发烧录
  await listen<boolean>("done", () => {
    programming.value = false;
  });
});

// ---- 选文件（调用 Tauri dialog 插件）----
interface FirmwareInfo {
  valid: boolean;
  error: string;
  segment_count: number;
  total_bytes: number;
  start_address: number;
  end_address: number;
}
const firmwareInfo = ref<FirmwareInfo | null>(null);

const firmwarePath = ref("");
async function pickFile() {
  const selected = await open({
    filters: [{ name: "Motorola S-record", extensions: ["s19", "s28", "s37", "srec", "mot"] }],
  });
  if (typeof selected === "string") {
    firmwarePath.value = selected;
    log(`已选择固件: ${selected}`);
    // 解析固件概览，填充主界面固件信息面板
    try {
      firmwareInfo.value = await invoke<FirmwareInfo>("firmware_info", { file: selected });
    } catch (e) {
      firmwareInfo.value = { valid: false, error: String(e), segment_count: 0, total_bytes: 0, start_address: 0, end_address: 0 };
    }
  }
}

// ---- 进度 / 烧录中标志 ----
const progress = ref(0);
const programming = ref(false);

// ---- 设置界面开关 ----
const showSettings = ref(false);

// ---- 底部 GitHub 图标：打开仓库链接 ----
async function openRepo() {
  try {
    await openUrl("https://github.com/darwinstudio/openblt_host_linux");
  } catch (e) {
    log(`打开仓库链接失败: ${e}`);
  }
}

// ---- 烧录（调用后端 program command，进度/日志由事件回传）----
async function program() {
  if (programming.value) return;
  if (!firmwarePath.value) {
    log("请先选择固件文件");
    return;
  }
  programming.value = true;
  progress.value = 0;
  logLines.value = [];
  log(`开始烧录（通道=${transport.value}）...`);
  await invoke("program", {
    transport: transport.value,
    port: rs232Port.value,
    baudrate: rs232Baud.value,
    file: firmwarePath.value,
  });
}
</script>

<template>
  <n-config-provider>
    <n-layout style="height: 100vh">
      <n-layout-header
        bordered
        style="padding: 12px 24px; display: flex; align-items: baseline; gap: 16px"
      >
        <h2 style="margin: 0">OpenBLT 烧录工具</h2>
        <span>LibOpenBLT 版本：{{ libVersion }}</span>
        <n-button style="margin-left: auto" @click="showSettings = true">设置</n-button>
      </n-layout-header>

      <n-layout-content content-style="padding: 16px" style="overflow: hidden">
        <n-space vertical :size="8">
          <!-- 控制区：选固件 + 烧录 -->
          <n-space align="center">
            <n-input
              v-model:value="firmwarePath"
              placeholder="未选择固件"
              readonly
              style="width: 360px"
            />
            <n-button @click="pickFile">选择固件</n-button>
            <n-button
              type="primary"
              :disabled="programming"
              :loading="programming"
              @click="program"
            >
              烧录
            </n-button>
          </n-space>

          <!-- 当前配置概览 -->
          <n-space align="center">
            <span style="color: var(--n-text-color-3, #888)">当前配置：</span>
            <span>{{ configSummary }}</span>
          </n-space>

          <!-- 固件信息面板（选完文件后由后端解析填充）；固定最小高度，选/不选文件时布局不跳动 -->
          <n-card title="固件信息" size="small" style="min-height: 118px">
            <n-descriptions
              v-if="firmwareInfo && firmwareInfo.valid"
              :column="2"
              bordered
              size="small"
              label-placement="left"
            >
              <n-descriptions-item label="段数">{{ firmwareInfo.segment_count }}</n-descriptions-item>
              <n-descriptions-item label="总大小"
                >{{ firmwareInfo.total_bytes }} 字节</n-descriptions-item
              >
              <n-descriptions-item label="起始地址"
                >0x{{ firmwareInfo.start_address.toString(16).toUpperCase() }}</n-descriptions-item
              >
              <n-descriptions-item label="结束地址"
                >0x{{ firmwareInfo.end_address.toString(16).toUpperCase() }}</n-descriptions-item
              >
            </n-descriptions>
            <n-alert v-else-if="firmwareInfo" type="error" :show-icon="true">{{
              firmwareInfo.error
            }}</n-alert>
            <span v-else style="color: var(--n-text-color-3, #888)">尚未选择固件文件</span>
          </n-card>

          <!-- 进度条单独一行 -->
          <n-progress type="line" :percentage="progress" :height="18" />

          <!-- 日志区域往下排，空时显示占位提示 -->
          <n-card title="日志" size="small">
            <template #header-extra>
              <n-button size="small" @click="clearLog">清除</n-button>
            </template>
            <n-log v-if="logLines.length" ref="logRef" :log="logText" style="height: 48px" />
            <span v-else style="color: var(--n-text-color-3, #888)">等待操作，日志将在此处显示…</span>
          </n-card>
        </n-space>
      </n-layout-content>

      <n-layout-footer
        bordered
        style="
          padding: 6px 24px;
          display: flex;
          align-items: center;
          gap: 12px;
          font-size: 13px;
          color: var(--n-text-color-3, #888);
        "
      >
        <span>版本 v0.2.0</span>
        <span>|</span>
        <span>作者：shenzan &amp; CodeBuddy</span>
        <span
          style="margin-left: auto; display: inline-flex; align-items: center; cursor: pointer"
          title="在 GitHub 上查看"
          @click="openRepo"
        >
          <svg viewBox="0 0 16 16" width="18" height="18" fill="currentColor">
            <path
              d="M8 0C3.58 0 0 3.58 0 8c0 3.54 2.29 6.53 5.47 7.59.4.07.55-.17.55-.38 0-.19-.01-.82-.01-1.49-2.01.37-2.53-.49-2.69-.94-.09-.23-.48-.94-.82-1.13-.28-.15-.68-.52-.01-.53.63-.01 1.08.58 1.23.82.72 1.21 1.87.87 2.33.66.07-.52.28-.87.51-1.07-1.78-.2-3.64-.89-3.64-3.95 0-.87.31-1.59.82-2.15-.08-.2-.36-1.02.08-2.12 0 0 .67-.21 2.2.82.64-.18 1.32-.27 2-.27.68 0 1.36.09 2 .27 1.53-1.04 2.2-.82 2.2-.82.44 1.1.16 1.92.08 2.12.51.56.82 1.27.82 2.15 0 3.07-1.87 3.75-3.65 3.95.29.25.54.73.54 1.48 0 1.07-.01 1.93-.01 2.2 0 .21.15.46.55.38A8.013 8.013 0 0 0 16 8c0-4.42-3.58-8-8-8z"
            />
          </svg>
        </span>
      </n-layout-footer>
    </n-layout>

    <!-- 设置二级界面：通道 / 串口设备 / 波特率 -->
    <n-drawer v-model:show="showSettings" :width="380" placement="right" title="设置">
      <n-drawer-content title="通道与串口设置" :native-scrollbar="false">
        <n-form label-placement="left" :label-width="90">
          <n-form-item label="通道">
            <n-select
              v-model:value="transport"
              :options="transportOptions"
              style="width: 100%"
            />
          </n-form-item>

          <template v-if="transport === 'rs232'">
            <n-form-item label="串口设备">
              <n-input v-model:value="rs232Port" placeholder="/dev/ttyUSB0" style="width: 100%" />
            </n-form-item>
            <n-form-item label="波特率">
              <n-input-number v-model:value="rs232Baud" :min="1" style="width: 100%" />
            </n-form-item>
          </template>

          <n-form-item v-else label="USB">
            <span>{{ usbNote }}</span>
          </n-form-item>
        </n-form>

        <template #footer>
          <n-space justify="end">
            <n-button @click="showSettings = false">取消</n-button>
            <n-button
              type="primary"
              @click="
                () => {
                  saveSettings();
                  showSettings = false;
                }
              "
            >
              保存
            </n-button>
          </n-space>
        </template>
      </n-drawer-content>
    </n-drawer>
  </n-config-provider>
</template>
