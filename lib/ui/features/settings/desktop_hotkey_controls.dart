import 'dart:async';

import 'package:flutter/material.dart';
import 'package:provider/provider.dart';

import '../../../data/services/desktop_hotkey_service.dart';
import '../player/player_view_model.dart';

class DesktopHotkeyControls extends StatelessWidget {
  const DesktopHotkeyControls({super.key});

  @override
  Widget build(BuildContext context) {
    final viewModel = context.watch<PlayerViewModel>();
    final settings = viewModel.desktopHotkeySettings;
    return Column(
      children: [
        SwitchListTile(
          value: settings.enabled,
          onChanged: (enabled) => unawaited(
            viewModel.updateDesktopHotkeySettings(
              settings.copyWith(enabled: enabled),
            ),
          ),
          title: const Text('启用全局快捷键'),
        ),
        if (settings.enabled)
          for (final action in desktopHotkeyActions)
            _BindingTile(
              key: ValueKey(action.id),
              action: action,
              binding: settings.bindings[action.id] ?? '',
              onChanged: (binding) {
                final bindings = Map<String, String>.from(settings.bindings)
                  ..[action.id] = binding;
                unawaited(
                  viewModel.updateDesktopHotkeySettings(
                    settings.copyWith(bindings: bindings),
                  ),
                );
              },
            ),
        if (viewModel.desktopHotkeyError case final error?)
          ListTile(
            leading: const Icon(Icons.error_outline, color: Colors.red),
            title: Text(error),
          ),
        Align(
          alignment: Alignment.centerRight,
          child: IconButton(
            tooltip: '恢复默认快捷键',
            onPressed: () => unawaited(
              viewModel.updateDesktopHotkeySettings(
                DesktopHotkeySettings.defaults(),
              ),
            ),
            icon: const Icon(Icons.restart_alt),
          ),
        ),
      ],
    );
  }
}

class _BindingTile extends StatefulWidget {
  const _BindingTile({
    super.key,
    required this.action,
    required this.binding,
    required this.onChanged,
  });

  final DesktopHotkeyAction action;
  final String binding;
  final ValueChanged<String> onChanged;

  @override
  State<_BindingTile> createState() => _BindingTileState();
}

class _BindingTileState extends State<_BindingTile> {
  final FocusNode _focusNode = FocusNode();
  bool _listening = false;
  String? _error;

  @override
  void dispose() {
    _focusNode.dispose();
    super.dispose();
  }

  KeyEventResult _onKeyEvent(FocusNode node, KeyEvent event) {
    if (!_listening || event is! KeyDownEvent) return KeyEventResult.ignored;
    if (event.logicalKey == LogicalKeyboardKey.escape) {
      setState(() {
        _listening = false;
        _error = null;
      });
      return KeyEventResult.handled;
    }
    final binding = hotkeyBindingFromEvent(event);
    if (binding == null) {
      setState(() => _error = '组合键必须包含修饰键');
      return KeyEventResult.handled;
    }
    setState(() {
      _listening = false;
      _error = null;
    });
    widget.onChanged(binding);
    return KeyEventResult.handled;
  }

  @override
  Widget build(BuildContext context) {
    final label = _listening
        ? '等待按键'
        : hotkeyBindingLabel(widget.binding);
    return Focus(
      focusNode: _focusNode,
      onKeyEvent: _onKeyEvent,
      child: ListTile(
        dense: true,
        title: Text(widget.action.label),
        subtitle: Text(_error ?? label),
        trailing: IconButton(
          tooltip: _listening ? '取消' : '录入快捷键',
          onPressed: () {
            setState(() {
              _listening = !_listening;
              _error = null;
            });
            if (_listening) _focusNode.requestFocus();
          },
          icon: Icon(_listening ? Icons.close : Icons.keyboard),
        ),
      ),
    );
  }
}
