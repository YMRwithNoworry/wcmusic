import '../models/track.dart';

abstract interface class SourceRepository {
  Future<List<SourceScript>> loadSources();
  Future<String?> loadSelectedSourceId();
  Future<void> selectSource(String? id);
  Future<SourceScript> importScript(String rawScript);
  Future<void> deleteSource(String id);
  Future<String> resolveUrl(
    Track track, {
    String quality = '320k',
    bool background = false,
  });
}
