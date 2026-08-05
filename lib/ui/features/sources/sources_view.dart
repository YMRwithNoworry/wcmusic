import 'dart:convert';
import 'dart:io';

import 'package:file_selector/file_selector.dart';
import 'package:flutter/material.dart';
import 'package:provider/provider.dart';

import '../../../domain/models/source_import_result.dart';
import '../../../domain/models/track.dart';
import '../../core/page_scaffold.dart';
import '../player/player_view_model.dart';

class SourcesView extends StatelessWidget {
  const SourcesView({super.key});

  @override
  Widget build(BuildContext context) {
    final viewModel = context.watch<PlayerViewModel>();
    return PageScaffold(
      title: '音源',
      subtitle: '兼容洛雪自定义源协议；脚本默认视为不可信代码',
      actions: [
        TextButton.icon(
          onPressed: viewModel.isImportingSources
              ? null
              : () => _pickDirectory(context),
          icon: const Icon(Icons.folder_open),
          label: const Text('导入文件夹'),
        ),
        FilledButton.tonalIcon(
          onPressed: viewModel.isImportingSources
              ? null
              : () => _pickScript(context),
          icon: const Icon(Icons.add_link),
          label: const Text('导入脚本'),
        ),
      ],
      child: viewModel.sources.isEmpty
          ? const _SourceEmptyState()
          : Column(
              children: [
                Padding(
                  padding: const EdgeInsets.only(bottom: 12),
                  child: Align(
                    alignment: Alignment.centerLeft,
                    child: Text(
                      viewModel.selectedSource == null
                          ? '当前自动使用全部音源；点选音源可指定整曲解析来源'
                          : '整曲解析优先使用：${viewModel.selectedSource!.name}',
                      style: Theme.of(context).textTheme.bodySmall,
                    ),
                  ),
                ),
                for (final source in viewModel.sources)
                  _SourceRow(
                    source: source,
                    selected: viewModel.selectedSourceId == source.id,
                    onSelect: () => viewModel.selectSource(source.id),
                    onDelete: source.isBuiltIn
                        ? null
                        : () => _deleteSource(context, source),
                  ),
              ],
            ),
    );
  }

  Future<void> _pickScript(BuildContext context) async {
    try {
      const scriptTypes = XTypeGroup(
        label: '洛雪音源脚本',
        extensions: ['js'],
        mimeTypes: ['application/javascript', 'text/javascript'],
      );
      final selected = await openFile(acceptedTypeGroups: const [scriptTypes]);
      if (selected == null || !context.mounted) return;
      final bytes = await selected.readAsBytes();
      final raw = utf8.decode(bytes, allowMalformed: false);
      if (!context.mounted) return;
      final accepted = await showDialog<bool>(
        context: context,
        builder: (context) => AlertDialog(
          title: const Text('导入第三方音源？'),
          content: const Text('音源脚本属于第三方代码。仅导入你信任且来源明确的脚本。'),
          actions: [
            TextButton(
              onPressed: () => Navigator.pop(context, false),
              child: const Text('取消'),
            ),
            FilledButton(
              onPressed: () => Navigator.pop(context, true),
              child: const Text('检查并导入'),
            ),
          ],
        ),
      );
      if (accepted == true && context.mounted) {
        final viewModel = context.read<PlayerViewModel>();
        await viewModel.importSource(raw);
        if (context.mounted) _showMessage(context, viewModel.message ?? '导入完成');
      }
    } on FormatException {
      if (context.mounted) _showMessage(context, '脚本必须使用 UTF-8 编码');
    } on Object catch (error) {
      if (context.mounted) _showMessage(context, '导入失败：$error');
    }
  }

  Future<void> _pickDirectory(BuildContext context) async {
    try {
      final path = await getDirectoryPath();
      if (path == null || !context.mounted) return;
      final directory = Directory(path);
      if (!await directory.exists()) {
        if (context.mounted) _showMessage(context, '文件夹不存在');
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
        if (context.mounted) _showMessage(context, '没有找到 .js 音源脚本');
        return;
      }
      if (!context.mounted) return;
      final accepted = await showDialog<bool>(
        context: context,
        builder: (context) => AlertDialog(
          title: const Text('批量导入音源？'),
          content: Text(
            '找到 ${files.length} 个音源脚本，将逐个检查并导入。'
            '同名但不同版本的音源会作为独立条目保留。',
          ),
          actions: [
            TextButton(
              onPressed: () => Navigator.pop(context, false),
              child: const Text('取消'),
            ),
            FilledButton(
              onPressed: () => Navigator.pop(context, true),
              child: const Text('检查并导入'),
            ),
          ],
        ),
      );
      if (accepted != true || !context.mounted) return;
      final entries = <({String name, String script})>[];
      final readFailures = <String>[];
      for (final file in files) {
        final name = file.uri.pathSegments.last;
        try {
          final raw = utf8.decode(
            await file.readAsBytes(),
            allowMalformed: false,
          );
          entries.add((name: name, script: raw));
        } on FormatException {
          readFailures.add('$name：不是有效的 UTF-8 编码');
        } on Object catch (error) {
          readFailures.add('$name：$error');
        }
      }
      if (entries.isEmpty) {
        if (context.mounted) _showMessage(context, '没有可读取的音源脚本');
        return;
      }
      if (!context.mounted) return;
      final viewModel = context.read<PlayerViewModel>();
      final result = await viewModel.importSources(entries);
      if (context.mounted) {
        await _showImportResult(context, result, readFailures);
      }
    } on Object catch (error) {
      if (context.mounted) _showMessage(context, '批量导入失败：$error');
    }
  }

  Future<void> _showImportResult(
    BuildContext context,
    SourceImportResult result,
    List<String> readFailures,
  ) async {
    final failures = [...readFailures, ...result.failures];
    await showDialog<void>(
      context: context,
      builder: (context) => AlertDialog(
        title: const Text('导入完成'),
        content: SizedBox(
          width: 520,
          child: Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text(
                '成功导入 ${result.imported} 个音源${failures.isEmpty ? '' : '，失败 ${failures.length} 个'}',
              ),
              if (failures.isNotEmpty) ...[
                const SizedBox(height: 12),
                ConstrainedBox(
                  constraints: const BoxConstraints(maxHeight: 240),
                  child: ListView.builder(
                    shrinkWrap: true,
                    itemCount: failures.length,
                    itemBuilder: (context, index) => Padding(
                      padding: const EdgeInsets.only(bottom: 6),
                      child: Text(
                        failures[index],
                        style: Theme.of(context).textTheme.bodySmall,
                      ),
                    ),
                  ),
                ),
              ],
            ],
          ),
        ),
        actions: [
          FilledButton(
            onPressed: () => Navigator.pop(context),
            child: const Text('知道了'),
          ),
        ],
      ),
    );
  }

  Future<void> _deleteSource(BuildContext context, SourceScript source) async {
    final confirmed = await showDialog<bool>(
      context: context,
      builder: (context) => AlertDialog(
        title: const Text('删除音源？'),
        content: Text('将从本机移除“${source.name}”及其脚本。'),
        actions: [
          TextButton(
            onPressed: () => Navigator.pop(context, false),
            child: const Text('取消'),
          ),
          FilledButton(
            onPressed: () => Navigator.pop(context, true),
            child: const Text('删除'),
          ),
        ],
      ),
    );
    if (confirmed == true && context.mounted) {
      final viewModel = context.read<PlayerViewModel>();
      await viewModel.deleteSource(source.id);
      if (context.mounted) _showMessage(context, viewModel.message ?? '已删除音源');
    }
  }

  void _showMessage(BuildContext context, String message) {
    final messenger = ScaffoldMessenger.of(context);
    messenger
      ..hideCurrentSnackBar()
      ..showSnackBar(SnackBar(content: Text(message)));
  }
}

class _SourceRow extends StatelessWidget {
  const _SourceRow({
    required this.source,
    required this.selected,
    required this.onSelect,
    required this.onDelete,
  });

  final SourceScript source;
  final bool selected;
  final VoidCallback onSelect;
  final VoidCallback? onDelete;

  @override
  Widget build(BuildContext context) {
    return Container(
      margin: const EdgeInsets.only(bottom: 10),
      padding: const EdgeInsets.all(18),
      decoration: BoxDecoration(
        color: Theme.of(context).colorScheme.surface,
        borderRadius: BorderRadius.circular(8),
      ),
      child: Row(
        children: [
          InkWell(
            onTap: onSelect,
            borderRadius: BorderRadius.circular(22),
            child: Padding(
              padding: const EdgeInsets.all(6),
              child: Tooltip(
                message: selected ? '当前整曲解析音源' : '选择该音源用于整曲解析',
                child: Icon(
                  selected
                      ? Icons.radio_button_checked
                      : Icons.radio_button_unchecked,
                  color: selected
                      ? Theme.of(context).colorScheme.primary
                      : Theme.of(context).colorScheme.onSurfaceVariant,
                ),
              ),
            ),
          ),
          const SizedBox(width: 8),
          Container(
            width: 44,
            height: 44,
            decoration: BoxDecoration(
              color: Theme.of(context).colorScheme.primaryContainer,
              borderRadius: BorderRadius.circular(8),
            ),
            child: const Icon(Icons.extension),
          ),
          const SizedBox(width: 16),
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  source.name,
                  style: Theme.of(context).textTheme.titleLarge,
                ),
                const SizedBox(height: 3),
                Text('${source.author} · ${source.version}'),
                const SizedBox(height: 7),
                Text(
                  source.sourceKeys.isEmpty
                      ? '未声明服务'
                      : '服务：${source.sourceKeys.join(' / ')}',
                  maxLines: 2,
                  overflow: TextOverflow.ellipsis,
                  style: Theme.of(context).textTheme.bodySmall,
                ),
              ],
            ),
          ),
          Tooltip(
            message: '已通过 Rust 运行时校验',
            child: Icon(
              Icons.verified_outlined,
              color: Theme.of(context).colorScheme.primary,
            ),
          ),
          if (source.isBuiltIn)
            const Tooltip(message: '应用内置音源', child: Icon(Icons.lock_outline))
          else
            PopupMenuButton<_SourceAction>(
              icon: const Icon(Icons.more_horiz),
              tooltip: '音源菜单',
              onSelected: (action) {
                switch (action) {
                  case _SourceAction.select:
                    onSelect();
                    break;
                  case _SourceAction.delete:
                    onDelete?.call();
                    break;
                }
              },
              itemBuilder: (context) => [
                if (!selected)
                  const PopupMenuItem(
                    value: _SourceAction.select,
                    child: Row(
                      children: [
                        Icon(Icons.radio_button_checked),
                        SizedBox(width: 12),
                        Text('设为整曲解析音源'),
                      ],
                    ),
                  ),
                PopupMenuItem(
                  value: _SourceAction.delete,
                  child: const Row(
                    children: [
                      Icon(Icons.delete_outline),
                      SizedBox(width: 12),
                      Text('删除音源'),
                    ],
                  ),
                ),
              ],
            ),
        ],
      ),
    );
  }
}

enum _SourceAction { select, delete }

class _SourceEmptyState extends StatelessWidget {
  const _SourceEmptyState();

  @override
  Widget build(BuildContext context) {
    return Container(
      width: double.infinity,
      padding: const EdgeInsets.symmetric(horizontal: 22, vertical: 58),
      decoration: BoxDecoration(
        color: Theme.of(context).colorScheme.surface,
        borderRadius: BorderRadius.circular(8),
        border: Border.all(
          color: Theme.of(context).dividerColor.withValues(alpha: .16),
        ),
      ),
      child: Column(
        children: [
          Icon(
            Icons.hub_outlined,
            size: 44,
            color: Theme.of(context).colorScheme.primary,
          ),
          const SizedBox(height: 18),
          Text('还没有音源', style: Theme.of(context).textTheme.headlineMedium),
          const SizedBox(height: 8),
          const Text('导入 .js 文件后会先检查脚本信息与初始化协议'),
        ],
      ),
    );
  }
}
