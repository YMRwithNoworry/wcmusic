import type { ReactNode } from "react";

import { useSettings } from "../settings-context";
import type { LyricsAlignment, LyricsAnimation, PlaybackMode } from "../types";

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
  if (!settings) {
    return <p className="hint">正在读取设置…</p>;
  }

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
    </div>
  );
}
