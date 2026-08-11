<script setup lang="ts">
import { ref, computed, onMounted, watch, nextTick } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import {
  NConfigProvider,
  NLayout,
  NLayoutHeader,
  NLayoutContent,
  NCard,
  NForm,
  NFormItem,
  NSelect,
  NInput,
  NInputNumber,
  NButton,
  NProgress,
  NLog,
  NSpace,
} from "naive-ui";

// ---- 库版本（验证 FFI 打通）----
const libVersion = ref("");
async function loadVersion() {
  libVersion.value = await invoke("version");
}

// ---- 后端事件监听（进度 / 日志）----
onMounted(async () => {
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

// ---- 设置状态 ----
type Transport = "rs232" | "usb";
const transport = ref<Transport>("rs232");
const rs232Port = ref("/dev/ttyUSB0");
const rs232Baud = ref(115200);
const usbNote = "USB 使用固定 VID/PID 0x1D50/0x60AC，无需额外设置";
const firmwarePath = ref("");

const transportOptions = [
  { label: "RS232 串口", value: "rs232" },
  { label: "USB", value: "usb" },
];

// ---- 日志 ----
const logLines = ref<string[]>([]);
function log(msg: string) {
  logLines.value.push(msg);
}
const logText = computed(() => logLines.value.join("\n"));
function clearLog() {
  logLines.value = [];
}

// 自动滚动日志到底部，避免手动下拉查看
const logRef = ref<{ scrollToBottom: (silent?: boolean) => void } | null>(null);
watch(logText, () => {
  nextTick(() => logRef.value?.scrollToBottom());
});

// ---- 选文件（调用 Tauri dialog 插件）----
async function pickFile() {
  const selected = await open({
    filters: [{ name: "Motorola S-record", extensions: ["s19", "s28", "s37", "srec", "mot"] }],
  });
  if (typeof selected === "string") {
    firmwarePath.value = selected;
    log(`已选择固件: ${selected}`);
  }
}

// ---- 进度 ----
const progress = ref(0);

// ---- 烧录中标志（防止按钮被反复点击产生并发烧录）----
const programming = ref(false);

// ---- 烧录（调用后端 program command，进度/日志由事件回传）----
async function program() {
  if (programming.value) return;
  if (!firmwarePath.value) {
    log("请先选择固件文件");
    return;
  }
  programming.value = true;
  progress.value = 0;
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
      <n-layout-header bordered style="padding: 12px 24px; display: flex; align-items: baseline; gap: 16px">
        <h2 style="margin: 0">OpenBLT 烧录工具</h2>
        <span>LibOpenBLT 版本：{{ libVersion }}</span>
      </n-layout-header>

      <n-layout-content content-style="padding: 24px">
        <n-space vertical :size="16">
          <n-card title="设置">
            <n-form label-placement="left" :label-width="100">
              <n-form-item label="通道">
                <n-select v-model:value="transport" :options="transportOptions" style="width: 240px" />
              </n-form-item>

              <template v-if="transport === 'rs232'">
                <n-form-item label="串口设备">
                  <n-input v-model:value="rs232Port" placeholder="/dev/ttyUSB0" style="width: 240px" />
                </n-form-item>
                <n-form-item label="波特率">
                  <n-input-number v-model:value="rs232Baud" :min="1" style="width: 240px" />
                </n-form-item>
              </template>

              <n-form-item v-else label="USB">
                <span>{{ usbNote }}</span>
              </n-form-item>

              <n-form-item label="固件文件">
                <n-space>
                  <n-input v-model:value="firmwarePath" placeholder="未选择" style="width: 320px" readonly />
                  <n-button @click="pickFile">选择</n-button>
                </n-space>
              </n-form-item>
            </n-form>

            <n-space>
              <n-button type="primary" :disabled="programming" :loading="programming" @click="program">烧录</n-button>
            </n-space>
          </n-card>

          <n-card title="进度">
            <n-progress type="line" :percentage="progress" :height="20" />
          </n-card>

          <n-card title="日志">
            <template #header-extra>
              <n-button size="small" @click="clearLog">清除</n-button>
            </template>
            <n-log ref="logRef" :log="logText" style="height: 220px" />
          </n-card>
        </n-space>
      </n-layout-content>
    </n-layout>
  </n-config-provider>
</template>
