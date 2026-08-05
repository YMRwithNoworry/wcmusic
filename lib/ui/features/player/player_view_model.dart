import 'dart:async';
import 'dart:math';

import 'package:flutter/foundation.dart';

import '../../../data/services/player_service.dart';
import '../../../data/services/online_search_service.dart';
import '../../../data/services/desktop_window_service.dart';
import '../../../data/services/floating_lyrics_service.dart';
import '../../../data/services/lyric_service.dart';
import '../../../data/services/track_download_service.dart';
import '../../../domain/models/lyric_line.dart';
import '../../../domain/models/playback_mode.dart';
import '../../../domain/models/playback_quality.dart';
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
    LyricService? lyricService,
    FloatingLyricsService? floatingLyricsService,
    TrackDownloadService? downloadService,
  }) : onlineSearchService =
           onlineSearchService ?? MultiSourceOnlineSearchService(),
       playerService = playerService ?? PlayerService(),
       lyricService = lyricService ?? OnlineLyricService(),
       floatingLyricsService =
           floatingLyricsService ?? PlatformFloatingLyricsService(),
       downloadService = downloadService ?? TrackDownloadService() {
    _playingSubscription = this.playerService.playing.listen((value) {
      isPlaying = value;
      notifyListeners();
    });
    _positionSubscription = this.playerService.position.listen((value) {
      position = value;
      _syncLyrics(value);
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
    _completedSubscription = this.playerService.completed.listen((_) {
      unawaited(_handleCompleted());
    });
  }

  final MusicRepository musicRepository;
  final SourceRepository sourceRepository;
  final OnlineSearchService onlineSearchService;
  final WindowLifecycleService? windowLifecycleService;
  final AudioPlayerService playerService;
  final LyricService lyricService;
  final FloatingLyricsService floatingLyricsService;
  final TrackDownloadService downloadService;
  List<Track> tracks = const [];
  List<Playlist> playlists = const [];
  List<SourceScript> sources = const [];
  String? selectedSourceId;
  Track? current;
  bool isLoading = true;
  bool isPlaying = false;
  Duration position = Duration.zero;
  Duration duration = Duration.zero;
  Duration buffered = Duration.zero;
  double volume = 1;
  double _volumeBeforeMute = .8;
  String? message;
  bool floatingLyricsEnabled = false;
  PlaybackMode playbackMode = PlaybackMode.listLoop;
  PlaybackQuality playbackQuality = PlaybackQuality.high;
  bool isDownloading = false;
  double? downloadProgress;
  List<LyricLine> lyrics = const [];
  int currentLyricIndex = -1;
  late bool backgroundPlayback = windowLifecycleService?.closeToTray ?? true;
  String query = '';
  String onlineQuery = '';
  OnlineSearchChannel onlineSearchChannel = OnlineSearchChannel.kuwo;
  List<Track> onlineResults = const [];
  List<PlatformPlaylist> platformPlaylists = const [];
  List<PlatformPlaylist> _platformPlaylistCatalog = const [];
  final Set<String> loadingPlatformPlaylistIds = {};
  List<Track> recentTracks = const [];
  List<Track> _recentTrackCatalog = const [];
  bool isLoadingPlatformPlaylists = false;
  String? platformPlaylistError;
  bool isLoadingRecentTracks = false;
  String? recentTracksError;
  bool isSearchingOnline = false;
  String? onlineSearchError;
  int _searchGeneration = 0;
  int _lyricGeneration = 0;
  StreamSubscription<bool>? _playingSubscription;
  StreamSubscription<Duration>? _positionSubscription;
  StreamSubscription<Duration>? _durationSubscription;
  StreamSubscription<double>? _volumeSubscription;
  StreamSubscription<void>? _completedSubscription;

  Duration get playbackDuration =>
      duration > Duration.zero ? duration : current?.duration ?? Duration.zero;

  SourceScript? get selectedSource {
    for (final source in sources) {
      if (source.id == selectedSourceId) return source;
    }
    return null;
  }

  Set<String> get _activeSourceKeys {
    final selected = selectedSource;
    if (selected != null) return selected.sourceKeys.toSet();
    return sources.expand((source) => source.sourceKeys).toSet();
  }

  Future<void> load() async {
    isLoading = true;
    notifyListeners();
    try {
      tracks = await musicRepository.loadTracks();
      playlists = await musicRepository.loadPlaylists();
      sources = await sourceRepository.loadSources();
      selectedSourceId = await sourceRepository.loadSelectedSourceId();
      unawaited(refreshPlatformPlaylists());
      unawaited(refreshRecentTracks());
      unawaited(_upgradeEmptyPlatformFavorites());
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

  String _platformPlaylistId(PlatformPlaylist playlist) =>
      'platform-${playlist.platform}-${playlist.id}';

  bool isPlatformPlaylistFavorite(PlatformPlaylist playlist) =>
      playlists.any((item) => item.id == _platformPlaylistId(playlist));

  bool isPlatformPlaylistLoading(PlatformPlaylist playlist) =>
      loadingPlatformPlaylistIds.contains(_platformPlaylistId(playlist));

  Future<void> togglePlatformPlaylistFavorite(PlatformPlaylist playlist) async {
    final id = _platformPlaylistId(playlist);
    if (loadingPlatformPlaylistIds.contains(id)) return;
    try {
      if (isPlatformPlaylistFavorite(playlist)) {
        await musicRepository.deletePlaylist(id);
        message = '已取消收藏 ${playlist.name}';
      } else {
        loadingPlatformPlaylistIds.add(id);
        notifyListeners();
        final tracks = await onlineSearchService.discoverPlaylistTracks(
          playlist,
        );
        if (tracks.isEmpty) throw StateError('歌单暂无可播放歌曲');
        await musicRepository.savePlaylist(
          Playlist(
            id: id,
            name: playlist.name,
            tracks: tracks,
            artworkUri: playlist.artworkUri,
            externalUrl: playlist.url,
            platform: playlist.platform,
          ),
        );
        message = '已收藏 ${playlist.name}，共 ${tracks.length} 首可播放歌曲';
      }
      playlists = await musicRepository.loadPlaylists();
    } on Object catch (error) {
      message = '更新歌单收藏失败：$error';
    } finally {
      loadingPlatformPlaylistIds.remove(id);
    }
    notifyListeners();
  }

  Future<void> _upgradeEmptyPlatformFavorites() async {
    final pending = playlists
        .where(
          (playlist) => playlist.isPlatformFavorite && playlist.tracks.isEmpty,
        )
        .toList(growable: false);
    for (final saved in pending) {
      final url = saved.externalUrl;
      if (url == null) continue;
      final uri = Uri.tryParse(url);
      if (uri == null || uri.pathSegments.isEmpty) continue;
      final platformPlaylist = PlatformPlaylist(
        id: uri.pathSegments.last,
        name: saved.name,
        artworkUri: saved.artworkUri ?? '',
        url: url,
        platform: saved.platform ?? '网易云音乐',
      );
      loadingPlatformPlaylistIds.add(saved.id);
      notifyListeners();
      try {
        final tracks = await onlineSearchService.discoverPlaylistTracks(
          platformPlaylist,
        );
        if (tracks.isEmpty) continue;
        final latest = await musicRepository.loadPlaylists();
        if (!latest.any((playlist) => playlist.id == saved.id)) continue;
        await musicRepository.savePlaylist(
          Playlist(
            id: saved.id,
            name: saved.name,
            tracks: tracks,
            artworkUri: saved.artworkUri,
            externalUrl: saved.externalUrl,
            platform: saved.platform,
          ),
        );
        playlists = await musicRepository.loadPlaylists();
        notifyListeners();
      } on Object {
        continue;
      } finally {
        loadingPlatformPlaylistIds.remove(saved.id);
        notifyListeners();
      }
    }
  }

  Future<void> removeSavedPlaylist(Playlist playlist) async {
    try {
      await musicRepository.deletePlaylist(playlist.id);
      playlists = await musicRepository.loadPlaylists();
      message = '已取消收藏 ${playlist.name}';
    } on Object catch (error) {
      message = '取消歌单收藏失败：$error';
    }
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
    message = null;
    notifyListeners();

    var playbackTrack = track;
    if (track.source != TrackSource.local && sources.isNotEmpty) {
      try {
        final sourceKeys = _activeSourceKeys;
        if (!sourceKeys.contains(playbackTrack.source.name)) {
          final matched = await onlineSearchService.matchTrackToSources(
            track,
            sourceKeys,
          );
          if (matched != null) playbackTrack = matched;
        }
        if (sourceKeys.contains(playbackTrack.source.name)) {
          message = '正在通过洛雪音源解析整曲...';
          notifyListeners();
          final url = await sourceRepository.resolveUrl(
            playbackTrack,
            quality: playbackQuality.sourceValue,
          );
          playbackTrack = track.copyWith(
            uri: url,
            source: playbackTrack.source,
            sourceId: playbackTrack.sourceId,
            quality: '洛雪音源 · ${playbackQuality.label}',
          );
          current = playbackTrack;
          message = null;
        }
      } on Object {
        message = track.uri.isEmpty ? '整曲解析失败，该歌曲暂无播放地址' : '整曲解析失败，已回退平台试听';
        playbackTrack = track;
        current = track;
      }
    }

    if (playbackTrack.uri.isEmpty) {
      message ??= playbackTrack.source == TrackSource.local
          ? '本地歌曲文件不可用'
          : '该歌曲暂无可用的播放地址';
    } else {
      await playerService.play(playbackTrack);
      isPlaying = true;
      if (floatingLyricsEnabled) unawaited(_loadLyrics(playbackTrack));
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

  void cyclePlaybackMode() {
    playbackMode = PlaybackMode
        .values[(playbackMode.index + 1) % PlaybackMode.values.length];
    notifyListeners();
  }

  Future<void> setPlaybackQuality(PlaybackQuality quality) async {
    if (quality == playbackQuality) return;
    playbackQuality = quality;
    message = '播放音质已切换为 ${quality.label}';
    notifyListeners();
    final track = current;
    if (track != null &&
        track.source != TrackSource.local &&
        track.uri.isNotEmpty) {
      await playTrack(track);
    }
  }

  Future<void> downloadCurrentTrack() async {
    final track = current;
    if (track == null || track.uri.isEmpty) {
      message = '当前没有可下载的音频地址';
      notifyListeners();
      return;
    }
    if (isDownloading) return;
    isDownloading = true;
    downloadProgress = null;
    notifyListeners();
    try {
      final path = await downloadService.download(
        track,
        fallbackExtension: playbackQuality.sourceValue == 'flac'
            ? '.flac'
            : '.mp3',
        onProgress: (progress) {
          downloadProgress = progress;
          notifyListeners();
        },
      );
      message = '已下载到 $path';
    } on Object catch (error) {
      message = '下载失败：$error';
    } finally {
      isDownloading = false;
      downloadProgress = null;
      notifyListeners();
    }
  }

  Future<void> playNext() => _advance(auto: false);

  Future<void> playPrevious() async {
    if (tracks.isEmpty || current == null) return;
    final index = _currentIndex;
    if (index < 0) return;
    await playTrack(tracks[(index - 1 + tracks.length) % tracks.length]);
  }

  Future<void> _handleCompleted() async {
    if (current == null) return;
    if (playbackMode == PlaybackMode.singleLoop) {
      await _replayCurrent();
      return;
    }
    await _advance(auto: true);
  }

  Future<void> _advance({required bool auto}) async {
    if (tracks.isEmpty || current == null) return;
    final index = _currentIndex;
    final Track? next;
    if (playbackMode == PlaybackMode.shuffle) {
      next = _randomTrack(index);
    } else {
      final candidate = index + 1;
      if (candidate < tracks.length) {
        next = tracks[candidate];
      } else if (playbackMode == PlaybackMode.listLoop || !auto) {
        next = tracks.first;
      } else {
        next = null;
      }
    }
    if (next == null) {
      await playerService.stop();
      isPlaying = false;
      position = Duration.zero;
      notifyListeners();
      return;
    }
    await playTrack(next);
  }

  Track? _randomTrack(int exceptIndex) {
    if (tracks.isEmpty) return null;
    if (tracks.length == 1) return tracks.first;
    final candidates = [
      for (var index = 0; index < tracks.length; index++)
        if (index != exceptIndex) tracks[index],
    ];
    if (candidates.isEmpty) return tracks.first;
    return candidates[Random().nextInt(candidates.length)];
  }

  Future<void> _replayCurrent() async {
    final track = current;
    if (track == null || track.uri.isEmpty) return;
    await playerService.seek(Duration.zero);
    await playerService.play(track);
    isPlaying = true;
    notifyListeners();
  }

  int get _currentIndex =>
      tracks.indexWhere((track) => track.id == current?.id);

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

  Future<void> setFloatingLyrics(bool value) async {
    try {
      final enabled = await floatingLyricsService.setEnabled(value);
      if (value && !enabled) {
        floatingLyricsEnabled = false;
        message = '请在系统设置中允许 WCMusic 显示悬浮窗，然后再次启用歌词';
      } else {
        floatingLyricsEnabled = value;
        message = value ? '歌词浮层已开启' : '歌词浮层已关闭';
        if (value) {
          final track = current;
          if (track == null) {
            await floatingLyricsService.update(
              title: 'WCMusic',
              currentLine: '播放歌曲后将在这里显示歌词',
              nextLine: '',
            );
          } else {
            await _loadLyrics(track);
          }
        } else {
          _lyricGeneration++;
        }
      }
    } on Object catch (error) {
      floatingLyricsEnabled = false;
      message = '开启歌词浮层失败：$error';
    }
    notifyListeners();
  }

  Future<void> _loadLyrics(Track track) async {
    final generation = ++_lyricGeneration;
    lyrics = const [];
    currentLyricIndex = -1;
    await floatingLyricsService.update(
      title: '${track.title} · ${track.artist}',
      currentLine: '正在加载歌词...',
      nextLine: '',
    );
    try {
      var lyricTrack = track;
      if (track.source == TrackSource.local ||
          track.source == TrackSource.custom) {
        final matched = await onlineSearchService.matchTrackToSources(
          track,
          const {'wy'},
        );
        if (matched != null) lyricTrack = matched;
      }
      if (generation != _lyricGeneration || current?.id != track.id) return;
      final loaded = await lyricService.loadLyrics(lyricTrack);
      if (generation != _lyricGeneration || current?.id != track.id) return;
      lyrics = loaded;
      if (loaded.isEmpty) {
        await floatingLyricsService.update(
          title: '${track.title} · ${track.artist}',
          currentLine: '暂无歌词',
          nextLine: '',
        );
      } else {
        _syncLyrics(position, force: true);
      }
    } on Object {
      if (generation != _lyricGeneration || current?.id != track.id) return;
      await floatingLyricsService.update(
        title: '${track.title} · ${track.artist}',
        currentLine: '歌词暂时不可用',
        nextLine: '',
      );
    }
    notifyListeners();
  }

  void _syncLyrics(Duration value, {bool force = false}) {
    if (!floatingLyricsEnabled || lyrics.isEmpty) return;
    var nextIndex = -1;
    for (var index = 0; index < lyrics.length; index++) {
      if (lyrics[index].time > value) break;
      nextIndex = index;
    }
    if (!force && nextIndex == currentLyricIndex) return;
    currentLyricIndex = nextIndex;
    final track = current;
    if (track == null) return;
    final currentLine = nextIndex < 0 ? track.title : lyrics[nextIndex].text;
    final followingIndex = nextIndex + 1;
    final nextLine = followingIndex >= 0 && followingIndex < lyrics.length
        ? lyrics[followingIndex].text
        : '';
    unawaited(
      floatingLyricsService.update(
        title: '${track.title} · ${track.artist}',
        currentLine: currentLine,
        nextLine: nextLine,
      ),
    );
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

  Future<void> selectSource(String? id) async {
    String? name;
    if (id != null) {
      for (final source in sources) {
        if (source.id == id) {
          name = source.name;
          break;
        }
      }
    }
    try {
      await sourceRepository.selectSource(id);
      selectedSourceId = id;
      message = id == null ? '已恢复自动选择所有音源' : '已选择 $name 用于整曲解析';
    } on Object catch (error) {
      message = '选择音源失败：$error';
    }
    notifyListeners();
  }

  Future<void> deleteSource(String id) async {
    try {
      await sourceRepository.deleteSource(id);
      sources = await sourceRepository.loadSources();
      selectedSourceId = await sourceRepository.loadSelectedSourceId();
      message = '已删除音源';
    } on Object catch (error) {
      message = '删除失败：$error';
    }
    notifyListeners();
  }

  @override
  void dispose() {
    _lyricGeneration++;
    unawaited(_playingSubscription?.cancel());
    unawaited(_positionSubscription?.cancel());
    unawaited(_durationSubscription?.cancel());
    unawaited(_volumeSubscription?.cancel());
    unawaited(_completedSubscription?.cancel());
    unawaited(playerService.dispose());
    unawaited(floatingLyricsService.dispose());
    super.dispose();
  }
}
