import { useCallback, useEffect, useState, type ReactNode } from "react";
import { toast } from "sonner";

import { app, hotkeys, lyrics as lyricsApi, lyricsWindow } from "@/api";
import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Slider } from "@/components/ui/slider";
import { Switch } from "@/components/ui/switch";
import { useSettings } from "@/settings-context";
import type {
  HotKeyIssue,
  HotKeyView,
  LyricsAlignment,
  LyricsAnimation,
  LyricsPresets,
  PlaybackMode,
  UpdateCheck,
} from "@/types";

const PLAYBACK_MODES: { value: PlaybackMode; label: string }[] = [
  { value: "sequence", label: "顺序播放" },
  { value: "listLoop", label: "列表循环" },
  { value: "shuffle", label: "随机播放" },
  { value: "singleLoop", label: "单曲循环" },
];

const QUALITIES = ["标准 128k", "高品 320k", "无损 FLAC"];

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

function hex(rgb: number): string {
  return `#${rgb.toString(16).padStart(6, "0")}`;
}

function Section({
  title,
  hint,
  children,
}: {
  title: string;
  hint?: string;
  children: ReactNode;
}) {
  return (
    <Card className="gap-0 py-0">
      <div className="border-b px-4 py-2.5">
        <h2 className="text-[13px] font-semibold">{title}</h2>
        {hint ? <p className="mt-0.5 text-[11px] text-muted-foreground">{hint}</p> : null}
      </div>
      <div className="px-4 py-1">{children}</div>
    </Card>
  );
}

function Row({
  label,
  hint,
  children,
}: {
  label: string;
  hint?: string;
  children: ReactNode;
}) {
  return (
    <div className="flex items-center justify-between gap-6 py-2.5">
      <div className="min-w-0">
        <div className="text-[13px]">{label}</div>
        {hint ? <div className="text-[11px] text-muted-foreground">{hint}</div> : null}
      </div>
      <div className="flex shrink-0 items-center gap-2">{children}</div>
    </div>
  );
}

function ColorSelect({
  colors,
  value,
  onChange,
}: {
  colors: number[];
  value: number;
  onChange: (next: number) => void;
}) {
  const options = colors.includes(value) ? colors : [value, ...colors];
  return (
    <Select value={String(value)} onValueChange={(next) => onChange(Number(next))}>
      <SelectTrigger className="w-[112px]">
        <span className="flex items-center gap-2">
          <span
            className="size-3.5 rounded-[4px] ring-1 ring-border"
            style={{ background: hex(value) }}
          />
          <span className="tabular text-[11px]">{hex(value)}</span>
        </span>
      </SelectTrigger>
      <SelectContent>
        {options.map((color) => (
          <SelectItem key={color} value={String(color)}>
            <span className="flex items-center gap-2">
              <span
                className="size-3.5 rounded-[4px] ring-1 ring-border"
                style={{ background: hex(color) }}
              />
              <span className="tabular text-[11px]">{hex(color)}</span>
            </span>
          </SelectItem>
        ))}
      </SelectContent>
    </Select>
  );
}

export default function SettingsPage() {
  const { settings, update } = useSettings();
  const [updateCheck, setUpdateCheck] = useState<UpdateCheck | null>(null);
  const [checking, setChecking] = useState(false);
  const [bindings, setBindings] = useState<HotKeyView[]>([]);
  const [issues, setIssues] = useState<HotKeyIssue[]>([]);
  const [presets, setPresets] = useState<LyricsPresets | null>(null);

  const loadHotkeys = useCallback(async () => {
    try {
      setBindings(await hotkeys.list());
      setIssues(await hotkeys.issues());
    } catch {
      // 后端还没起来时忽略。
    }
  }, []);

  useEffect(() => {
    void loadHotkeys();
  }, [loadHotkeys, settings?.hotkeys.enabled]);

  useEffect(() => {
    lyricsApi
      .presets()
      .then(setPresets)
      .catch(() => setPresets(null));
  }, []);

  if (!settings) {
    return (
      <div className="flex h-full items-center justify-center text-sm text-muted-foreground">
        正在读取设置…
      </div>
    );
  }

  const lyrics = settings.lyrics;
  const patchLyrics = (patch: Partial<typeof lyrics>) =>
    void update({ lyrics: { ...lyrics, ...patch } });

  const checkUpdate = async () => {
    setChecking(true);
    try {
      setUpdateCheck(await app.checkUpdate());
    } catch (problem) {
      toast.error(String(problem));
    } finally {
      setChecking(false);
    }
  };

  return (
    <div className="h-full overflow-y-auto px-7 py-6">
      <h1 className="mb-5 text-[22px] font-semibold">设置</h1>

      <div className="flex max-w-[760px] flex-col gap-4 pb-6">
        <Section title="外观">
          <Row label="深色主题">
            <Switch
              checked={settings.dark_theme}
              onCheckedChange={(dark_theme) => void update({ dark_theme })}
            />
          </Row>
        </Section>

        <Section title="播放">
          <Row label="播放方式">
            <Select
              value={settings.playback_mode}
              onValueChange={(value) =>
                void update({ playback_mode: value as PlaybackMode })
              }
            >
              <SelectTrigger className="w-[132px]">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {PLAYBACK_MODES.map((mode) => (
                  <SelectItem key={mode.value} value={mode.value}>
                    {mode.label}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </Row>
          <Row label="音质">
            <Select
              value={String(settings.quality_index)}
              onValueChange={(value) => void update({ quality_index: Number(value) })}
            >
              <SelectTrigger className="w-[132px]">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {QUALITIES.map((label, index) => (
                  <SelectItem key={label} value={String(index)}>
                    {label}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </Row>
          <Row label="空间音频" hint="双耳空间化：拓宽、串扰、早期反射与房间混响">
            <Switch
              checked={settings.spatial_audio_enabled}
              onCheckedChange={(spatial_audio_enabled) => {
                void update({ spatial_audio_enabled });
              }}
            />
          </Row>
        </Section>

        <Section title="歌词">
          <Row label="桌面歌词" hint="启动时自动显示置顶歌词窗口">
            <Switch
              checked={settings.lyrics_enabled}
              onCheckedChange={(lyrics_enabled) => void update({ lyrics_enabled })}
            />
          </Row>
          <Row label="桌面歌词窗口">
            <Button variant="secondary" size="sm" onClick={() => void lyricsWindow.toggle()}>
              显示 / 隐藏
            </Button>
          </Row>
          <Row label="显示翻译">
            <Switch
              checked={lyrics.show_translation}
              onCheckedChange={(show_translation) => patchLyrics({ show_translation })}
            />
          </Row>
          <Row label="逐字填充">
            <Switch
              checked={lyrics.karaoke}
              onCheckedChange={(karaoke) => patchLyrics({ karaoke })}
            />
          </Row>
          <Row label="单行模式">
            <Switch
              checked={lyrics.single_line}
              onCheckedChange={(single_line) => patchLyrics({ single_line })}
            />
          </Row>
          <Row label="窗口置顶">
            <Switch
              checked={lyrics.always_on_top}
              onCheckedChange={(always_on_top) => patchLyrics({ always_on_top })}
            />
          </Row>
          <Row
            label="锁定鼠标穿透"
            hint="锁定时点击会落到下层窗口，不会挡住其它软件；要拖动歌词窗口先解锁"
          >
            <Switch
              checked={lyrics.locked}
              onCheckedChange={(locked) => patchLyrics({ locked })}
            />
          </Row>
          <Row label="对齐方式">
            <Select
              value={lyrics.alignment}
              onValueChange={(value) => patchLyrics({ alignment: value as LyricsAlignment })}
            >
              <SelectTrigger className="w-[112px]">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {ALIGNMENTS.map((item) => (
                  <SelectItem key={item.value} value={item.value}>
                    {item.label}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </Row>
          <Row label="切换动画">
            <Select
              value={lyrics.animation}
              onValueChange={(value) => patchLyrics({ animation: value as LyricsAnimation })}
            >
              <SelectTrigger className="w-[112px]">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {ANIMATIONS.map((item) => (
                  <SelectItem key={item.value} value={item.value}>
                    {item.label}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </Row>
          <Row label="歌词延迟" hint="毫秒，正值歌词提前">
            <Input
              type="number"
              className="w-[104px]"
              value={lyrics.offset_ms}
              onChange={(event) => patchLyrics({ offset_ms: Number(event.target.value) })}
            />
          </Row>
        </Section>

        <Section title="歌词外观" hint="桌面歌词窗口与详情页共用这套外观">
          <Row label="字体">
            <Select
              value={lyrics.font_family}
              onValueChange={(font_family) => patchLyrics({ font_family })}
            >
              <SelectTrigger className="w-[176px]">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {(presets?.fonts ?? [lyrics.font_family]).map((font) => (
                  <SelectItem key={font} value={font}>
                    {font}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </Row>
          <Row label="字号">
            <Slider
              className="w-[160px]"
              value={[lyrics.font_size]}
              min={14}
              max={72}
              step={1}
              onValueChange={([font_size]) => patchLyrics({ font_size })}
            />
            <span className="tabular w-8 text-right text-[11px] text-muted-foreground">
              {Math.round(lyrics.font_size)}
            </span>
          </Row>
          <Row label="文字颜色">
            <ColorSelect
              colors={presets?.text_colors ?? [lyrics.text_color]}
              value={lyrics.text_color}
              onChange={(text_color) => patchLyrics({ text_color })}
            />
          </Row>
          <Row label="高亮颜色">
            <ColorSelect
              colors={presets?.highlight_colors ?? [lyrics.highlight_color]}
              value={lyrics.highlight_color}
              onChange={(highlight_color) => patchLyrics({ highlight_color })}
            />
          </Row>
          <Row label="描边">
            <ColorSelect
              colors={presets?.stroke_colors ?? [lyrics.stroke_color]}
              value={lyrics.stroke_color}
              onChange={(stroke_color) => patchLyrics({ stroke_color })}
            />
            <Slider
              className="w-[120px]"
              value={[lyrics.stroke_width]}
              min={0}
              max={6}
              step={0.5}
              onValueChange={([stroke_width]) => patchLyrics({ stroke_width })}
            />
          </Row>
          <Row label="背景不透明度">
            <Slider
              className="w-[160px]"
              value={[Math.round(lyrics.background_opacity * 100)]}
              max={100}
              step={1}
              onValueChange={([value]) => patchLyrics({ background_opacity: value / 100 })}
            />
          </Row>
          <Row label="整体不透明度">
            <Slider
              className="w-[160px]"
              value={[Math.round(lyrics.opacity * 100)]}
              min={10}
              max={100}
              step={1}
              onValueChange={([value]) => patchLyrics({ opacity: value / 100 })}
            />
          </Row>
        </Section>

        <Section title="全局快捷键" hint="关闭后不再抢占其它程序的按键">
          <Row label="启用全局快捷键">
            <Switch
              checked={settings.hotkeys.enabled}
              onCheckedChange={(enabled) =>
                void update({ hotkeys: { ...settings.hotkeys, enabled } })
              }
            />
          </Row>
          <div className="grid grid-cols-2 gap-x-8">
            {bindings.map((item) => (
              <div
                key={item.action}
                className="flex items-center justify-between border-t py-2 text-[13px]"
              >
                <span className="text-muted-foreground">{item.label}</span>
                <span className="tabular text-[11px]">{item.display}</span>
              </div>
            ))}
          </div>
          {issues.map((issue) => (
            <div
              key={`${issue.action}-${issue.message}`}
              className="mt-2 rounded-[10px] border border-destructive/30 bg-destructive/10 px-3 py-2 text-[11px] text-destructive"
            >
              {issue.label}：{issue.message}
            </div>
          ))}
        </Section>

        <Section title="网络">
          <Row label="使用系统代理" hint="读取 HTTP_PROXY / HTTPS_PROXY">
            <Switch
              checked={settings.use_network_proxy}
              onCheckedChange={(use_network_proxy) => void update({ use_network_proxy })}
            />
          </Row>
        </Section>

        <Section title="关于">
          <Row label="版本更新">
            <Button variant="secondary" size="sm" disabled={checking} onClick={() => void checkUpdate()}>
              {checking ? "检查中" : "检查更新"}
            </Button>
          </Row>
          {updateCheck ? (
            <Row label={`当前版本 ${updateCheck.current_version}`}>
              <span className="text-[12px] text-muted-foreground">
                {updateCheck.latest_version
                  ? `最新版本 ${updateCheck.latest_version}`
                  : "暂未发布版本"}
              </span>
              {updateCheck.release_url ? (
                <Button
                  variant="ghost"
                  size="sm"
                  onClick={() => void app.openExternal(updateCheck.release_url as string)}
                >
                  打开发布页
                </Button>
              ) : null}
            </Row>
          ) : null}
        </Section>
      </div>
    </div>
  );
}
