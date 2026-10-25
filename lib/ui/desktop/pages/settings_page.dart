import 'package:flutter/material.dart';
import 'package:forui/forui.dart';
import 'package:provider/provider.dart';

import '../../../domain/models/lyrics_overlay_style.dart';
import '../../../domain/models/playback_quality.dart';
import '../../core/app_theme_controller.dart';
import '../../state/player_view_model.dart';
import '../widgets/common.dart';
import 'feedback_dialog.dart';

class SettingsPage extends StatelessWidget {
  const SettingsPage({super.key});

  @override
  Widget build(BuildContext context) {
    final model = context.watch<PlayerViewModel>();
    final theme = context.watch<AppThemeController>();
    final isAndroid = Theme.of(context).platform == TargetPlatform.android;
    return ListView(
      padding: const EdgeInsets.only(bottom: 32),
      children: [
        const PageHeader(
          eyebrow: 'WCMusic / 设置',
          title: '设置',
          subtitle: '保持安静，也保持可控。更改会立即保存。',
        ),
        Padding(
          padding: const EdgeInsets.symmetric(horizontal: 32),
          child: PageBody(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                _SettingsSection(
                  title: '播放',
                  icon: FLucideIcons.play,
                  children: [
                    _SettingRow(
                      title: '关闭后继续播放',
                      description: '关闭窗口时隐藏到系统托盘，音乐不中断',
                      trailing: FSwitch(
                        value: model.backgroundPlayback,
                        onChange: model.setBackgroundPlayback,
                      ),
                    ),
                    _SettingRow(
                      title: isAndroid ? '悬浮窗歌词' : '桌面歌词',
                      description: isAndroid
                          ? '在其他应用上方同步显示歌词'
                          : '在桌面置顶窗口中同步显示歌词',
                      trailing: FSwitch(
                        value: model.floatingLyricsEnabled,
                        onChange: model.floatingLyricsService.isSupported
                            ? model.setFloatingLyrics
                            : null,
                      ),
                    ),
                    _SettingRow(
                      title: '空间音效',
                      description: '拓宽立体声声场，增强空间感',
                      trailing: FSwitch(
                        value: model.spatialAudioEnabled,
                        onChange: model.setSpatialAudioEnabled,
                      ),
                    ),
                    _SettingRow(
                      title: '播放音量',
                      description: '与底部播放条的音量保持同步',
                      trailing: SizedBox(
                        width: 220,
                        child: Row(
                          children: [
                            Expanded(
                              child: FSlider(
                                control: FSliderControl.liftedContinuous(
                                  value: FSliderValue(
                                    max: model.volume.clamp(0.0, 1.0),
                                  ),
                                  onChange: (value) =>
                                      model.setVolume(value.max),
                                ),
                              ),
                            ),
                            SizedBox(
                              width: 52,
                              child: Text(
                                '${(model.volume * 100).round()}%',
                                maxLines: 1,
                                textAlign: TextAlign.end,
                                style: context.theme.typography.body.xs
                                    .copyWith(
                                      color:
                                          context.theme.colors.mutedForeground,
                                    ),
                              ),
                            ),
                          ],
                        ),
                      ),
                    ),
                    _SettingRow(
                      title: '播放音质',
                      description: '整曲解析时优先请求的音质',
                      trailing: SizedBox(
                        width: 190,
                        child: FSelect<PlaybackQuality>(
                          control: FSelectControl.managed(
                            initial: model.playbackQuality,
                            onChange: (quality) {
                              if (quality != null) {
                                model.setPlaybackQuality(quality);
                              }
                            },
                          ),
                          items: {
                            for (final quality in PlaybackQuality.values)
                              quality.label: quality,
                          },
                        ),
                      ),
                    ),
                  ],
                ),
                const SizedBox(height: 16),
                _SettingsSection(
                  title: '外观与动效',
                  icon: FLucideIcons.sparkles,
                  children: [
                    _SettingRow(
                      title: '主题模式',
                      description: '日间、夜间或跟随系统',
                      trailing: Wrap(
                        spacing: 6,
                        children: [
                          for (final entry in const [
                            (ThemeMode.system, '系统', FLucideIcons.monitor),
                            (ThemeMode.light, '日间', FLucideIcons.sun),
                            (ThemeMode.dark, '夜间', FLucideIcons.moon),
                          ])
                            FilterPill(
                              label: entry.$2,
                              icon: entry.$3,
                              selected: theme.mode == entry.$1,
                              onPress: () => theme.setMode(entry.$1),
                            ),
                        ],
                      ),
                    ),
                    _SettingRow(
                      title: '减少动态效果',
                      description: '关闭呼吸、缩放与大幅转场，只保留必要反馈',
                      trailing: FSwitch(
                        value: theme.reduceMotion,
                        onChange: theme.setReduceMotion,
                      ),
                    ),
                  ],
                ),
                if (!isAndroid) ...[
                  const SizedBox(height: 16),
                  _LyricsStyleSection(style: model.lyricsStyle),
                ],
                const SizedBox(height: 16),
                _SettingsSection(
                  title: '帮助与反馈',
                  icon: FLucideIcons.messageSquare,
                  children: [
                    _SettingRow(
                      title: '问题反馈',
                      description: '反馈将通过邮件送达开发者',
                      trailing: FButton(
                        variant: FButtonVariant.outline,
                        size: FButtonSizeVariant.sm,
                        mainAxisSize: MainAxisSize.min,
                        prefix: const Icon(FLucideIcons.send, size: 15),
                        onPress: () => showFeedbackSheet(context),
                        child: const Text('写反馈'),
                      ),
                    ),
                  ],
                ),
                const SizedBox(height: 16),
                _SettingsSection(
                  title: '关于',
                  icon: FLucideIcons.info,
                  children: [
                    _SettingRow(
                      title: 'WCMusic',
                      description:
                          'Flutter 3.44 · Forui · Rust 音源运行时${model.appVersion == null ? '' : ' · 当前版本 v${model.appVersion}'}',
                    ),
                    _SettingRow(
                      title: '界面',
                      description: '桌面端 UI 使用 Forui 组件构建，动画与列表按 60fps 预算调优',
                    ),
                    _UpdateRow(model: model),
                  ],
                ),
              ],
            ),
          ),
        ),
      ],
    );
  }
}

class _UpdateRow extends StatelessWidget {
  const _UpdateRow({required this.model});

  final PlayerViewModel model;

  String get _description {
    if (model.updateError != null) return model.updateError!;
    if (model.isInstallingUpdate) {
      final progress = model.updateDownloadProgress;
      return progress == null
          ? '正在下载更新包…'
          : '正在下载更新包 ${(progress * 100).clamp(0, 100).toStringAsFixed(0)}%';
    }
    if (model.isCheckingUpdate) return '正在连接 GitHub 检查版本…';
    final update = model.availableUpdate;
    if (update != null) {
      return '发现新版本 v${update.version} · ${_formatSize(update.assetSize)}';
    }
    final latest = model.latestRelease;
    if (latest != null) return '已是最新版本（v${latest.version}）';
    return '从 GitHub Releases 获取最新版本，并自动下载安装';
  }

  static String _formatSize(int bytes) {
    if (bytes <= 0) return '大小未知';
    if (bytes >= 1024 * 1024) {
      return '${(bytes / 1024 / 1024).toStringAsFixed(1)} MB';
    }
    return '${(bytes / 1024).toStringAsFixed(0)} KB';
  }

  @override
  Widget build(BuildContext context) {
    final update = model.availableUpdate;
    return _SettingRow(
      title: '软件更新',
      description: _description,
      trailing: Wrap(
        spacing: 8,
        runSpacing: 8,
        alignment: WrapAlignment.end,
        children: [
          if (update != null && !model.isInstallingUpdate)
            FButton(
              variant: FButtonVariant.primary,
              size: FButtonSizeVariant.sm,
              prefix: const Icon(FLucideIcons.download, size: 15),
              onPress: model.installUpdate,
              child: const Text('一键更新'),
            ),
          FButton(
            variant: FButtonVariant.outline,
            size: FButtonSizeVariant.sm,
            mainAxisSize: MainAxisSize.min,
            onPress: model.isCheckingUpdate || model.isInstallingUpdate
                ? null
                : () => model.checkForUpdate(),
            child: Text(update == null ? '检查更新' : '重新检查'),
          ),
          if (update != null)
            FButton(
              variant: FButtonVariant.ghost,
              size: FButtonSizeVariant.sm,
              mainAxisSize: MainAxisSize.min,
              onPress: () => model.releaseService.openExternal(update.pageUrl),
              child: const Text('版本说明'),
            ),
        ],
      ),
    );
  }
}

class _SettingsSection extends StatelessWidget {
  const _SettingsSection({
    required this.title,
    required this.children,
    this.icon,
  });

  final String title;
  final List<Widget> children;
  final IconData? icon;

  @override
  Widget build(BuildContext context) {
    final colors = context.theme.colors;
    final type = context.theme.typography;
    return Container(
      decoration: BoxDecoration(
        color: colors.card,
        borderRadius: const BorderRadius.all(Radius.circular(18)),
        border: Border.all(color: colors.border),
      ),
      padding: const EdgeInsets.fromLTRB(20, 18, 20, 8),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Row(
            children: [
              if (icon != null) ...[
                Icon(icon, size: 16, color: colors.primary),
                const SizedBox(width: 9),
              ],
              Text(
                title,
                style: type.display.xs.copyWith(
                  color: colors.foreground,
                  fontWeight: FontWeight.w600,
                ),
              ),
            ],
          ),
          const SizedBox(height: 6),
          ...children,
        ],
      ),
    );
  }
}

class _SettingRow extends StatelessWidget {
  const _SettingRow({required this.title, this.description, this.trailing});

  final String title;
  final String? description;
  final Widget? trailing;

  @override
  Widget build(BuildContext context) {
    final colors = context.theme.colors;
    final type = context.theme.typography;
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 10),
      child: Row(
        children: [
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  title,
                  style: type.body.sm.copyWith(
                    color: colors.foreground,
                    fontWeight: FontWeight.w500,
                  ),
                ),
                if (description != null) ...[
                  const SizedBox(height: 2),
                  Text(
                    description!,
                    style: type.body.xs.copyWith(color: colors.mutedForeground),
                  ),
                ],
              ],
            ),
          ),
          if (trailing != null) ...[const SizedBox(width: 18), trailing!],
        ],
      ),
    );
  }
}

class _LyricsStyleSection extends StatelessWidget {
  const _LyricsStyleSection({required this.style});

  /// 内置默认字体，也是旧版本样式的默认值
  static const _defaultFont = 'Microsoft YaHei UI';

  static const _fonts = [
    _defaultFont,
    'Microsoft YaHei',
    'SimSun',
    'SimHei',
    'KaiTi',
    'DengXian',
    'Segoe UI',
    'Arial',
  ];

  static const _textColors = [
    0xFFF5F3EC,
    0xFFFFFFFF,
    0xFF1C1F1B,
    0xFFE0533D,
    0xFFF2A93B,
    0xFFF5D76E,
    0xFF7AC74F,
    0xFF4FC3F7,
    0xFF5B8DEF,
    0xFF9B59B6,
    0xFFF27BA2,
  ];

  static const _backgroundColors = [
    0x00000000,
    0xFF1C1F1B,
    0xFF101418,
    0xFF1E2A24,
    0xFF14212E,
    0xFF2A2A2E,
    0xFFF5F3EC,
    0xFFFFF8E1,
    0xFFE8F5E9,
    0xFFE3F2FD,
    0xFFFCE4EC,
  ];

  final LyricsOverlayStyle style;

  /// 字体下拉选项：标签 -> 字族名。系统字体就绪后以系统列表为准，
  /// 并把当前选中项与内置默认字体放在最前，保证选中项始终可见。
  List<(String, String)> _fontOptions(PlayerViewModel model) {
    final current = style.fontFamily;
    final systemFonts = model.systemFonts;
    if (systemFonts.isEmpty) {
      return [
        for (final font in _fonts)
          if (font == current || font == _defaultFont) (font, font),
      ];
    }
    final options = <(String, String)>[];
    final defaultFont = model.defaultSystemFont ?? _defaultFont;
    final hasDefault = systemFonts.contains(defaultFont);
    if (hasDefault) {
      options.add(('系统默认（$defaultFont）', defaultFont));
    }
    if (current != defaultFont || !hasDefault) {
      options.add((
        systemFonts.contains(current) ? current : '$current（当前）',
        current,
      ));
    }
    for (final font in systemFonts) {
      if (font == defaultFont) continue;
      options.add((font, font));
    }
    return options;
  }

  @override
  Widget build(BuildContext context) {
    final model = context.read<PlayerViewModel>();
    final colors = context.theme.colors;
    final type = context.theme.typography;
    void update(LyricsOverlayStyle next) => model.updateLyricsStyle(next);
    return _SettingsSection(
      title: '桌面歌词外观',
      icon: FLucideIcons.micVocal,
      children: [
        _SettingRow(
          title: '固定歌词窗口',
          description: '固定时可拖动左侧把手；关闭后可拖动整个歌词面板',
          trailing: FSwitch(
            value: style.locked,
            onChange: (value) => update(style.copyWith(locked: value)),
          ),
        ),
        _SettingRow(
          title: '对齐方式',
          trailing: Wrap(
            spacing: 6,
            children: [
              for (final entry in const [
                ('left', '左对齐'),
                ('center', '居中'),
                ('right', '右对齐'),
              ])
                FilterPill(
                  label: entry.$2,
                  selected: style.alignment == entry.$1,
                  onPress: () => update(style.copyWith(alignment: entry.$1)),
                ),
            ],
          ),
        ),
        _SettingRow(
          title: '字体',
          description: model.systemFonts.isEmpty
              ? '字体列表暂不可用，显示内置字体'
              : '共 ${model.systemFonts.length} 款系统字体',
          trailing: SizedBox(
            width: 220,
            child: FSelect<String>(
              key: ValueKey(model.systemFonts.length),
              control: FSelectControl.managed(
                initial: style.fontFamily,
                onChange: (font) {
                  if (font != null) update(style.copyWith(fontFamily: font));
                },
              ),
              items: {
                for (final entry in _fontOptions(model)) entry.$1: entry.$2,
              },
            ),
          ),
        ),
        _SettingRow(
          title: '字体变体',
          description: '仅作用于当前高亮行，其余歌词保持常规字重',
          trailing: Wrap(
            spacing: 6,
            children: [
              for (final entry in const [
                (lyricsOverlayFontWeightRegular, '常规'),
                (lyricsOverlayFontWeightBold, '粗体'),
                (lyricsOverlayFontWeightLight, '细体'),
              ])
                FilterPill(
                  label: entry.$2,
                  selected: style.fontWeight == entry.$1,
                  onPress: () => update(style.copyWith(fontWeight: entry.$1)),
                ),
            ],
          ),
        ),
        _SettingRow(
          title: '字号',
          description: '${style.fontSize.round()} px',
          trailing: SizedBox(
            width: 240,
            child: FSlider(
              control: FSliderControl.liftedContinuous(
                value: FSliderValue(
                  max: ((style.fontSize - 16) / 32).clamp(0.0, 1.0),
                ),
                onChange: (value) =>
                    update(style.copyWith(fontSize: 16 + value.max * 32)),
              ),
            ),
          ),
        ),
        _SettingRow(
          title: '文字颜色',
          trailing: _ColorRow(
            colors: _textColors,
            selected: style.textColor,
            onSelected: (color) => update(style.copyWith(textColor: color)),
          ),
        ),
        _SettingRow(
          title: '背景颜色',
          trailing: _ColorRow(
            colors: _backgroundColors,
            selected: style.backgroundColor,
            onSelected: (color) =>
                update(style.copyWith(backgroundColor: color)),
          ),
        ),
        _SettingRow(
          title: '背景不透明度',
          description: '${(style.opacity * 100).round()}%',
          trailing: SizedBox(
            width: 240,
            child: FSlider(
              control: FSliderControl.liftedContinuous(
                value: FSliderValue(
                  max: ((style.opacity - .15) / .85).clamp(0.0, 1.0),
                ),
                onChange: (value) =>
                    update(style.copyWith(opacity: .15 + value.max * .85)),
              ),
            ),
          ),
        ),
        _SettingRow(
          title: '圆角大小',
          description: '${style.cornerRadius.round()} px',
          trailing: SizedBox(
            width: 240,
            child: FSlider(
              control: FSliderControl.liftedContinuous(
                value: FSliderValue(
                  max: (style.cornerRadius / 40).clamp(0.0, 1.0),
                ),
                onChange: (value) =>
                    update(style.copyWith(cornerRadius: value.max * 40)),
              ),
            ),
          ),
        ),
        Padding(
          padding: const EdgeInsets.only(top: 2, bottom: 10),
          child: Text(
            '拖动歌词窗口位置会自动保存。',
            style: type.body.xs.copyWith(
              color: colors.mutedForeground.withValues(alpha: .8),
            ),
          ),
        ),
      ],
    );
  }
}

class _ColorRow extends StatelessWidget {
  const _ColorRow({
    required this.colors,
    required this.selected,
    required this.onSelected,
  });

  final List<int> colors;
  final int selected;
  final ValueChanged<int> onSelected;

  @override
  Widget build(BuildContext context) {
    return Wrap(
      spacing: 8,
      runSpacing: 8,
      children: [
        for (final color in colors)
          FTappable(
            onPress: () => onSelected(color),
            builder: (context, variants, _) => AnimatedContainer(
              duration: const Duration(milliseconds: 150),
              width: 26,
              height: 26,
              decoration: BoxDecoration(
                color: Color(color),
                shape: BoxShape.circle,
                border: Border.all(
                  color: color == selected
                      ? context.theme.colors.primary
                      : context.theme.colors.border,
                  width: color == selected ? 2.5 : 1,
                ),
                boxShadow: color == selected
                    ? [
                        BoxShadow(
                          color: context.theme.colors.primary.withValues(
                            alpha: .35,
                          ),
                          blurRadius: 8,
                        ),
                      ]
                    : null,
              ),
              child: color == selected
                  ? Icon(FLucideIcons.check, size: 14, color: _contrast(color))
                  : null,
            ),
          ),
      ],
    );
  }

  Color _contrast(int argb) {
    final red = (argb >> 16) & 0xFF;
    final green = (argb >> 8) & 0xFF;
    final blue = argb & 0xFF;
    final luminance = (red * 299 + green * 587 + blue * 114) / 1000;
    return luminance > 150 ? const Color(0xFF111111) : const Color(0xFFFFFFFF);
  }
}
