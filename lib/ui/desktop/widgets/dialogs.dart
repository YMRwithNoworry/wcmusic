import 'package:flutter/widgets.dart';
import 'package:forui/forui.dart';

/// 统一的对话框外壳。
///
/// 桌面端用 Forui 的 [FDialog] 呈现：圆角、边框、缩放淡入都来自主题，
/// 内部布局保持一致的 24px 内边距与右对齐操作区。
Future<T?> showAppDialog<T>({
  required BuildContext context,
  required String title,
  String? description,
  Widget? content,
  List<Widget> actions = const [],
  bool barrierDismissible = true,
  BoxConstraints constraints = const BoxConstraints(
    minWidth: 340,
    maxWidth: 560,
  ),
}) {
  return showFDialog<T>(
    context: context,
    barrierDismissible: barrierDismissible,
    builder: (context, style, animation) => FDialog(
      animation: animation,
      constraints: constraints,
      builder: (context, style) => Padding(
        padding: const EdgeInsets.fromLTRB(24, 22, 24, 18),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(title, style: style.titleTextStyle),
            if (description != null) ...[
              const SizedBox(height: 8),
              Text(description, style: style.bodyTextStyle),
            ],
            if (content != null) ...[const SizedBox(height: 18), content],
            if (actions.isNotEmpty) ...[
              const SizedBox(height: 22),
              Row(
                mainAxisAlignment: MainAxisAlignment.end,
                spacing: 10,
                children: actions,
              ),
            ],
          ],
        ),
      ),
    ),
  );
}

/// 危险操作确认。
Future<bool> confirmAction(
  BuildContext context, {
  required String title,
  String? description,
  String confirmLabel = '确认',
  String cancelLabel = '取消',
  bool destructive = false,
}) async {
  final result = await showAppDialog<bool>(
    context: context,
    title: title,
    description: description,
    actions: [
      FButton(
        variant: FButtonVariant.outline,
        size: FButtonSizeVariant.sm,
        mainAxisSize: MainAxisSize.min,
        onPress: () => Navigator.of(context).pop(false),
        child: Text(cancelLabel),
      ),
      FButton(
        variant: destructive
            ? FButtonVariant.destructive
            : FButtonVariant.primary,
        size: FButtonSizeVariant.sm,
        mainAxisSize: MainAxisSize.min,
        onPress: () => Navigator.of(context).pop(true),
        child: Text(confirmLabel),
      ),
    ],
  );
  return result ?? false;
}

/// 单行文本输入。
Future<String?> promptForText(
  BuildContext context, {
  required String title,
  String? description,
  String hint = '',
  String initial = '',
  String confirmLabel = '确定',
  String cancelLabel = '取消',
}) async {
  final controller = TextEditingController(text: initial);
  try {
    final result = await showAppDialog<String>(
      context: context,
      title: title,
      description: description,
      content: FTextField(
        control: FTextFieldControl.managed(controller: controller),
        hint: hint,
        autofocus: true,
        onSubmit: (_) => Navigator.of(context).pop(controller.text.trim()),
      ),
      actions: [
        FButton(
          variant: FButtonVariant.outline,
          size: FButtonSizeVariant.sm,
          mainAxisSize: MainAxisSize.min,
          onPress: () => Navigator.of(context).pop(),
          child: Text(cancelLabel),
        ),
        FButton(
          size: FButtonSizeVariant.sm,
          mainAxisSize: MainAxisSize.min,
          onPress: () => Navigator.of(context).pop(controller.text.trim()),
          child: Text(confirmLabel),
        ),
      ],
    );
    if (result == null || result.isEmpty) return null;
    return result;
  } finally {
    controller.dispose();
  }
}

/// 轻量提示。Forui 的 toast 会自己排队，不会像 SnackBar 一样互相覆盖。
void showAppToast(
  BuildContext context,
  String message, {
  bool error = false,
}) {
  if (message.trim().isEmpty) return;
  showFToast(
    context: context,
    // 右下角被播放条占着，提示放右上角，避免挡住播放控制。
    alignment: FToastAlignment.topRight,
    variant: error ? FToastVariant.destructive : FToastVariant.primary,
    icon: Icon(
      error ? FLucideIcons.circleAlert : FLucideIcons.circleCheck,
      size: 18,
      color: error
          ? context.theme.colors.error
          : context.theme.colors.primary,
    ),
    title: Text(message),
  );
}
