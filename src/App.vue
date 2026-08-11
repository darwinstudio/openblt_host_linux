<script setup lang="ts">
import { ref, computed, onMounted } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import {
  NConfigProvider,
  NLayout,
  NLayoutHeader,
  NLayoutContent,
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

// ---- 库版本（验证 FFI 打通）----
const libVersion = ref("");
async function loadVersion() {
  libVersion.value = await invoke("version");
}

// ---- 后端事件监听（进度 / 日志）----
const logLines = ref<string[]>([]);
const latestLog = computed(() => logLines.value[logLines.value.length - 1] ?? "");
function log(msg: string) {
  logLines.value.push(msg);
}

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
const firmwarePath = ref("");
async function pickFile() {
  const selected = await open({
    filters: [{ name: "Motorola S-record", extensions: ["s19", "s28", "s37", "srec", "mot"] }],
  });
  if (typeof selected === "string") {
    firmwarePath.value = selected;
    log(`已选择固件: ${selected}`);
  }
}

// ---- 进度 / 烧录中标志 ----
const progress = ref(0);
const programming = ref(false);

// ---- 设置界面开关 ----
const showSettings = ref(false);

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

      <n-layout-content content-style="padding: 24px">
        <n-space vertical :size="16">
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

          <!-- 进度条 + 日志固定一行 -->
          <div style="display: flex; align-items: center; gap: 16px; width: 100%">
            <n-progress
              type="line"
              :percentage="progress"
              :height="18"
              style="flex: 2; min-width: 0"
            />
            <span
              class="log-line"
              style="
                flex: 3;
                min-width: 0;
                white-space: nowrap;
                overflow: hidden;
                text-overflow: ellipsis;
              "
              >{{ latestLog }}</span
            >
          </div>
        </n-space>
      </n-layout-content>
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
