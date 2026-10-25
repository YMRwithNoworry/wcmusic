/// GitHub 上的一条发布记录（已挑出当前平台的安装包）。
class ReleaseInfo {
  const ReleaseInfo({
    required this.tag,
    required this.version,
    required this.name,
    required this.notes,
    required this.assetName,
    required this.assetUrl,
    required this.assetSize,
    required this.pageUrl,
    this.publishedAt,
  });

  /// 例如 v1.2.82 或 android-v1.2.81
  final String tag;

  /// 去掉平台前缀后的版本号，例如 1.2.82
  final String version;
  final String name;
  final String notes;
  final String assetName;
  final String assetUrl;
  final int assetSize;
  final String pageUrl;
  final DateTime? publishedAt;

  /// 比较两个版本号（按数字逐段比较），[a] 大于 [b] 时返回正数。
  static int compareVersions(String a, String b) {
    final left = _segments(a);
    final right = _segments(b);
    final length = left.length > right.length ? left.length : right.length;
    for (var index = 0; index < length; index++) {
      final l = index < left.length ? left[index] : 0;
      final r = index < right.length ? right[index] : 0;
      if (l != r) return l.compareTo(r);
    }
    return 0;
  }

  static bool isNewer(String candidate, String current) =>
      compareVersions(candidate, current) > 0;

  static List<int> _segments(String version) => version
      .split('+')
      .first
      .split('.')
      .map((part) => int.tryParse(part.replaceAll(RegExp(r'[^0-9]'), '')) ?? 0)
      .toList();
}

/// 一键更新的安装动作：下载结束后交给原生界面完成安装。
class UpdateInstallResult {
  const UpdateInstallResult.apk(this.apkPath);

  /// Android 安装包路径，界面据此唤起系统安装器
  final String apkPath;
}
