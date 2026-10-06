import { useCallback, useEffect, useState, type ReactNode } from "react";

import { app, hotkeys } from "../api";
import { useSettings } from "../settings-context";
import type {
  HotKeyIssue,
  HotKeyView,
  LyricsAlignment,
  LyricsAnimation,
  PlaybackMode,
  UpdateCheck,
} from "../types";

const PLAYBACK_MODES: { value: PlaybackMode; label: string }[] = [
  { value: "sequence", label: "顺序播放" },
  { value: "listLoop", label: "列表循环" },
  { value: "shuffle", label: "随机播放" },
  { value: "singleLoop", label: "单曲循环" },
];

const QUALITIES = ["流畅", "标准", "高品"];

const ALIGNMENTS: { value: LyricsAlignment; label: string }[] = [
  { value: "left", label: "左对齐" },
  { value: "center", label: "居中" },
  { value: "right", label: "右对齐" },
];

const ANIMATIONS: { value: LyricsAnimation; label: string }[] = [
  { value: "off", label: "无动画" },
  { value: "slide", label: "上下滚动" },
  { value: "scale", label: "缩放淡入" },
];

function Row({ label, hint, children }: { label: string; hint?: string; children: ReactNode }) {
  return (
    <div className="row">
      <div className="row-label">
        <span>{label}</span>
        {hint ? <span className="row-hint">{hint}</span> : null}
      </div>
      <div className="row-control">{children}</div>
    </div>
  );
}

function Switch({ value, onChange }: { value: boolean; onChange: (next: boolean) => void }) {
  return (
    <button
      className={value ? "switch on" : "switch"}
      onClick={() => onChange(!value)}
      aria-pressed={value}
    >
      <span className="knob" />
    </button>
  );
}

export default function SettingsPage() {
  const { settings, update } = useSettings();
  const [updateCheck, setUpdateCheck] = useState<UpdateCheck | null>(null);
  const [checking, setChecking] = useState(false);
  const [checkError, setCheckError] = useState<string | null>(null);
  const [bindings, setBindings] = useState<HotKeyView[]>([]);
  const [issues, setIssues] = useState<HotKeyIssue[]>([]);

  const loadHotkeys = useCallback(async () => {
    try {
      setBindings(await hotkeys.list());
      setIssues(await hotkeys.issues());
    } catch {
      // 后端还没起来时忽略。
    }
  }, []);

  // 开关变化后后端会重新注册，这里跟着刷新绑定与失败信息。
  useEffect(() => {
    void loadHotkeys();
  }, [loadHotkeys, settings?.hotkeys.enabled]);

  if (!settings) {
    return <p className="hint">正在读取设置…</p>;
  }

  const checkUpdate = async () => {
    setChecking(true);
    setCheckError(null);
    try {
      setUpdateCheck(await app.checkUpdate());
    } catch (problem) {
      setCheckError(String(problem));
    } finally {
      setChecking(false);
    }
  };

  return (
    <div className="settings">
      <section>
        <h2>外观</h2>
        <Row label="深色主题">
          <Switch
            value={settings.dark_theme}
            onChange={(dark_theme) => void update({ dark_theme })}
          />
        </Row>
      </section>

      <section>
        <h2>播放</h2>
        <Row label="播放方式">
          <select
            value={settings.playback_mode}
            onChange={(event) =>
              void update({ playback_mode: event.target.value as PlaybackMode })
            }
          >
            {PLAYBACK_MODES.map((mode) => (
              <option key={mode.value} value={mode.value}>
                {mode.label}
              </option>
            ))}
          </select>
        </Row>
        <Row label="音质">
          <select
            value={settings.quality_index}
            onChange={(event) =>
              void update({ quality_index: Number(event.target.value) })
            }
          >
            {QUALITIES.map((label, index) => (
              <option key={label} value={index}>
                {label}
              </option>
            ))}
          </select>
        </Row>
        <Row label="空间音频">
          <Switch
            value={settings.spatial_audio_enabled}
            onChange={(spatial_audio_enabled) => void update({ spatial_audio_enabled })}
          />
        </Row>
      </section>

      <section>
        <h2>歌词</h2>
        <Row label="桌面歌词">
          <Switch
            value={settings.lyrics_enabled}
            onChange={(lyrics_enabled) => void update({ lyrics_enabled })}
          />
        </Row>
        <Row label="显示翻译">
          <Switch
            value={settings.lyrics.show_translation}
            onChange={(show_translation) =>
              void update({ lyrics: { ...settings.lyrics, show_translation } })
            }
          />
        </Row>
        <Row label="逐字填充">
          <Switch
            value={settings.lyrics.karaoke}
            onChange={(karaoke) => void update({ lyrics: { ...settings.lyrics, karaoke } })}
          />
        </Row>
        <Row label="对齐方式">
          <select
            value={settings.lyrics.alignment}
            onChange={(event) =>
              void update({
                lyrics: {
                  ...settings.lyrics,
                  alignment: event.target.value as LyricsAlignment,
                },
              })
            }
          >
            {ALIGNMENTS.map((item) => (
              <option key={item.value} value={item.value}>
                {item.label}
              </option>
            ))}
          </select>
        </Row>
        <Row label="切换动画">
          <select
            value={settings.lyrics.animation}
            onChange={(event) =>
              void update({
                lyrics: {
                  ...settings.lyrics,
                  animation: event.target.value as LyricsAnimation,
                },
              })
            }
          >
            {ANIMATIONS.map((item) => (
              <option key={item.value} value={item.value}>
                {item.label}
              </option>
            ))}
          </select>
        </Row>
        <Row label="歌词延迟" hint="毫秒，正值歌词提前">
          <input
            type="number"
            value={settings.lyrics.offset_ms}
            onChange={(event) =>
              void update({
                lyrics: { ...settings.lyrics, offset_ms: Number(event.target.value) },
              })
            }
          />
        </Row>
      </section>

      <section>
        <h2>网络</h2>
        <Row label="使用系统代理" hint="读取 HTTP_PROXY / HTTPS_PROXY">
          <Switch
            value={settings.use_network_proxy}
            onChange={(use_network_proxy) => void update({ use_network_proxy })}
          />
        </Row>
      </section>

      <section>
        <h2>全局快捷键</h2>
        <Row label="启用全局快捷键" hint="关闭后不再抢占其它程序的按键">
          <Switch
            value={settings.hotkeys.enabled}
            onChange={(enabled) => void update({ hotkeys: { ...settings.hotkeys, enabled } })}
          />
        </Row>
        {bindings.map((item) => (
          <Row key={item.action} label={item.label}>
            <span className="hint">{item.display}</span>
          </Row>
        ))}
        {issues.map((issue) => (
          <p className="error" key={`${issue.action}-${issue.message}`}>
            {issue.label}：{issue.message}
          </p>
        ))}
      </section>

      <section>
        <h2>关于</h2>
        <Row label="版本更新">
          <button onClick={() => void checkUpdate()} disabled={checking}>
            {checking ? "检查中…" : "检查更新"}
          </button>
        </Row>
        {checkError ? <p className="error">{checkError}</p> : null}
        {updateCheck ? (
          <Row label={`当前版本 ${updateCheck.current_version}`}>
            <span className="hint">
              {updateCheck.latest_version
                ? `最新版本 ${updateCheck.latest_version}`
                : "暂未发布版本"}
            </span>
            {updateCheck.release_url ? (
              <button
                className="link"
                onClick={() => void app.openExternal(updateCheck.release_url as string)}
              >
                打开发布页
              </button>
            ) : null}
          </Row>
        ) : null}
      </section>
    </div>
  );
}
