import 'dart:async';

import 'package:flutter/widgets.dart';
import 'package:forui/forui.dart';
import 'package:provider/provider.dart';

import '../../core/app_theme_controller.dart';
import '../../state/player_view_model.dart';
import '../nav.dart';
import '../pages/home_page.dart';
import '../pages/library_page.dart';
import '../pages/playlists_page.dart';
import '../pages/rankings_page.dart';
import '../pages/search_page.dart';
import '../pages/settings_page.dart';
import '../pages/sources_page.dart';
import '../player/now_playing.dart';
import '../player/player_bar.dart';
import '../widgets/dialogs.dart';
import 'sidebar.dart';
import 'title_bar.dart';

/// 桌面外壳：标题栏 / 侧栏 / 内容 / 播放条。
///
/// 内容切换只做短距离淡入 + 位移；聚焦播放是内容区上的一层，因此侧栏、播放条
/// 不会因为切页而重建。窄窗口退化为单列布局 + 底部导航。
class DesktopShell extends StatefulWidget {
  const DesktopShell({super.key});

  @override
  State<DesktopShell> createState() => _DesktopShellState();
}

class _DesktopShellState extends State<DesktopShell> {
  AppSection _section = AppSection.home;
  bool _nowPlaying = false;
  Timer? _demoTimer;

  /// 只在 `--dart-define=WCMUSIC_AUTO_DEMO=true` 的构建里跑一段自动巡演，
  /// 用来在不占用系统鼠标的情况下采集真实帧耗时（配合 FrameProbe）。
  static const _autoDemo = bool.fromEnvironment('WCMUSIC_AUTO_DEMO');

  @override
  void initState() {
    super.initState();
    if (!_autoDemo) return;
    var step = 0;
    _demoTimer = Timer.periodic(const Duration(milliseconds: 900), (timer) {
      if (!mounted) return;
      final model = context.read<PlayerViewModel>();
      if (step == 0 && model.current == null && model.tracks.isNotEmpty) {
        unawaited(model.playTrack(model.tracks.first));
      }
      const order = [
        AppSection.search,
        AppSection.rankings,
        AppSection.library,
        AppSection.playlists,
        AppSection.sources,
        AppSection.settings,
        AppSection.home,
      ];
      if (step < order.length) {
        _select(order[step]);
      } else if (step == order.length) {
        _openNowPlaying();
      } else if (step == order.length + 3) {
        _closeNowPlaying();
      } else if (step > order.length + 6) {
        timer.cancel();
      }
      step++;
    });
  }

  @override
  void dispose() {
    _demoTimer?.cancel();
    super.dispose();
  }

  void _select(AppSection section) {
    if (_section == section && !_nowPlaying) return;
    setState(() {
      _section = section;
      _nowPlaying = false;
    });
  }

  void _openNowPlaying() {
    if (_nowPlaying) return;
    setState(() => _nowPlaying = true);
  }

  void _closeNowPlaying() => setState(() => _nowPlaying = false);

  Widget _page(AppSection section) => switch (section) {
    AppSection.home => HomePage(onNavigate: _select),
    AppSection.search => const SearchPage(),
    AppSection.rankings => const RankingsPage(),
    AppSection.library => const LibraryPage(),
    AppSection.playlists => const PlaylistsPage(),
    AppSection.sources => const SourcesPage(),
    AppSection.settings => const SettingsPage(),
  };

  @override
  Widget build(BuildContext context) {
    final reduceMotion = context.select<AppThemeController, bool>(
      (theme) => theme.reduceMotion,
    );
    final duration = reduceMotion
        ? Duration.zero
        : const Duration(milliseconds: 240);
    return FToaster(
      child: LayoutBuilder(
        builder: (context, constraints) {
          final desktop = constraints.maxWidth >= 900;
          final content = Stack(
            fit: StackFit.expand,
            children: [
              AnimatedSwitcher(
                duration: duration,
                switchInCurve: Curves.easeOutCubic,
                switchOutCurve: Curves.easeInCubic,
                layoutBuilder: (current, previous) => Stack(
                  fit: StackFit.expand,
                  children: [
                    for (final child in previous)
                      Positioned.fill(
                        child: ExcludeSemantics(child: child),
                      ),
                    if (current != null) Positioned.fill(child: current),
                  ],
                ),
                transitionBuilder: (child, animation) => FadeTransition(
                  opacity: animation,
                  child: SlideTransition(
                    position: Tween<Offset>(
                      begin: const Offset(0, .012),
                      end: Offset.zero,
                    ).animate(
                      CurvedAnimation(
                        parent: animation,
                        curve: Curves.easeOutCubic,
                      ),
                    ),
                    child: child,
                  ),
                ),
                child: RepaintBoundary(
                  key: ValueKey(_section),
                  child: _page(_section),
                ),
              ),
              AnimatedSwitcher(
                duration: reduceMotion
                    ? Duration.zero
                    : const Duration(milliseconds: 300),
                switchInCurve: Curves.easeOutCubic,
                switchOutCurve: Curves.easeInCubic,
                transitionBuilder: (child, animation) => FadeTransition(
                  opacity: animation,
                  child: ScaleTransition(
                    scale: Tween<double>(
                      begin: .985,
                      end: 1,
                    ).animate(animation),
                    child: child,
                  ),
                ),
                child: _nowPlaying
                    ? NowPlayingPanel(
                        key: const ValueKey('now-playing'),
                        onClose: _closeNowPlaying,
                      )
                    : const SizedBox.shrink(key: ValueKey('pages')),
              ),
            ],
          );
          final body = Column(
            children: [
              if (desktop) const DesktopTitleBar(),
              Expanded(
                child: Row(
                  children: [
                    if (desktop)
                      DesktopSidebar(
                        section: _section,
                        onSelect: _select,
                        onOpenNowPlaying: _openNowPlaying,
                        nowPlayingActive: _nowPlaying,
                      ),
                    Expanded(
                      child: FScaffold(
                        childPad: false,
                        child: content,
                      ),
                    ),
                  ],
                ),
              ),
              PlayerBar(onOpenNowPlaying: _openNowPlaying),
              if (!desktop)
                CompactBottomNavigation(
                  section: _section,
                  onSelect: _select,
                ),
            ],
          );
          return SafeArea(
            top: !desktop,
            bottom: false,
            child: ColoredBox(
              color: context.theme.colors.background,
              child: MessageToaster(child: body),
            ),
          );
        },
      ),
    );
  }
}

/// 把 PlayerViewModel.message 转成 Forui toast。
class MessageToaster extends StatefulWidget {
  const MessageToaster({super.key, required this.child});

  final Widget child;

  @override
  State<MessageToaster> createState() => _MessageToasterState();
}

class _MessageToasterState extends State<MessageToaster> {
  PlayerViewModel? _model;
  String? _last;

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    final model = context.read<PlayerViewModel>();
    if (identical(model, _model)) return;
    _model?.removeListener(_onMessage);
    _model = model..addListener(_onMessage);
    _last = model.message;
  }

  @override
  void dispose() {
    _model?.removeListener(_onMessage);
    super.dispose();
  }

  void _onMessage() {
    final message = _model?.message;
    if (message == null || message == _last) return;
    _last = message;
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (!mounted) return;
      final error =
          message.contains('失败') ||
          message.contains('不可用') ||
          message.contains('无法') ||
          message.contains('错误');
      showAppToast(context, message, error: error);
    });
  }

  @override
  Widget build(BuildContext context) => widget.child;
}
