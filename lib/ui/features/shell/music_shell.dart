import 'package:flutter/material.dart';
import 'package:provider/provider.dart';

import '../home/home_view.dart';
import '../library/library_view.dart';
import '../player/now_playing_view.dart';
import '../player/player_bar.dart';
import '../player/player_view_model.dart';
import '../playlists/playlists_view.dart';
import '../settings/settings_view.dart';
import '../search/search_view.dart';
import '../sources/sources_view.dart';

class MusicShell extends StatefulWidget {
  const MusicShell({super.key});

  @override
  State<MusicShell> createState() => _MusicShellState();
}

class _MusicShellState extends State<MusicShell> {
  int _index = 0;

  static const _destinations = [
    (icon: Icons.spa_outlined, selected: Icons.spa, label: '此刻'),
    (icon: Icons.search, selected: Icons.manage_search, label: '搜索'),
    (
      icon: Icons.library_music_outlined,
      selected: Icons.library_music,
      label: '曲库',
    ),
    (
      icon: Icons.queue_music_outlined,
      selected: Icons.queue_music,
      label: '歌单',
    ),
    (icon: Icons.extension_outlined, selected: Icons.extension, label: '音源'),
    (icon: Icons.tune_outlined, selected: Icons.tune, label: '设置'),
  ];

  static const _pages = [
    HomeView(),
    SearchView(),
    LibraryView(),
    PlaylistsView(),
    SourcesView(),
    SettingsView(),
  ];

  @override
  Widget build(BuildContext context) {
    return LayoutBuilder(
      builder: (context, constraints) {
        final desktop = constraints.maxWidth >= 860;
        final wide = constraints.maxWidth >= 1240;
        final viewModel = context.watch<PlayerViewModel>();
        final content = AnimatedSwitcher(
          duration: const Duration(milliseconds: 360),
          switchInCurve: Curves.easeOutCubic,
          switchOutCurve: Curves.easeInCubic,
          transitionBuilder: (child, animation) => FadeTransition(
            opacity: animation,
            child: SlideTransition(
              position: Tween(
                begin: const Offset(.025, 0),
                end: Offset.zero,
              ).animate(animation),
              child: child,
            ),
          ),
          child: KeyedSubtree(key: ValueKey(_index), child: _pages[_index]),
        );

        return Scaffold(
          body: SafeArea(
            bottom: false,
            child: Row(
              children: [
                if (desktop)
                  _DesktopNavigation(index: _index, onChanged: _select),
                Expanded(
                  child: Column(
                    children: [
                      Expanded(child: content),
                      if (viewModel.current != null)
                        PlayerBar(onOpen: () => _openNowPlaying(context)),
                    ],
                  ),
                ),
                if (wide) const _QueuePanel(),
              ],
            ),
          ),
          bottomNavigationBar: desktop
              ? null
              : NavigationBar(
                  selectedIndex: _index,
                  onDestinationSelected: _select,
                  destinations: [
                    for (final destination in _destinations)
                      NavigationDestination(
                        icon: Icon(destination.icon),
                        selectedIcon: Icon(destination.selected),
                        label: destination.label,
                      ),
                  ],
                ),
        );
      },
    );
  }

  void _select(int value) => setState(() => _index = value);

  void _openNowPlaying(BuildContext context) {
    Navigator.of(context).push(
      PageRouteBuilder<void>(
        transitionDuration: const Duration(milliseconds: 420),
        reverseTransitionDuration: const Duration(milliseconds: 320),
        pageBuilder: (_, animation, _) =>
            FadeTransition(opacity: animation, child: const NowPlayingView()),
      ),
    );
  }
}

class _DesktopNavigation extends StatelessWidget {
  const _DesktopNavigation({required this.index, required this.onChanged});

  final int index;
  final ValueChanged<int> onChanged;

  @override
  Widget build(BuildContext context) {
    return Container(
      width: 108,
      padding: const EdgeInsets.fromLTRB(12, 22, 12, 18),
      decoration: BoxDecoration(
        border: Border(
          right: BorderSide(
            color: Theme.of(context).dividerColor.withValues(alpha: .16),
          ),
        ),
      ),
      child: Column(
        children: [
          const _BrandMark(),
          const SizedBox(height: 30),
          Expanded(
            child: NavigationRail(
              selectedIndex: index,
              onDestinationSelected: onChanged,
              labelType: NavigationRailLabelType.all,
              destinations: [
                for (final destination in _MusicShellState._destinations)
                  NavigationRailDestination(
                    icon: Icon(destination.icon),
                    selectedIcon: Icon(destination.selected),
                    label: Text(destination.label),
                  ),
              ],
            ),
          ),
        ],
      ),
    );
  }
}

class _BrandMark extends StatelessWidget {
  const _BrandMark();

  @override
  Widget build(BuildContext context) {
    return Semantics(
      label: 'WCMusic',
      child: Container(
        width: 48,
        height: 48,
        decoration: BoxDecoration(
          color: Theme.of(context).colorScheme.primary,
          borderRadius: BorderRadius.circular(8),
        ),
        child: const Icon(Icons.graphic_eq, color: Colors.white),
      ),
    );
  }
}

class _QueuePanel extends StatelessWidget {
  const _QueuePanel();

  @override
  Widget build(BuildContext context) {
    final viewModel = context.watch<PlayerViewModel>();
    return Container(
      width: 320,
      padding: const EdgeInsets.fromLTRB(24, 30, 18, 24),
      decoration: BoxDecoration(
        border: Border(
          left: BorderSide(
            color: Theme.of(context).dividerColor.withValues(alpha: .16),
          ),
        ),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text('接下来', style: Theme.of(context).textTheme.titleLarge),
          const SizedBox(height: 18),
          Expanded(
            child: ReorderableListView.builder(
              itemCount: viewModel.tracks.length,
              onReorderItem: viewModel.reorderTracks,
              buildDefaultDragHandles: false,
              itemBuilder: (context, index) {
                final track = viewModel.tracks[index];
                return ListTile(
                  key: ValueKey(track.id),
                  contentPadding: EdgeInsets.zero,
                  dense: true,
                  onTap: () => viewModel.playTrack(track),
                  title: Text(
                    track.title,
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                  ),
                  subtitle: Text(
                    track.artist,
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                  ),
                  trailing: ReorderableDragStartListener(
                    index: index,
                    child: const Tooltip(
                      message: '拖动排序',
                      child: Icon(Icons.drag_indicator, size: 19),
                    ),
                  ),
                );
              },
            ),
          ),
        ],
      ),
    );
  }
}
