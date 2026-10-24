import 'package:flutter/widgets.dart';
import 'package:forui/forui.dart';

/// 桌面端主导航。分组让 7 个入口更好扫读。
enum AppSection { home, search, rankings, library, playlists, sources, settings }

extension AppSectionInfo on AppSection {
  String get label => switch (this) {
    AppSection.home => '发现音乐',
    AppSection.search => '搜索音乐',
    AppSection.rankings => '排行榜',
    AppSection.library => '曲库',
    AppSection.playlists => '我的歌单',
    AppSection.sources => '音源管理',
    AppSection.settings => '设置',
  };

  String get shortLabel => switch (this) {
    AppSection.home => '发现',
    AppSection.search => '搜索',
    AppSection.rankings => '榜单',
    AppSection.library => '曲库',
    AppSection.playlists => '歌单',
    AppSection.sources => '音源',
    AppSection.settings => '设置',
  };

  String get caption => switch (this) {
    AppSection.home => '每日发现与新曲',
    AppSection.search => '跨平台找歌',
    AppSection.rankings => '实时榜单',
    AppSection.library => '本地收藏与文件夹',
    AppSection.playlists => '收藏的平台歌单',
    AppSection.sources => '洛雪兼容音源',
    AppSection.settings => '外观、歌词与播放',
  };

  IconData get icon => switch (this) {
    AppSection.home => FLucideIcons.compass,
    AppSection.search => FLucideIcons.search,
    AppSection.rankings => FLucideIcons.trophy,
    AppSection.library => FLucideIcons.library,
    AppSection.playlists => FLucideIcons.listMusic,
    AppSection.sources => FLucideIcons.plug,
    AppSection.settings => FLucideIcons.settings,
  };
}

const browseSections = [AppSection.home, AppSection.search, AppSection.rankings];
const librarySections = [
  AppSection.library,
  AppSection.playlists,
  AppSection.sources,
];
const systemSections = [AppSection.settings];

const compactSections = [
  AppSection.home,
  AppSection.search,
  AppSection.rankings,
  AppSection.library,
];
