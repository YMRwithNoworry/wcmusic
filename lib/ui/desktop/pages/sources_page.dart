import 'dart:convert';
import 'dart:io';

import 'package:file_selector/file_selector.dart';
import 'package:flutter/widgets.dart';
import 'package:forui/forui.dart';
import 'package:provider/provider.dart';

import '../../../domain/models/source_import_result.dart';
import '../../../domain/models/track.dart';
import '../../state/player_view_model.dart';
import '../widgets/common.dart';
import '../widgets/dialogs.dart';
import '../widgets/hover.dart';

class SourcesPage extends StatelessWidget {
  const SourcesPage({super.key});

  @override
  Widget build(BuildContext context) {
    final model = context.watch<PlayerViewModel>();
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        PageHeader(
          eyebrow: 'WCMusic / 音源',
          title: '音源管理',
          subtitle: '兼容洛雪自定义源协议；脚本默认视为不可信代码',
          actions: [
            FButton(
              variant: FButtonVariant.outline,
              size: FButtonSizeVariant.sm,
              mainAxisSize: MainAxisSize.min,
              prefix: const Icon(FLucideIcons.folderOpen, size: 15),
              onPress: model.isImportingSources
                  ? null
                  : () => _pickDirectory(context),
              child: const Text('导入文件夹'),
            ),
            FButton(
              size: FButtonSizeVariant.sm,
              mainAxisSize: MainAxisSize.min,
              prefix: const Icon(FLucideIcons.link, size: 15),
              onPress: model.isImportingSources
                  ? null
                  : () => _pickScript(context),
              child: const Text('导入脚本'),
            ),
          ],
        ),
        Expanded(
          child: PageBody(
            child: model.sources.isEmpty
                ? const EmptyState(
                    icon: FLucideIcons.plug,
                    title: '还没有音源',
                    description: '导入 .js 文件后会先检查脚本信息与初始化协议，再启用整曲解析。',
                  )
                : ListView(
                    padding: const EdgeInsets.fromLTRB(32, 4, 32, 28),
                    children: [
                      Padding(
                        padding: const EdgeInsets.only(bottom: 12),
                        child: Text(
                          model.selectedSource == null
                              ? '当前自动使用全部音源；点选音源可指定整曲解析来源'
                              : '整曲解析优先使用：${model.selectedSource!.name}',
                          style: context.theme.typography.body.xs.copyWith(
                            color: context.theme.colors.mutedForeground,
                          ),
                        ),
                      ),
                      for (final source in model.sources)
                        Padding(
                          padding: const EdgeInsets.only(bottom: 10),
                          child: _SourceCard(
                            source: source,
                            selected: model.selectedSourceId == source.id,
                            onSelect: () => model.selectSource(source.id),
                            onDelete: source.isBuiltIn
                                ? null
                                : () => _deleteSource(context, source),
                          ),
                        ),
                    ],
                  ),
          ),
        ),
      ],
    );
  }

  Future<void> _pickScript(BuildContext context) async {
    final model = context.read<PlayerViewModel>();
    try {
      const scriptTypes = XTypeGroup(
        label: '洛雪音源脚本',
        extensions: ['js'],
        mimeTypes: ['application/javascript', 'text/javascript'],
      );
      final selected = await openFile(acceptedTypeGroups: const [scriptTypes]);
      if (selected == null || !context.mounted) return;
      final raw = utf8.decode(await selected.readAsBytes());
      if (!context.mounted) return;
      final accepted = await confirmAction(
        context,
        title: '导入第三方音源？',
        description: '音源脚本属于第三方代码。仅导入你信任且来源明确的脚本。',
        confirmLabel: '检查并导入',
      );
      if (!accepted || !context.mounted) return;
      await model.importSource(raw);
      if (context.mounted) {
        showAppToast(context, model.message ?? '导入完成');
      }
    } on FormatException {
      if (context.mounted) {
        showAppToast(context, '脚本必须使用 UTF-8 编码', error: true);
      }
    } on Object catch (error) {
      if (context.mounted) showAppToast(context, '导入失败：$error', error: true);
    }
  }

  Future<void> _pickDirectory(BuildContext context) async {
    final model = context.read<PlayerViewModel>();
    try {
      final path = await getDirectoryPath();
      if (path == null || !context.mounted) return;
      final directory = Directory(path);
      if (!await directory.exists()) {
        if (context.mounted) showAppToast(context, '文件夹不存在', error: true);
        return;
      }
      final files = <File>[];
      await for (final entity in directory.list(
        recursive: true,
        followLinks: false,
      )) {
        if (entity is File && entity.path.toLowerCase().endsWith('.js')) {
          files.add(entity);
        }
      }
      files.sort((left, right) => left.path.compareTo(right.path));
      if (files.isEmpty) {
        if (context.mounted) {
          showAppToast(context, '没有找到 .js 音源脚本', error: true);
        }
        return;
      }
      if (!context.mounted) return;
      final accepted = await confirmAction(
        context,
        title: '批量导入音源？',
        description:
            '找到 ${files.length} 个音源脚本，将逐个检查并导入。同名但不同版本的音源会作为独立条目保留。',
        confirmLabel: '检查并导入',
      );
      if (!accepted || !context.mounted) return;
      final entries = <({String name, String script})>[];
      final readFailures = <String>[];
      for (final file in files) {
        final name = file.uri.pathSegments.last;
        try {
          final raw = utf8.decode(await file.readAsBytes());
          entries.add((name: name, script: raw));
        } on FormatException {
          readFailures.add('$name：不是有效的 UTF-8 编码');
        } on Object catch (error) {
          readFailures.add('$name：$error');
        }
      }
      if (entries.isEmpty) {
        if (context.mounted) {
          showAppToast(context, '没有可读取的音源脚本', error: true);
        }
        return;
      }
      if (!context.mounted) return;
      final result = await model.importSources(entries);
      if (context.mounted) {
        await _showImportResult(context, result, readFailures);
      }
    } on Object catch (error) {
      if (context.mounted) {
        showAppToast(context, '批量导入失败：$error', error: true);
      }
    }
  }

  Future<void> _showImportResult(
    BuildContext context,
    SourceImportResult result,
    List<String> readFailures,
  ) async {
    final failures = [...readFailures, ...result.failures];
    await showAppDialog<void>(
      context: context,
      title: '导入完成',
      description:
          '成功导入 ${result.imported} 个音源${failures.isEmpty ? '' : '，失败 ${failures.length} 个'}',
      content: failures.isEmpty
          ? null
          : SizedBox(
              width: 520,
              child: ConstrainedBox(
                constraints: const BoxConstraints(maxHeight: 260),
                child: ListView.builder(
                  shrinkWrap: true,
                  itemCount: failures.length,
                  itemBuilder: (context, index) => Padding(
                    padding: const EdgeInsets.only(bottom: 6),
                    child: Text(
                      failures[index],
                      style: context.theme.typography.body.xs.copyWith(
                        color: context.theme.colors.mutedForeground,
                      ),
                    ),
                  ),
                ),
              ),
            ),
      actions: [
        FButton(
          size: FButtonSizeVariant.sm,
          mainAxisSize: MainAxisSize.min,
          onPress: () => Navigator.of(context).pop(),
          child: const Text('知道了'),
        ),
      ],
    );
  }

  Future<void> _deleteSource(BuildContext context, SourceScript source) async {
    final model = context.read<PlayerViewModel>();
    final confirmed = await confirmAction(
      context,
      title: '删除音源？',
      description: '将从本机移除「${source.name}」及其脚本。',
      confirmLabel: '删除',
      destructive: true,
    );
    if (!confirmed || !context.mounted) return;
    await model.deleteSource(source.id);
    if (context.mounted) {
      showAppToast(context, model.message ?? '已删除音源');
    }
  }
}

class _SourceCard extends StatelessWidget {
  const _SourceCard({
    required this.source,
    required this.selected,
    required this.onSelect,
    this.onDelete,
  });

  final SourceScript source;
  final bool selected;
  final VoidCallback onSelect;
  final VoidCallback? onDelete;

  @override
  Widget build(BuildContext context) {
    final colors = context.theme.colors;
    final type = context.theme.typography;
    return HoverLift(
      scale: 1.005,
      onTap: onSelect,
      borderRadius: const BorderRadius.all(Radius.circular(16)),
      child: AnimatedContainer(
        duration: const Duration(milliseconds: 170),
        padding: const EdgeInsets.all(16),
        decoration: BoxDecoration(
          color: colors.secondary.withValues(alpha: selected ? .72 : .42),
          borderRadius: const BorderRadius.all(Radius.circular(16)),
          border: Border.all(
            color: selected
                ? colors.primary.withValues(alpha: .45)
                : colors.border,
          ),
        ),
        child: Row(
          children: [
            Container(
              width: 42,
              height: 42,
              decoration: BoxDecoration(
                color: selected
                    ? colors.primary.withValues(alpha: .18)
                    : colors.muted,
                borderRadius: const BorderRadius.all(Radius.circular(11)),
              ),
              child: Icon(
                selected ? FLucideIcons.circleCheck : FLucideIcons.plug,
                size: 19,
                color: selected ? colors.primary : colors.mutedForeground,
              ),
            ),
            const SizedBox(width: 14),
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Row(
                    children: [
                      Flexible(
                        child: Text(
                          source.name,
                          maxLines: 1,
                          overflow: TextOverflow.ellipsis,
                          style: type.body.sm.copyWith(
                            color: colors.foreground,
                            fontWeight: FontWeight.w600,
                          ),
                        ),
                      ),
                      if (source.isBuiltIn) ...[
                        const SizedBox(width: 8),
                        FBadge(
                          variant: FBadgeVariant.secondary,
                          child: const Text('内置'),
                        ),
                      ],
                      if (selected) ...[
                        const SizedBox(width: 8),
                        FBadge(child: const Text('整曲解析')),
                      ],
                    ],
                  ),
                  const SizedBox(height: 3),
                  Text(
                    '${source.author} · ${source.version}',
                    style: type.body.xs.copyWith(
                      color: colors.mutedForeground,
                    ),
                  ),
                  const SizedBox(height: 5),
                  Text(
                    source.sourceKeys.isEmpty
                        ? '未声明服务'
                        : '服务：${source.sourceKeys.join(' / ')}',
                    maxLines: 2,
                    overflow: TextOverflow.ellipsis,
                    style: type.body.xs.copyWith(
                      color: colors.mutedForeground.withValues(alpha: .85),
                      fontSize: 11,
                    ),
                  ),
                ],
              ),
            ),
            const SizedBox(width: 10),
            FTooltip(
              tipBuilder: (context, _) => const Text('已通过 Rust 运行时校验'),
              child: Icon(
                FLucideIcons.badgeCheck,
                size: 18,
                color: colors.primary.withValues(alpha: .8),
              ),
            ),
            if (source.isBuiltIn)
              Padding(
                padding: const EdgeInsets.only(left: 10),
                child: Icon(
                  FLucideIcons.lock,
                  size: 16,
                  color: colors.mutedForeground,
                ),
              )
            else ...[
              const SizedBox(width: 6),
              FPopoverMenu(
                menu: [
                  FItemGroup(
                    children: [
                      if (!selected)
                        FItem(
                          title: const Text('设为整曲解析音源'),
                          prefix: const Icon(
                            FLucideIcons.circleCheck,
                            size: 16,
                          ),
                          onPress: onSelect,
                        ),
                      FItem(
                        title: const Text('删除音源'),
                        prefix: const Icon(FLucideIcons.trash2, size: 16),
                        onPress: onDelete,
                      ),
                    ],
                  ),
                ],
                builder: (context, controller, child) => FButton.icon(
                  variant: FButtonVariant.ghost,
                  size: FButtonSizeVariant.sm,
                  onPress: controller.toggle,
                  child: const Icon(FLucideIcons.moreHorizontal, size: 16),
                ),
              ),
            ],
          ],
        ),
      ),
    );
  }
}
