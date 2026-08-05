import 'package:flutter/material.dart';
import 'package:provider/provider.dart';

import '../../core/page_scaffold.dart';
import '../player/player_view_model.dart';
import 'feedback_dialog.dart';

class SettingsView extends StatefulWidget {
  const SettingsView({super.key});

  @override
  State<SettingsView> createState() => _SettingsViewState();
}

class _SettingsViewState extends State<SettingsView> {
  bool _reduceMotion = false;

  @override
  Widget build(BuildContext context) {
    final viewModel = context.watch<PlayerViewModel>();
    final isAndroid = Theme.of(context).platform == TargetPlatform.android;
    return PageScaffold(
      title: '设置',
      subtitle: '保持安静，也保持可控',
      child: Column(
        children: [
          _SettingSection(
            title: '播放',
            children: [
              SwitchListTile(
                value: viewModel.backgroundPlayback,
                onChanged: viewModel.setBackgroundPlayback,
                title: const Text('关闭后继续播放'),
                subtitle: const Text('关闭窗口时隐藏到系统托盘'),
              ),
              SwitchListTile(
                value: viewModel.floatingLyricsEnabled,
                onChanged: viewModel.floatingLyricsService.isSupported
                    ? viewModel.setFloatingLyrics
                    : null,
                title: Text(isAndroid ? '悬浮窗歌词' : '桌面歌词'),
                subtitle: Text(isAndroid ? '在其他应用上方同步显示歌词' : '在桌面置顶窗口中同步显示歌词'),
              ),
              ListTile(
                title: const Text('播放音量'),
                subtitle: Slider(
                  value: viewModel.volume,
                  onChanged: viewModel.setVolume,
                ),
                trailing: Text('${(viewModel.volume * 100).round()}%'),
              ),
            ],
          ),
          const SizedBox(height: 18),
          _SettingSection(
            title: '外观与辅助功能',
            children: [
              SwitchListTile(
                value: _reduceMotion,
                onChanged: (value) => setState(() => _reduceMotion = value),
                title: const Text('减少动态效果'),
                subtitle: const Text('保留状态反馈，关闭呼吸与大幅转场'),
              ),
              const ListTile(
                title: Text('主题'),
                subtitle: Text('跟随系统'),
                trailing: Icon(Icons.chevron_right),
              ),
            ],
          ),
          const SizedBox(height: 18),
          _SettingSection(
            title: '帮助与反馈',
            children: [
              ListTile(
                leading: const Icon(Icons.feedback_outlined),
                title: const Text('问题反馈'),
                subtitle: const Text('反馈将通过邮件送达开发者'),
                trailing: const Icon(Icons.chevron_right),
                onTap: () => showDialog<void>(
                  context: context,
                  builder: (_) => const FeedbackDialog(),
                ),
              ),
            ],
          ),
          const SizedBox(height: 18),
          const _SettingSection(
            title: '关于',
            children: [
              ListTile(
                title: Text('WCMusic'),
                subtitle: Text('Flutter 3.44 · Rust 1.97 · 洛雪自定义源兼容层'),
              ),
            ],
          ),
        ],
      ),
    );
  }
}

class _SettingSection extends StatelessWidget {
  const _SettingSection({required this.title, required this.children});

  final String title;
  final List<Widget> children;

  @override
  Widget build(BuildContext context) {
    return Container(
      width: double.infinity,
      padding: const EdgeInsets.fromLTRB(8, 16, 8, 8),
      decoration: BoxDecoration(
        color: Theme.of(context).colorScheme.surface,
        borderRadius: BorderRadius.circular(8),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Padding(
            padding: const EdgeInsets.symmetric(horizontal: 12),
            child: Text(title, style: Theme.of(context).textTheme.titleLarge),
          ),
          const SizedBox(height: 6),
          ...children,
        ],
      ),
    );
  }
}
