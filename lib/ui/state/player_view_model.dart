import 'dart:async';
import 'dart:math';

import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';
import 'package:package_info_plus/package_info_plus.dart';

import '../../data/services/audio_effect_settings_store.dart';
import '../../data/services/player_service.dart';
import '../../data/services/online_search_service.dart';
import '../../data/services/release_service.dart';
import '../../data/services/desktop_window_service.dart';
import '../../data/services/floating_lyrics_service.dart';
import '../../data/services/lyric_service.dart';
import '../../data/services/track_download_service.dart';
import '../../domain/models/lyric_line.dart';
import '../../domain/models/lyrics_overlay_style.dart';
import '../../domain/models/music_folder.dart';
import '../../domain/models/playback_mode.dart';
import '../../domain/models/playback_quality.dart';
import '../../domain/models/release_info.dart';
import '../../domain/models/source_import_result.dart';
import '../../domain/models/track.dart';
import '../../domain/repositories/music_repository.dart';
import '../../domain/repositories/source_repository.dart';

class PlayerViewModel extends ChangeNotifier {
  /// 调试构建下保留最近创建的实例，便于用 VM Service 现场排查（release 下为 null）。
  static PlayerViewModel? debugLatest;

  PlayerViewModel({
    required this.musicRepository,
    required this.sourceRepository,
    OnlineSearchService? onlineSearchService,
    this.windowLifecycleService,
    AudioPlayerService? playerService,
    LyricService? lyricService,
    FloatingLyricsService? floatingLyricsService,
    ReleaseService? releaseService,
    TrackDownloadService? downloadService,
    AudioEffectSettingsStore? audioEffectSettingsStore,
  }) : onlineSearchService =
           onlineSearchService ?? MultiSourceOnlineSearchService(),
       playerService = playerService ?? PlayerService(),
       audioEffectSettingsStore =
           audioEffectSettingsStore ?? FileAudioEffectSettingsStore(),
       lyricService = lyricService ?? OnlineLyricService(),
       floatingLyricsService =
           floatingLyricsService ?? PlatformFloatingLyricsService(),
       releaseService = releaseService ?? GitHubReleaseService(),
       downloadService = downloadService ?? TrackDownloadService() {
    if (kDebugMode) debugLatest = this;
    _playingSubscription = this.playerService.playing.listen((value) {
      isPlaying = value;
      notifyListeners();
    });
    _positionSubscription = this.playerService.position.listen((value) {
      position = value;
      // 拖动进度条期间 mpv 可能还在上报旧位置，以拖动目标为准，避免滑块回弹
      if (_pendingSeek != null) return;
      positionListenable.value = value;
      _syncLyrics(value);
    });
    _durationSubscription = this.playerService.duration.listen((value) {
      final next = value > Duration.zero
          ? value
          : current?.duration ?? Duration.zero;
      if (next == duration) return;
      duration = next;
      durationListenable.value = duration;
      notifyListeners();
    });
    _volumeSubscription = this.playerService.volume.listen((value) {
      volume = value.clamp(0.0, 1.0);
      volumeListenable.value = volume;
      notifyListeners();
    });
    _completedSubscription = this.playerService.completed.listen((_) {
      unawaited(_handleCompleted());
    });
    _errorSubscription = this.playerService.errors.listen((error) {
      message = '播放失败：$error';
      notifyListeners();
    });
  }

  final MusicRepository musicRepository;
  final SourceRepository sourceRepository;
  final OnlineSearchService onlineSearchService;
  final WindowLifecycleService? windowLifecycleService;
  final AudioPlayerService playerService;
  final AudioEffectSettingsStore audioEffectSettingsStore;
  final LyricService lyricService;
  final FloatingLyricsService floatingLyricsService;
  final ReleaseService releaseService;

  /// 当前运行版本，例如 1.2.82；读取失败时为空
  String? appVersion;

  /// 最近一次检测到的发布记录（不论是否比当前版本新）
  ReleaseInfo? latestRelease;

  /// 有更新时指向 GitHub 上的最新发布
  ReleaseInfo? availableUpdate;
  bool isCheckingUpdate = false;
  bool isInstallingUpdate = false;

  /// 一键更新下载进度 0~1，大小未知时为空
  double? updateDownloadProgress;
  String? updateError;
  DateTime? lastUpdateCheck;
  final TrackDownloadService downloadService;
  List<Track> tracks = const [];
  List<Playlist> playlists = const [];
  List<MusicFolder> folders = const [];
  String? selectedFolderId;
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
  bool spatialAudioEnabled = false;
  LyricsOverlayStyle lyricsStyle = const LyricsOverlayStyle();

  /// 系统已安装字体（来自原生端），为空时界面使用内置字体列表
  List<String> systemFonts = const [];

  /// 系统默认界面字体，用于「系统默认」项与内置默认字体的显示名
  String? defaultSystemFont;
  PlaybackMode playbackMode = PlaybackMode.listLoop;
  PlaybackQuality playbackQuality = PlaybackQuality.high;
  bool isDownloading = false;
  double? downloadProgress;
  List<LyricLine> lyrics = const [];
  int currentLyricIndex = -1;
  late bool backgroundPlayback = windowLifecycleService?.closeToTray ?? true;
  String query = '';
  String onlineQuery = '';
  int onlineSearchPage = 1;
  static const int onlineSearchPageSize = 30;
  OnlineSearchChannel onlineSearchChannel = OnlineSearchChannel.kuwo;
  List<Track> onlineResults = const [];
  List<PlatformPlaylist> platformPlaylists = const [];
  List<PlatformPlaylist> _platformPlaylistCatalog = const [];
  final Set<String> loadingPlatformPlaylistIds = {};
  final Map<String, List<Track>> _platformPlaylistTrackCache = {};
  List<Track> recentTracks = const [];
  List<Track> _recentTrackCatalog = const [];
  bool isLoadingPlatformPlaylists = false;
  String? platformPlaylistError;
  bool isLoadingRecentTracks = false;
  String? recentTracksError;
  bool isSearchingOnline = false;
  bool isImportingSources = false;
  String? onlineSearchError;
  OnlineSearchChannel rankingChannel = OnlineSearchChannel.kuwo;
  List<PlatformRanking> rankings = const [];
  PlatformRanking? selectedRanking;
  List<Track> rankingTracks = const [];
  bool isLoadingRankings = false;
  bool isLoadingRankingTracks = false;
  String? rankingsError;
  int _rankingListGeneration = 0;
  int _rankingTrackGeneration = 0;
  int _searchGeneration = 0;
  int _lyricGeneration = 0;
  int _playGeneration = 0;
  StreamSubscription<bool>? _playingSubscription;
  StreamSubscription<Duration>? _positionSubscription;
  StreamSubscription<Duration>? _durationSubscription;
  StreamSubscription<double>? _volumeSubscription;
  StreamSubscription<void>? _completedSubscription;
  StreamSubscription<String>? _errorSubscription;

  /// seek 尚未生效时记录目标位置，丢弃期间上报的旧进度
  Duration? _pendingSeek;

  /// 播放进度。只给进度条 / 歌词这类高频组件使用。
  final ValueNotifier<Duration> positionListenable = ValueNotifier(
    Duration.zero,
  );
  final ValueNotifier<Duration> durationListenable = ValueNotifier(
    Duration.zero,
  );
  final ValueNotifier<double> volumeListenable = ValueNotifier(1);

  /// 当前歌词行与行内进度（0-1），供歌词视图做无重建高亮。
  final ValueNotifier<int> lyricIndexListenable = ValueNotifier(-1);
  final ValueNotifier<double> lyricProgressListenable = ValueNotifier(0);

  bool isLoadingLyrics = false;
  bool _disposed = false;
  Timer? _updateCheckTimer;

  @override
  void notifyListeners() {
    // 异步刷新（歌单、新曲、歌词）可能在页面销毁后才返回。
    if (_disposed) return;
    super.notifyListeners();
  }

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
      folders = await musicRepository.loadFolders();
      sources = await sourceRepository.loadSources();
      selectedSourceId = await sourceRepository.loadSelectedSourceId();
      await _loadSpatialAudioSetting();
      unawaited(refreshPlatformPlaylists());
      unawaited(refreshRecentTracks());
      unawaited(_upgradeEmptyPlatformFavorites());
      unawaited(_loadLyricsStyle());
      // 启动后延迟静默检查更新，避免和首屏加载抢网络；用可取消的 Timer，
      // 页面销毁后不会留下挂起的计时器。
      _updateCheckTimer?.cancel();
      _updateCheckTimer = Timer(const Duration(seconds: 4), () {
        if (!_disposed) unawaited(checkForUpdate(silent: true));
      });
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

  Future<List<Track>?> loadPlatformPlaylistTracks(
    PlatformPlaylist playlist,
  ) async {
    final id = _platformPlaylistId(playlist);
    final cached = _platformPlaylistTrackCache[id];
    if (cached != null) return cached;
    if (loadingPlatformPlaylistIds.contains(id)) return null;
    loadingPlatformPlaylistIds.add(id);
    message = null;
    notifyListeners();
    try {
      final loaded = await onlineSearchService.discoverPlaylistTracks(playlist);
      if (loaded.isEmpty) throw StateError('歌单暂时没有可查看的歌曲');
      _platformPlaylistTrackCache[id] = loaded;
      return loaded;
    } on Object catch (error) {
      message = '载入歌单失败：$error';
      return null;
    } finally {
      loadingPlatformPlaylistIds.remove(id);
      notifyListeners();
    }
  }

  Future<void> togglePlatformPlaylistFavorite(PlatformPlaylist playlist) async {
    final id = _platformPlaylistId(playlist);
    if (loadingPlatformPlaylistIds.contains(id)) return;
    try {
      if (isPlatformPlaylistFavorite(playlist)) {
        await musicRepository.deletePlaylist(id);
        message = '已取消收藏 ${playlist.name}';
      } else {
        final tracks =
            await loadPlatformPlaylistTracks(playlist) ?? const <Track>[];
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

  Future<void> loadRankings(OnlineSearchChannel channel) async {
    final generation = ++_rankingListGeneration;
    _rankingTrackGeneration++;
    rankingChannel = channel;
    rankings = const [];
    selectedRanking = null;
    rankingTracks = const [];
    rankingsError = null;
    isLoadingRankings = true;
    notifyListeners();
    try {
      final loaded = await onlineSearchService.loadRankings(channel);
      if (generation != _rankingListGeneration) return;
      rankings = loaded;
      if (loaded.isNotEmpty) await selectRanking(loaded.first);
    } on Object catch (error) {
      if (generation != _rankingListGeneration) return;
      rankingsError = '榜单加载失败：$error';
    } finally {
      if (generation == _rankingListGeneration) {
        isLoadingRankings = false;
        notifyListeners();
      }
    }
  }

  Future<void> selectRanking(PlatformRanking ranking) async {
    final generation = ++_rankingTrackGeneration;
    selectedRanking = ranking;
    rankingTracks = const [];
    rankingsError = null;
    isLoadingRankingTracks = true;
    notifyListeners();
    try {
      final loaded = await onlineSearchService.loadRankingTracks(ranking);
      if (generation != _rankingTrackGeneration) return;
      rankingTracks = loaded;
      if (loaded.isEmpty) rankingsError = '该榜单暂时没有歌曲';
    } on Object catch (error) {
      if (generation != _rankingTrackGeneration) return;
      rankingsError = '榜单歌曲加载失败：$error';
    } finally {
      if (generation == _rankingTrackGeneration) {
        isLoadingRankingTracks = false;
        notifyListeners();
      }
    }
  }

  List<Track> get visibleTracks {
    final base = selectedFolder == null ? tracks : folderTracks;
    final needle = query.trim().toLowerCase();
    if (needle.isEmpty) return base;
    return base
        .where(
          (track) => '${track.title} ${track.artist} ${track.album}'
              .toLowerCase()
              .contains(needle),
        )
        .toList();
  }

  MusicFolder? get selectedFolder {
    for (final folder in folders) {
      if (folder.id == selectedFolderId) return folder;
    }
    return null;
  }

  List<Track> get folderTracks {
    final folder = selectedFolder;
    if (folder == null) return const [];
    final byId = {for (final track in tracks) track.id: track};
    return [for (final id in folder.trackIds) ?byId[id]];
  }

  Set<String> folderIdsOf(String trackId) => {
    for (final folder in folders)
      if (folder.trackIds.contains(trackId)) folder.id,
  };

  void selectFolder(String? id) {
    selectedFolderId = id;
    notifyListeners();
  }

  Future<MusicFolder?> createFolder(String name) async {
    try {
      final folder = await musicRepository.createFolder(name);
      folders = await musicRepository.loadFolders();
      message = '已创建文件夹 ${folder.name}';
      notifyListeners();
      return folder;
    } on Object catch (error) {
      message = '创建文件夹失败：$error';
      notifyListeners();
      return null;
    }
  }

  Future<void> deleteFolder(String id) async {
    try {
      await musicRepository.deleteFolder(id);
      folders = await musicRepository.loadFolders();
      if (selectedFolderId == id) selectedFolderId = null;
      message = '已删除文件夹';
    } on Object catch (error) {
      message = '删除文件夹失败：$error';
    }
    notifyListeners();
  }

  Future<void> addTrackToFolder(String folderId, String trackId) async {
    try {
      await musicRepository.addTrackToFolder(folderId, trackId);
      folders = await musicRepository.loadFolders();
      final name = selectedFolderName(folderId);
      message = '已收藏到 ${name ?? '文件夹'}';
    } on Object catch (error) {
      message = '收藏失败：$error';
    }
    notifyListeners();
  }

  Future<void> favoriteTrack(String folderId, Track track) async {
    try {
      if (!tracks.any((item) => item.id == track.id)) {
        tracks = [...tracks, track];
        await musicRepository.saveTracks(tracks);
      }
      await musicRepository.addTrackToFolder(folderId, track.id);
      folders = await musicRepository.loadFolders();
      final name = selectedFolderName(folderId);
      message = '已收藏到 ${name ?? '文件夹'}';
    } on Object catch (error) {
      message = '收藏失败：$error';
    }
    notifyListeners();
  }

  Future<void> removeTrackFromFolder(String folderId, String trackId) async {
    try {
      await musicRepository.removeTrackFromFolder(folderId, trackId);
      folders = await musicRepository.loadFolders();
      message = '已移出文件夹';
    } on Object catch (error) {
      message = '移出失败：$error';
    }
    notifyListeners();
  }

  String? selectedFolderName(String id) {
    for (final folder in folders) {
      if (folder.id == id) return folder.name;
    }
    return null;
  }

  Future<void> playTrack(Track track) async {
    final generation = ++_playGeneration;
    current = track;
    position = Duration.zero;
    duration = track.duration;
    positionListenable.value = Duration.zero;
    durationListenable.value = track.duration;
    lyricIndexListenable.value = -1;
    lyricProgressListenable.value = 0;
    message = null;
    notifyListeners();
    unawaited(playerService.stop());
    // 歌词与整曲解析并行：先显示歌词，解析慢也不会让聚焦页空着。
    unawaited(_loadLyrics(track));

    var playbackTrack = track;
    if (track.source != TrackSource.local && sources.isNotEmpty) {
      try {
        final selected = selectedSource;
        final sourceKeys = _activeSourceKeys;
        if (!sourceKeys.contains(playbackTrack.source.name)) {
          if (selected != null) {
            throw StateError(
              '所选音源 ${selected.name} 不支持 ${playbackTrack.source.name} 平台',
            );
          }
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
            background: true,
          );
          if (generation != _playGeneration) return;
          playbackTrack = track.copyWith(
            uri: url,
            source: playbackTrack.source,
            sourceId: playbackTrack.sourceId,
            quality: '洛雪音源 · ${playbackQuality.label}',
          );
          current = playbackTrack;
          message = null;
        }
      } on Object catch (error) {
        if (generation != _playGeneration) return;
        message = error.toString().contains('不支持')
            ? '所选音源不支持 ${playbackTrack.source.name} 平台，请更换音源'
            : (track.uri.isEmpty ? '整曲解析失败，该歌曲暂无播放地址' : '整曲解析失败，已回退平台试听');
        playbackTrack = track;
        current = track;
      }
    }

    if (playbackTrack.uri.isEmpty) {
      message ??= playbackTrack.source == TrackSource.local
          ? '本地歌曲文件不可用'
          : '该歌曲暂无可用的播放地址';
    } else {
      var finalTrack = playbackTrack;
      if (_isNetworkUri(finalTrack.uri)) {
        message = '正在下载整曲，请稍候...';
        notifyListeners();
        try {
          final localPath = await downloadService.downloadToCache(
            finalTrack,
            fallbackExtension: playbackQuality.sourceValue == 'flac'
                ? '.flac'
                : '.mp3',
            onProgress: (_) {},
            shouldCancel: () => generation != _playGeneration,
          );
          if (generation != _playGeneration) return;
          finalTrack = finalTrack.copyWith(uri: localPath);
        } on Object catch (error) {
          if (generation != _playGeneration) return;
          message = '下载播放失败，尝试直接播放：$error';
          notifyListeners();
        }
      }
      try {
        if (generation != _playGeneration) return;
        await playerService.play(finalTrack);
        if (generation != _playGeneration) return;
        isPlaying = true;
        current = finalTrack;
        message = null;
      } on Object catch (error) {
        if (generation != _playGeneration) return;
        message = '播放失败：$error';
        isPlaying = false;
      }
    }
    notifyListeners();
  }

  bool _isNetworkUri(String uri) {
    final scheme = Uri.tryParse(uri)?.scheme.toLowerCase();
    return scheme == 'http' || scheme == 'https';
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

  Future<void> downloadCurrentTrack([PlaybackQuality? quality]) async {
    final track = current;
    if (track == null) {
      message = '当前没有可下载的音频地址';
      notifyListeners();
      return;
    }
    await downloadTrack(track, quality);
  }

  Future<void> downloadTrack(Track track, [PlaybackQuality? quality]) async {
    final canResolveQuality =
        track.source != TrackSource.local &&
        (track.sourceId?.isNotEmpty ?? false);
    if (track.uri.isEmpty && !canResolveQuality) {
      message = '该歌曲没有可下载的音频地址';
      notifyListeners();
      return;
    }
    if (isDownloading) return;
    isDownloading = true;
    downloadProgress = null;
    notifyListeners();
    try {
      final downloadQuality = quality ?? playbackQuality;
      var downloadTrack = track;
      if (canResolveQuality) {
        message = '正在解析 ${downloadQuality.label} 下载地址...';
        notifyListeners();
        final url = await sourceRepository.resolveUrl(
          track,
          quality: downloadQuality.sourceValue,
          background: true,
        );
        downloadTrack = track.copyWith(
          uri: url,
          quality: '洛雪音源 · ${downloadQuality.label}',
        );
      }
      final path = await downloadService.download(
        downloadTrack,
        fallbackExtension: downloadQuality.sourceValue == 'flac'
            ? '.flac'
            : '.mp3',
        onProgress: (progress) {
          downloadProgress = progress;
          notifyListeners();
        },
      );
      message = !canResolveQuality
          ? '已下载到 $path'
          : '${downloadQuality.label} 已下载到 $path';
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
      positionListenable.value = Duration.zero;
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
    _pendingSeek = target;
    // 高频进度只推给专门的可监听对象，页面骨架不跟着每 100ms 重建。
    positionListenable.value = target;
    notifyListeners();
    try {
      await playerService.seek(target);
    } finally {
      // 给 mpv 一点时间把新位置推回来；期间丢弃旧位置的迟到上报
      Timer(const Duration(milliseconds: 600), () {
        _pendingSeek = null;
      });
    }
  }

  Future<void> setVolume(double value) async {
    final next = value.clamp(0.0, 1.0);
    if (next > .001) _volumeBeforeMute = next;
    volume = next;
    volumeListenable.value = next;
    notifyListeners();
    await playerService.setVolume(next);
  }

  Future<void> setSpatialAudioEnabled(bool enabled) async {
    try {
      await playerService.setSpatialAudioEnabled(enabled);
      spatialAudioEnabled = enabled;
      await audioEffectSettingsStore.saveSpatialAudio(enabled);
      message = enabled ? '空间音效已开启' : '空间音效已关闭';
    } on Object catch (error) {
      message = '更新空间音效失败：$error';
    }
    notifyListeners();
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

  Future<void> _loadSpatialAudioSetting() async {
    try {
      final enabled = await audioEffectSettingsStore.loadSpatialAudio();
      if (enabled) await playerService.setSpatialAudioEnabled(true);
      spatialAudioEnabled = enabled;
      notifyListeners();
    } on Object catch (error) {
      message = '恢复空间音效设置失败：$error';
      notifyListeners();
    }
  }

  /// 检查 GitHub 上的最新版本；[silent] 为真时用于启动时的静默检查，失败不弹提示
  Future<void> checkForUpdate({bool silent = false}) async {
    if (isCheckingUpdate || isInstallingUpdate) return;
    isCheckingUpdate = true;
    updateError = null;
    notifyListeners();
    try {
      final release = await releaseService.fetchLatestRelease();
      latestRelease = release;
      final current = await _resolveAppVersion();
      availableUpdate =
          current == null || ReleaseInfo.isNewer(release.version, current)
          ? release
          : null;
      lastUpdateCheck = DateTime.now();
    } on Object catch (error) {
      updateError = silent ? null : '检查更新失败：$error';
    } finally {
      isCheckingUpdate = false;
      notifyListeners();
    }
  }

  /// 一键更新：下载当前平台安装包并安装。
  /// Windows 会启动后台脚本替换文件并重启；Android 唤起系统安装器。
  Future<void> installUpdate() async {
    final release = availableUpdate;
    if (release == null || isInstallingUpdate) return;
    isInstallingUpdate = true;
    updateDownloadProgress = 0;
    updateError = null;
    message = null;
    notifyListeners();
    try {
      final result = await releaseService.installUpdate(
        release,
        onProgress: (progress) {
          updateDownloadProgress = progress;
          notifyListeners();
        },
      );
      if (result != null) {
        // Android：把 APK 交给系统安装器
        const channel = MethodChannel('wcmusic/lyrics_overlay');
        final launched = await channel.invokeMethod<bool>('installApk', {
          'path': result.apkPath,
        });
        message = (launched ?? false)
            ? '已启动安装程序，按提示完成更新'
            : '请允许 WCMusic 安装未知应用后，再点击「一键更新」';
        availableUpdate = null;
      } else {
        // Windows：更新脚本已启动，退出后自动解压替换并重启
        message = '正在更新到 ${release.version}，程序即将重启…';
        availableUpdate = null;
        updateDownloadProgress = 1;
        notifyListeners();
        await windowLifecycleService?.exitForUpdate();
      }
    } on Object catch (error) {
      updateError = '更新失败：$error';
    } finally {
      isInstallingUpdate = false;
      notifyListeners();
    }
  }

  Future<String?> _resolveAppVersion() async {
    if (appVersion != null) return appVersion;
    // PackageInfo（--app-version）只是第一来源：GitHubReleaseService 还会参考
    // pubspec 以及上次更新写入的版本记录，避免装上了新版本却一直提示更新。
    try {
      final info = await PackageInfo.fromPlatform();
      if (info.version.isNotEmpty) appVersion = info.version;
    } on Object {
      // 退回到 ReleaseService 提供的来源
    }
    try {
      final resolved = await releaseService.currentVersionName();
      if (resolved != null) appVersion = resolved;
    } on Object {
      // 保持 PackageInfo 的结果
    }
    return appVersion;
  }

  Future<void> _loadLyricsStyle() async {
    try {
      lyricsStyle = await floatingLyricsService.loadStyle();
      notifyListeners();
    } on Object {
      // 使用默认样式
    }
    // 读取系统字体较慢，放到样式就绪之后再异步加载
    unawaited(_loadSystemFonts());
  }

  Future<void> _loadSystemFonts() async {
    try {
      final fonts = await floatingLyricsService.listSystemFonts();
      final defaultFont = await floatingLyricsService.defaultSystemFont();
      if (fonts.isEmpty) return;
      systemFonts = fonts;
      defaultSystemFont = defaultFont;
      notifyListeners();
    } on Object {
      // 保留内置字体列表
    }
  }

  Future<void> updateLyricsStyle(LyricsOverlayStyle style) async {
    lyricsStyle = style;
    notifyListeners();
    try {
      await floatingLyricsService.setStyle(style);
      await floatingLyricsService.saveStyle(style);
    } on Object catch (error) {
      message = '更新歌词样式失败：$error';
      notifyListeners();
    }
  }

  Future<void> _loadLyrics(Track track) async {
    final generation = ++_lyricGeneration;
    lyrics = const [];
    currentLyricIndex = -1;
    lyricIndexListenable.value = -1;
    lyricProgressListenable.value = 0;
    isLoadingLyrics = true;
    notifyListeners();
    if (floatingLyricsEnabled) {
      await floatingLyricsService.update(
        title: '${track.title} · ${track.artist}',
        currentLine: '正在加载歌词...',
        nextLine: '',
      );
    }
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
      if (floatingLyricsEnabled) {
        await floatingLyricsService.update(
          title: '${track.title} · ${track.artist}',
          currentLine: loaded.isEmpty ? '暂无歌词' : '准备播放',
          nextLine: '',
        );
      }
      if (loaded.isNotEmpty) _syncLyrics(position);
    } on Object {
      if (generation != _lyricGeneration || current?.id != track.id) return;
      if (floatingLyricsEnabled) {
        await floatingLyricsService.update(
          title: '${track.title} · ${track.artist}',
          currentLine: '歌词暂时不可用',
          nextLine: '',
        );
      }
    } finally {
      if (generation == _lyricGeneration && current?.id == track.id) {
        isLoadingLyrics = false;
        notifyListeners();
      }
    }
  }

  void _syncLyrics(Duration value) {
    if (lyrics.isEmpty) {
      if (currentLyricIndex != -1) {
        currentLyricIndex = -1;
        lyricIndexListenable.value = -1;
      }
      return;
    }
    var nextIndex = -1;
    for (var index = 0; index < lyrics.length; index++) {
      if (lyrics[index].time > value) break;
      nextIndex = index;
    }
    currentLyricIndex = nextIndex;
    lyricIndexListenable.value = nextIndex;
    final followingIndex = nextIndex + 1;
    final lineStart = nextIndex >= 0 ? lyrics[nextIndex].time : Duration.zero;
    final lineEnd = followingIndex < lyrics.length
        ? lyrics[followingIndex].time
        : playbackDuration;
    final lineSpan = lineEnd - lineStart;
    final lineProgress = lineSpan.inMilliseconds <= 0
        ? 0.0
        : ((value - lineStart).inMilliseconds / lineSpan.inMilliseconds).clamp(
            0.0,
            1.0,
          );
    lyricProgressListenable.value = lineProgress;
    if (!floatingLyricsEnabled) return;
    final track = current;
    if (track == null) return;
    final currentLine = nextIndex < 0 ? track.title : lyrics[nextIndex].text;
    final nextLine = followingIndex >= 0 && followingIndex < lyrics.length
        ? lyrics[followingIndex].text
        : '';
    const radius = 5;
    final windowStart = (nextIndex - radius).clamp(0, lyrics.length - 1);
    final windowEnd = (nextIndex + radius + 1).clamp(0, lyrics.length);
    final windowLines = [
      for (var index = windowStart; index < windowEnd; index++)
        lyrics[index].text,
    ];
    final translation = nextIndex < 0 ? null : lyrics[nextIndex].translation;
    final nextTranslation =
        followingIndex >= 0 && followingIndex < lyrics.length
        ? lyrics[followingIndex].translation
        : null;
    unawaited(
      floatingLyricsService.update(
        title: '${track.title} · ${track.artist}',
        currentLine: currentLine,
        nextLine: nextLine,
        currentTranslation: translation,
        nextTranslation: nextTranslation,
        lines: windowLines,
        currentIndex: nextIndex - windowStart,
        lineProgress: lineProgress,
      ),
    );
  }

  bool get canLoadMoreOnlineResults =>
      onlineResults.length >= onlineSearchPageSize;

  void setQuery(String value) {
    query = value;
    notifyListeners();
  }

  Future<void> searchOnline(String value, {int page = 1}) async {
    final keyword = value.trim();
    onlineQuery = keyword;
    onlineSearchPage = page < 1 ? 1 : page;
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
        limit: onlineSearchPageSize,
        page: onlineSearchPage,
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

  Future<SourceImportResult> importSources(
    List<({String name, String script})> entries,
  ) async {
    if (isImportingSources) {
      return const SourceImportResult(imported: 0, failures: []);
    }
    isImportingSources = true;
    var imported = 0;
    final failures = <String>[];
    notifyListeners();
    try {
      for (final entry in entries) {
        try {
          await sourceRepository.importScript(entry.script);
          imported++;
        } on FormatException catch (error) {
          failures.add('${entry.name}：${error.message}');
        } on Object catch (error) {
          failures.add('${entry.name}：$error');
        }
      }
      sources = await sourceRepository.loadSources();
      message = imported > 0 ? '已导入 $imported 个音源' : '没有可导入的音源';
    } finally {
      isImportingSources = false;
      notifyListeners();
    }
    return SourceImportResult(imported: imported, failures: failures);
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
    _disposed = true;
    _updateCheckTimer?.cancel();
    _lyricGeneration++;
    unawaited(_playingSubscription?.cancel());
    unawaited(_positionSubscription?.cancel());
    unawaited(_durationSubscription?.cancel());
    unawaited(_volumeSubscription?.cancel());
    unawaited(_completedSubscription?.cancel());
    unawaited(_errorSubscription?.cancel());
    positionListenable.dispose();
    durationListenable.dispose();
    volumeListenable.dispose();
    lyricIndexListenable.dispose();
    lyricProgressListenable.dispose();
    unawaited(playerService.dispose());
    unawaited(floatingLyricsService.dispose());
    super.dispose();
  }
}
