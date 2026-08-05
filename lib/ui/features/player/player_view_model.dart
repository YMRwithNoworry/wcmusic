import 'dart:async';
import 'dart:math';

import 'package:flutter/foundation.dart';

import '../../../data/services/player_service.dart';
import '../../../data/services/online_search_service.dart';
import '../../../data/services/desktop_window_service.dart';
import '../../../domain/models/track.dart';
import '../../../domain/repositories/music_repository.dart';
import '../../../domain/repositories/source_repository.dart';

class PlayerViewModel extends ChangeNotifier {
  PlayerViewModel({
    required this.musicRepository,
    required this.sourceRepository,
    OnlineSearchService? onlineSearchService,
    this.windowLifecycleService,
    AudioPlayerService? playerService,
  }) : onlineSearchService = onlineSearchService ?? AppleOnlineSearchService(),
       playerService = playerService ?? PlayerService() {
    _playingSubscription = this.playerService.playing.listen((value) {
      isPlaying = value;
      notifyListeners();
    });
    _positionSubscription = this.playerService.position.listen((value) {
      position = value;
      notifyListeners();
    });
    _durationSubscription = this.playerService.duration.listen((value) {
      duration = value > Duration.zero
          ? value
          : current?.duration ?? Duration.zero;
      notifyListeners();
    });
    _volumeSubscription = this.playerService.volume.listen((value) {
      volume = value.clamp(0.0, 1.0);
      notifyListeners();
    });
  }

  final MusicRepository musicRepository;
  final SourceRepository sourceRepository;
  final OnlineSearchService onlineSearchService;
  final WindowLifecycleService? windowLifecycleService;
  final AudioPlayerService playerService;
  List<Track> tracks = const [];
  List<Playlist> playlists = const [];
  List<SourceScript> sources = const [];
  Track? current;
  bool isLoading = true;
  bool isPlaying = false;
  Duration position = Duration.zero;
  Duration duration = Duration.zero;
  Duration buffered = Duration.zero;
  double volume = 1;
  double _volumeBeforeMute = .8;
  String? message;
  late bool backgroundPlayback = windowLifecycleService?.closeToTray ?? true;
  String query = '';
  String onlineQuery = '';
  OnlineSearchChannel onlineSearchChannel = OnlineSearchChannel.appleMusic;
  List<Track> onlineResults = const [];
  List<PlatformPlaylist> platformPlaylists = const [];
  List<PlatformPlaylist> _platformPlaylistCatalog = const [];
  List<Track> recentTracks = const [];
  List<Track> _recentTrackCatalog = const [];
  bool isLoadingPlatformPlaylists = false;
  String? platformPlaylistError;
  bool isLoadingRecentTracks = false;
  String? recentTracksError;
  bool isSearchingOnline = false;
  String? onlineSearchError;
  int _searchGeneration = 0;
  StreamSubscription<bool>? _playingSubscription;
  StreamSubscription<Duration>? _positionSubscription;
  StreamSubscription<Duration>? _durationSubscription;
  StreamSubscription<double>? _volumeSubscription;

  Duration get playbackDuration =>
      duration > Duration.zero ? duration : current?.duration ?? Duration.zero;

  Future<void> load() async {
    isLoading = true;
    notifyListeners();
    try {
      tracks = await musicRepository.loadTracks();
      playlists = await musicRepository.loadPlaylists();
      sources = await sourceRepository.loadSources();
      unawaited(refreshPlatformPlaylists());
      unawaited(refreshRecentTracks());
    } on Object catch (error) {
      message = '载入音乐数据失败：$error';
    } finally {
      isLoading = false;
      notifyListeners();
    }
  }

  Future<void> refreshPlatformPlaylists() async {
    isLoadingPlatformPlaylists = true;
    platformPlaylistError = null;
    notifyListeners();
    try {
      _platformPlaylistCatalog = await onlineSearchService.discoverPlaylists();
      shufflePlatformPlaylists();
    } on Object catch (_) {
      platformPlaylistError = '平台歌单暂时不可用';
    } finally {
      isLoadingPlatformPlaylists = false;
      notifyListeners();
    }
  }

  void shufflePlatformPlaylists() {
    final shuffled = [..._platformPlaylistCatalog];
    if (shuffled.length > 4) shuffled.shuffle(Random());
    platformPlaylists = shuffled.take(4).toList(growable: false);
    notifyListeners();
  }

  Future<void> refreshRecentTracks() async {
    isLoadingRecentTracks = true;
    recentTracksError = null;
    notifyListeners();
    try {
      _recentTrackCatalog = await onlineSearchService.discoverNewTracks();
      shuffleRecentTracks();
    } on Object catch (_) {
      recentTracksError = '新曲推荐暂时不可用';
    } finally {
      isLoadingRecentTracks = false;
      notifyListeners();
    }
  }

  void shuffleRecentTracks() {
    final shuffled = [..._recentTrackCatalog];
    if (shuffled.length > 4) shuffled.shuffle(Random());
    recentTracks = shuffled.take(4).toList(growable: false);
    notifyListeners();
  }

  List<Track> get visibleTracks {
    final needle = query.trim().toLowerCase();
    if (needle.isEmpty) return tracks;
    return tracks
        .where(
          (track) => '${track.title} ${track.artist} ${track.album}'
              .toLowerCase()
              .contains(needle),
        )
        .toList();
  }

  Future<void> playTrack(Track track) async {
    current = track;
    position = Duration.zero;
    duration = track.duration;
    message = track.uri.isEmpty
        ? track.source == TrackSource.local
              ? '本地歌曲文件不可用'
              : '该歌曲暂无可用的在线试听地址'
        : null;
    if (track.uri.isNotEmpty) {
      await playerService.play(track);
      isPlaying = true;
    }
    notifyListeners();
  }

  Future<void> togglePlayback() async {
    if (current == null) return;
    if (current!.uri.isEmpty) {
      message = '演示曲目没有音频地址';
      notifyListeners();
      return;
    }
    final shouldPlay = !isPlaying;
    await playerService.toggle();
    isPlaying = shouldPlay;
    notifyListeners();
  }

  Future<void> seek(double value) async {
    final maximum = playbackDuration.inMilliseconds;
    if (maximum <= 0) return;
    final target = Duration(milliseconds: value.round().clamp(0, maximum));
    position = target;
    await playerService.seek(target);
    notifyListeners();
  }

  Future<void> setVolume(double value) async {
    final next = value.clamp(0.0, 1.0);
    if (next > .001) _volumeBeforeMute = next;
    volume = next;
    notifyListeners();
    await playerService.setVolume(next);
  }

  Future<void> toggleMute() =>
      setVolume(volume <= .001 ? _volumeBeforeMute : 0);

  Future<void> setBackgroundPlayback(bool value) async {
    backgroundPlayback = value;
    notifyListeners();
    try {
      await windowLifecycleService?.setCloseToTray(value);
      message = value ? '关闭窗口后将在托盘继续播放' : '关闭窗口将退出 WCMusic';
    } on Object catch (error) {
      backgroundPlayback = !value;
      message = '更新后台播放设置失败：$error';
    }
    notifyListeners();
  }

  void setQuery(String value) {
    query = value;
    notifyListeners();
  }

  Future<void> searchOnline(String value) async {
    final keyword = value.trim();
    onlineQuery = keyword;
    final generation = ++_searchGeneration;
    if (keyword.isEmpty) {
      onlineResults = const [];
      onlineSearchError = null;
      isSearchingOnline = false;
      notifyListeners();
      return;
    }

    isSearchingOnline = true;
    onlineSearchError = null;
    notifyListeners();
    try {
      final results = await onlineSearchService.search(
        keyword,
        channel: onlineSearchChannel,
      );
      if (generation != _searchGeneration) return;
      onlineResults = results;
    } on Object catch (error) {
      if (generation != _searchGeneration) return;
      onlineResults = const [];
      onlineSearchError = '搜索失败，请检查网络后重试：$error';
    } finally {
      if (generation == _searchGeneration) {
        isSearchingOnline = false;
        notifyListeners();
      }
    }
  }

  Future<void> selectOnlineSearchChannel(OnlineSearchChannel channel) async {
    if (channel == onlineSearchChannel) return;
    onlineSearchChannel = channel;
    notifyListeners();
    if (onlineQuery.isNotEmpty) await searchOnline(onlineQuery);
  }

  Future<void> importPlaylist(String path, List<int> bytes) async {
    try {
      await musicRepository.importFile(path, bytes);
      await load();
      message = '已导入歌单';
    } on FormatException catch (error) {
      message = error.message;
    } on Object catch (error) {
      message = '导入失败：$error';
    }
    notifyListeners();
  }

  Future<void> importLocalAudio(List<String> paths) async {
    if (paths.isEmpty) return;
    try {
      final imported = await musicRepository.importAudioFiles(paths);
      tracks = await musicRepository.loadTracks();
      message = '已复制并添加 ${imported.length} 首本地音乐';
    } on Object catch (error) {
      message = '导入本地音乐失败：$error';
    }
    notifyListeners();
  }

  Future<void> reorderTracks(int oldIndex, int newIndex) async {
    final reordered = [...tracks];
    final track = reordered.removeAt(oldIndex);
    reordered.insert(newIndex, track);
    tracks = reordered;
    await musicRepository.saveTracks(tracks);
    notifyListeners();
  }

  Future<void> importSource(String rawScript) async {
    try {
      final source = await sourceRepository.importScript(rawScript);
      sources = await sourceRepository.loadSources();
      message = '已启用 ${source.name}';
    } on FormatException catch (error) {
      message = error.message;
    } on Object catch (error) {
      message = '导入失败：$error';
    }
    notifyListeners();
  }

  Future<void> deleteSource(String id) async {
    try {
      await sourceRepository.deleteSource(id);
      sources = await sourceRepository.loadSources();
      message = '已删除音源';
    } on Object catch (error) {
      message = '删除失败：$error';
    }
    notifyListeners();
  }

  @override
  void dispose() {
    unawaited(_playingSubscription?.cancel());
    unawaited(_positionSubscription?.cancel());
    unawaited(_durationSubscription?.cancel());
    unawaited(_volumeSubscription?.cancel());
    unawaited(playerService.dispose());
    super.dispose();
  }
}
