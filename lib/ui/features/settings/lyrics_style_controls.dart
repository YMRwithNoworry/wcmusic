import 'package:flutter/material.dart';
import 'package:provider/provider.dart';

import '../player/player_view_model.dart';

class LyricsStyleControls extends StatelessWidget {
  const LyricsStyleControls({super.key});

  static const _fonts = [
    'Microsoft YaHei',
    'Microsoft YaHei UI',
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

  @override
  Widget build(BuildContext context) {
    final viewModel = context.watch<PlayerViewModel>();
    final style = viewModel.lyricsStyle;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        SwitchListTile(
          value: style.locked,
          onChanged: (value) =>
              viewModel.updateLyricsStyle(style.copyWith(locked: value)),
          title: const Text('锁定歌词窗口'),
          subtitle: const Text('锁定后鼠标穿透，解锁后可拖动、缩放并使用悬浮工具栏'),
        ),
        SwitchListTile(
          value: style.alwaysOnTop,
          onChanged: (value) => viewModel.updateLyricsStyle(
            style.copyWith(alwaysOnTop: value),
          ),
          title: const Text('窗口置顶'),
        ),
        SwitchListTile(
          value: style.zoomActiveLine,
          onChanged: (value) => viewModel.updateLyricsStyle(
            style.copyWith(zoomActiveLine: value),
          ),
          title: const Text('放大当前行'),
        ),
        SwitchListTile(
          value: style.showTranslation,
          onChanged: (value) => viewModel.updateLyricsStyle(
            style.copyWith(showTranslation: value),
          ),
          title: const Text('显示歌词翻译'),
        ),
        ListTile(
          title: const Text('对齐方式'),
          subtitle: SegmentedButton<String>(
            segments: const [
              ButtonSegment(value: 'left', label: Text('左对齐')),
              ButtonSegment(value: 'center', label: Text('居中')),
              ButtonSegment(value: 'right', label: Text('右对齐')),
            ],
            selected: {style.alignment},
            onSelectionChanged: (selection) => viewModel.updateLyricsStyle(
              style.copyWith(alignment: selection.first),
            ),
          ),
        ),
        ListTile(
          title: const Text('字体'),
          subtitle: DropdownButton<String>(
            value: style.fontFamily,
            isExpanded: true,
            items: [
              for (final font in _fonts)
                DropdownMenuItem(value: font, child: Text(font)),
            ],
            onChanged: (font) {
              if (font != null) {
                viewModel.updateLyricsStyle(style.copyWith(fontFamily: font));
              }
            },
          ),
        ),
        ListTile(
          title: const Text('字号'),
          subtitle: Slider(
            value: style.fontSize,
            min: 16,
            max: 48,
            divisions: 32,
            label: style.fontSize.round().toString(),
            onChanged: (value) =>
                viewModel.updateLyricsStyle(style.copyWith(fontSize: value)),
          ),
          trailing: Text('${style.fontSize.round()}'),
        ),
        ListTile(
          title: const Text('文字颜色'),
          subtitle: _ColorRow(
            colors: _textColors,
            selected: style.textColor,
            onSelected: (color) =>
                viewModel.updateLyricsStyle(style.copyWith(textColor: color)),
          ),
        ),
        ListTile(
          title: const Text('背景颜色'),
          subtitle: _ColorRow(
            colors: _backgroundColors,
            selected: style.backgroundColor,
            onSelected: (color) => viewModel.updateLyricsStyle(
              style.copyWith(backgroundColor: color),
            ),
          ),
        ),
        ListTile(
          title: const Text('窗口宽度'),
          subtitle: Slider(
            value: style.windowWidth,
            min: 600,
            max: 1600,
            divisions: 100,
            label: '${style.windowWidth.round()} px',
            onChanged: (value) => viewModel.updateLyricsStyle(
              style.copyWith(windowWidth: value),
            ),
          ),
          trailing: Text('${style.windowWidth.round()}'),
        ),
        ListTile(
          title: const Text('窗口高度'),
          subtitle: Slider(
            value: style.windowHeight,
            min: 100,
            max: 260,
            divisions: 80,
            label: '${style.windowHeight.round()} px',
            onChanged: (value) => viewModel.updateLyricsStyle(
              style.copyWith(windowHeight: value),
            ),
          ),
          trailing: Text('${style.windowHeight.round()}'),
        ),
        ListTile(
          title: const Text('背景不透明度'),
          subtitle: Slider(
            value: style.opacity,
            min: 0.15,
            max: 1,
            onChanged: (value) =>
                viewModel.updateLyricsStyle(style.copyWith(opacity: value)),
          ),
          trailing: Text('${(style.opacity * 100).round()}%'),
        ),
        ListTile(
          title: const Text('圆角大小'),
          subtitle: Slider(
            value: style.cornerRadius,
            min: 0,
            max: 40,
            divisions: 40,
            onChanged: (value) => viewModel.updateLyricsStyle(
              style.copyWith(cornerRadius: value),
            ),
          ),
          trailing: Text('${style.cornerRadius.round()}px'),
        ),
        const Padding(
          padding: EdgeInsets.fromLTRB(16, 4, 16, 12),
          child: Text('拖动位置会自动保存。', style: TextStyle(fontSize: 12)),
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
      spacing: 10,
      runSpacing: 8,
      children: [
        for (final color in colors)
          InkWell(
            onTap: () => onSelected(color),
            borderRadius: BorderRadius.circular(20),
            child: Container(
              width: 32,
              height: 32,
              decoration: BoxDecoration(
                color: Color(color),
                shape: BoxShape.circle,
                border: Border.all(
                  color: color == selected
                      ? Theme.of(context).colorScheme.primary
                      : Theme.of(context).dividerColor,
                  width: color == selected ? 3 : 1,
                ),
              ),
              child: color == selected
                  ? Icon(Icons.check, size: 18, color: _contrast(color))
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
    return luminance > 150 ? Colors.black : Colors.white;
  }
}
