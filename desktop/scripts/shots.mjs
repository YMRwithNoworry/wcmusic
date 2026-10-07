// 截图脚本：用 Playwright 打开 vite preview 的产物，注入 Tauri IPC 桩数据，
// 把各个页面截下来，用来肉眼检查排版、配色与动画状态。
//
// 用法：先 `npx vite preview --port 4173 --strictPort`，再 `node scripts/shots.mjs`。
import fs from "node:fs";
import path from "node:path";

import { chromium } from "playwright";

const BASE = process.env.SHOT_URL ?? "http://localhost:4173";
const OUT = path.resolve("shots");
fs.mkdirSync(OUT, { recursive: true });

const cover = (seed) => `https://picsum.photos/seed/${seed}/300/300`;

const tracks = Array.from({ length: 14 }).map((_, index) => ({
  id: `t${index}`,
  title: ["凌晨信号", "潮汐来信", "玻璃晴空", "静默花园", "夜航西飞", "城市褶皱"][index % 6] + (index > 5 ? " (Live)" : ""),
  artist: ["林屿", "Nocturne", "海岸线乐队", "苏晚"][index % 4],
  album: ["夜行记", "蓝色时期", "海边的房间"][index % 3],
  durationMs: 180000 + index * 7000,
  uri: "",
  artworkUri: cover(index),
  source: "kw",
  sourceId: `sid${index}`,
  quality: null,
}));

const playlists = Array.from({ length: 8 }).map((_, index) => ({
  channel: "Kuwo",
  id: `p${index}`,
  name: ["深夜电台", "通勤低气压", "写代码时听", "雨天爵士", "旧唱片", "清晨第一杯"][index % 6],
  author: ["屿溪", "编辑部", "某位用户"][index % 3],
  artwork_uri: cover(`p${index}`),
  track_count: 20 + index,
  play_count: 1200 + index,
  description: null,
  url: null,
}));

const lyrics = [
  [0, "凌晨三点的信号", "Signal at 3am"],
  [6000, "穿过你窗前的雾", "Through the fog before your window"],
  [12000, "我把整座城市调成静音", "I muted the whole city"],
  [19000, "只为听清你呼吸的节拍", "Just to hear the rhythm of your breath"],
  [26000, "灯塔在水面上写字", "The lighthouse writes on the water"],
  [33000, "每一笔都是未寄出的信", "Every stroke an unsent letter"],
  [41000, "如果海真的记得", "If the sea truly remembers"],
  [48000, "请替我说一句晚安", "Say goodnight for me"],
  [56000, "潮汐会带走脚印", "The tide will take the footprints"],
  [63000, "但带不走此刻的光", "But not this moment's light"],
].map(([time_ms, text, translation]) => ({ time_ms, text, translation }));

const settings = {
  quality_index: 1,
  dark_theme: true,
  spatial_audio_enabled: true,
  lyrics_enabled: true,
  lyrics: {
    font_family: "MiSans",
    font_size: 30,
    font_weight: 600,
    text_color: 0xf4f4f6,
    text_alpha: 1,
    highlight_color: 0xf0b84e,
    highlight_alpha: 1,
    stroke_color: 0x101014,
    stroke_alpha: 0.9,
    stroke_width: 1,
    opacity: 1,
    background_color: 0x000000,
    background_opacity: 0,
    alignment: "center",
    single_line: false,
    show_translation: true,
    karaoke: true,
    animation: "slide",
    always_on_top: true,
    locked: false,
    allow_offscreen: false,
    hide_when_paused: false,
    offset_ms: 0,
    window_x: null,
    window_y: null,
    window_width: 900,
    window_height: 160,
  },
  hotkeys: {
    enabled: true,
    previous: "ctrl-alt-left",
    next: "ctrl-alt-right",
    toggle_play: "ctrl-alt-space",
    volume_up: "ctrl-alt-up",
    volume_down: "ctrl-alt-down",
    mute: "ctrl-alt-m",
    seek_forward: "ctrl-alt-shift-right",
    seek_backward: "ctrl-alt-shift-left",
    toggle_lyrics: "ctrl-alt-l",
    toggle_window: "ctrl-alt-w",
  },
  use_network_proxy: false,
  saved_playlists: [{ playlist: playlists[0] }, { playlist: playlists[2] }],
  saved_tracks: tracks.slice(0, 5).map((track) => ({ folder: "我的收藏", track })),
  playback_mode: "listLoop",
};

/// 注入到页面里的 Tauri IPC 桩：让前端在浏览器里也能拿到数据。
function installStub(data) {
  // 桩数据在页面里也要能改：深浅主题切换靠它才看得到效果。
  const state = { settings: data.settings };
  const handlers = {
    app_info: () => ({ name: "WCMusic", version: "1.2.27", platform: "windows" }),
    get_settings: () => state.settings,
    save_settings: (args) => {
      state.settings = args.settings;
      return state.settings;
    },
    playback_snapshot: () => ({
      playing: true,
      position_ms: 34200,
      finished: false,
      loading: false,
      current: data.tracks[0],
      volume: 0.78,
      spatial: true,
      error: null,
    }),
    track_artwork: () => data.covers[0],
    search_online: () => data.tracks,
    load_playlists: () => data.playlists,
    load_playlist_tracks: () => data.tracks,
    load_rankings: () => [
      { id: "r1", name: "热歌榜", channel: "Kuwo", artwork_uri: null },
      { id: "r2", name: "新歌榜", channel: "Kuwo", artwork_uri: null },
      { id: "r3", name: "飙升榜", channel: "Kuwo", artwork_uri: null },
      { id: "r4", name: "原创榜", channel: "Kuwo", artwork_uri: null },
    ],
    load_ranking_tracks: () => data.tracks,
    load_lyrics: () => data.lyrics,
    lyrics_presets: () => ({
      fonts: ["MiSans", "Microsoft YaHei UI", "Noto Sans SC"],
      text_colors: [0xf4f4f6, 0xffffff, 0xe7e7ec, 0x101014],
      highlight_colors: [0xf0b84e, 0x6ea8fe, 0xc3cf7a, 0xff9f7a],
      stroke_colors: [0x000000, 0x101014, 0xffffff, 0xe7e7ec],
    }),
    playlist_folders: () => ["试听列表", "我的收藏", "最近播放", "通勤"],
    toggle_saved_playlist: () => state.settings,
    toggle_saved_track: () => state.settings,
    list_sources: () => [
      { index: null, name: "内置音源（屿溪-终章）", active: false, capabilities: [] },
      {
        index: 0,
        name: "示例音源",
        active: true,
        capabilities: ["酷我音乐", "酷狗音乐", "QQ 音乐", "网易云音乐"],
      },
    ],
    pick_source_file: () => null,
    hotkey_list: () =>
      Object.entries(state.settings.hotkeys)
        .filter(([key]) => key !== "enabled")
        .map(([key, binding]) => ({
          action: key,
          label: {
            previous: "上一首",
            next: "下一首",
            toggle_play: "播放/暂停",
            volume_up: "音量加",
            volume_down: "音量减",
            mute: "静音",
            seek_forward: "快进 5 秒",
            seek_backward: "快退 5 秒",
            toggle_lyrics: "显示/隐藏桌面歌词",
            toggle_window: "显示/隐藏主界面",
          }[key],
          binding,
          display: String(binding).replace("ctrl-alt-", "Ctrl + Alt + ").replace("shift-", "Shift + "),
        })),
    hotkey_issues: () => [
      { action: "toggle_lyrics", label: "显示/隐藏桌面歌词", message: "Ctrl + Alt + L 已被其它程序占用，注册失败" },
    ],
    check_update: () => ({
      current_version: "1.2.27",
      latest_version: "1.2.30",
      release_url: "https://github.com/YMRwithNoworry/wcmusic/releases",
    }),
    toggle_lyrics_window: () => true,
    toggle_main_window: () => true,
  };

  let nextCallbackId = 1;
  window.__TAURI_INTERNALS__ = {
    metadata: { currentWindow: { label: "main" }, currentWebview: { label: "main" } },
    transformCallback(callback) {
      const id = nextCallbackId++;
      window[`_${id}`] = callback;
      return id;
    },
    async invoke(command, args) {
      const handler = handlers[command];
      if (handler) {
        return handler(args ?? {});
      }
      return null;
    },
  };
}

const browser = await chromium.launch();
const context = await browser.newContext({
  viewport: { width: 1180, height: 760 },
  deviceScaleFactor: 2,
  locale: "zh-CN",
});
const page = await context.newPage();
await page.addInitScript(installStub, { settings, tracks, playlists, lyrics, covers: tracks.map((_, index) => cover(index)) });
await page.goto(BASE, { waitUntil: "networkidle" });
await page.waitForTimeout(1200);

const shots = [
  ["01-home", "主页"],
  ["02-search", "搜索"],
  ["03-rankings", "排行榜"],
  ["04-playlists", "歌单"],
  ["05-sources", "音源"],
  ["06-settings", "设置"],
];

for (const [name, label] of shots) {
  await page.getByRole("button", { name: label, exact: true }).click();
  await page.waitForTimeout(900);
  await page.screenshot({ path: path.join(OUT, `${name}.png`) });
  console.log("截图", name);
}

// 搜索页：跑一次搜索看结果列表。
await page.getByRole("button", { name: "搜索", exact: true }).click();
await page.waitForTimeout(300);
await page.getByPlaceholder("搜索歌曲、歌手").fill("夜航");
await page.keyboard.press("Enter");
await page.waitForTimeout(1000);
await page.screenshot({ path: path.join(OUT, "07-search-results.png") });
console.log("截图 07-search-results");

// 详情页：点播放栏左侧。
await page.locator("footer button").first().click();
await page.waitForTimeout(1200);
await page.screenshot({ path: path.join(OUT, "08-now-playing.png") });
console.log("截图 08-now-playing");

// 浅色主题。
await page.getByRole("button", { name: "设置", exact: true }).click();
await page.waitForTimeout(400);
await page.getByRole("switch").first().click();
await page.waitForTimeout(600);
await page.getByRole("button", { name: "主页", exact: true }).click();
await page.waitForTimeout(900);
await page.screenshot({ path: path.join(OUT, "09-light-home.png") });
console.log("截图 09-light-home");

await browser.close();
