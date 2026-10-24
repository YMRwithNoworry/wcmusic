import 'package:flutter/widgets.dart';
import 'package:forui/forui.dart';

import '../../../data/services/feedback_service.dart';

/// 问题反馈表单。
///
/// 作为 Forui sheet 的内容使用：字段、错误提示、提交状态都由 Forui 组件承载。
class FeedbackForm extends StatefulWidget {
  const FeedbackForm({super.key, this.feedbackService});

  final FeedbackService? feedbackService;

  @override
  State<FeedbackForm> createState() => _FeedbackFormState();
}

class _FeedbackFormState extends State<FeedbackForm> {
  static const _issueTypes = ['播放问题', '搜索问题', '音源问题', '安装或更新', '功能建议', '其他'];
  static const _platforms = ['Windows 11', 'Windows 10', 'Android', '其他'];

  final _emailController = TextEditingController();
  final _descriptionController = TextEditingController();
  String? _issueType;
  String? _platform;
  String? _issueError;
  String? _platformError;
  String? _emailError;
  String? _descriptionError;
  bool _isSubmitting = false;
  String? _status;
  bool _success = false;

  FeedbackService get _service =>
      widget.feedbackService ?? FormSubmitFeedbackService();

  @override
  void dispose() {
    _emailController.dispose();
    _descriptionController.dispose();
    super.dispose();
  }

  bool _validate() {
    final email = _emailController.text.trim();
    final description = _descriptionController.text.trim();
    setState(() {
      _issueError = _issueType == null ? '请选择问题类型' : null;
      _platformError = _platform == null ? '请选择使用平台' : null;
      _emailError = email.isEmpty
          ? null
          : (RegExp(r'^[^\s@]+@[^\s@]+\.[^\s@]+$').hasMatch(email)
                ? null
                : '请输入有效邮箱');
      _descriptionError = description.length < 10 ? '请至少填写 10 个字' : null;
    });
    return _issueError == null &&
        _platformError == null &&
        _emailError == null &&
        _descriptionError == null;
  }

  Future<void> _submit() async {
    if (_isSubmitting || !_validate()) return;
    setState(() {
      _isSubmitting = true;
      _status = null;
      _success = false;
    });
    try {
      await _service.submit(
        FeedbackRequest(
          issueType: _issueType!,
          platform: _platform!,
          email: _emailController.text.trim(),
          description: _descriptionController.text.trim(),
        ),
      );
      if (!mounted) return;
      _descriptionController.clear();
      setState(() {
        _success = true;
        _status = '已送出，感谢你帮助 WCMusic 变得更好。';
      });
    } on Object {
      if (!mounted) return;
      setState(() {
        _success = false;
        _status = '暂时没有发送成功，请稍后再试。';
      });
    } finally {
      if (mounted) setState(() => _isSubmitting = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    final colors = context.theme.colors;
    final type = context.theme.typography;
    return SingleChildScrollView(
      padding: const EdgeInsets.fromLTRB(22, 18, 22, 24),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Row(
            children: [
              Icon(FLucideIcons.messageSquare, size: 20, color: colors.primary),
              const SizedBox(width: 10),
              Expanded(
                child: Text(
                  '问题反馈',
                  style: type.display.xs.copyWith(
                    color: colors.foreground,
                    fontWeight: FontWeight.w600,
                  ),
                ),
              ),
              FButton.icon(
                variant: FButtonVariant.ghost,
                size: FButtonSizeVariant.sm,
                onPress: _isSubmitting
                    ? null
                    : () => Navigator.of(context).pop(),
                child: const Icon(FLucideIcons.x, size: 16),
              ),
            ],
          ),
          const SizedBox(height: 6),
          Text(
            '请写清出现问题前的操作和你看到的现象。',
            style: type.body.xs.copyWith(color: colors.mutedForeground),
          ),
          const SizedBox(height: 18),
          LayoutBuilder(
            builder: (context, constraints) {
              final compact = constraints.maxWidth < 480;
              final issue = FSelect<String>(
                control: FSelectControl.managed(
                  initial: _issueType,
                  onChange: (value) => setState(() {
                    _issueType = value;
                    _issueError = null;
                  }),
                ),
                label: const Text('问题类型'),
                items: {for (final value in _issueTypes) value: value},
                forceErrorText: _issueError,
              );
              final platform = FSelect<String>(
                control: FSelectControl.managed(
                  initial: _platform,
                  onChange: (value) => setState(() {
                    _platform = value;
                    _platformError = null;
                  }),
                ),
                label: const Text('使用平台'),
                items: {for (final value in _platforms) value: value},
                forceErrorText: _platformError,
              );
              if (compact) {
                return Column(
                  children: [issue, const SizedBox(height: 14), platform],
                );
              }
              return Row(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Expanded(child: issue),
                  const SizedBox(width: 14),
                  Expanded(child: platform),
                ],
              );
            },
          ),
          const SizedBox(height: 14),
          FTextField(
            label: const Text('联系邮箱（选填）'),
            hint: 'you@example.com',
            keyboardType: TextInputType.emailAddress,
            enabled: !_isSubmitting,
            error: _emailError == null ? null : Text(_emailError!),
            control: FTextFieldControl.managed(
              controller: _emailController,
              onChange: (_) {
                if (_emailError != null) setState(() => _emailError = null);
              },
            ),
          ),
          const SizedBox(height: 14),
          FTextField.multiline(
            label: const Text('问题描述'),
            hint: '发生了什么？怎样可以再次遇到这个问题？',
            minLines: 5,
            maxLines: 8,
            maxLength: 2000,
            enabled: !_isSubmitting,
            error: _descriptionError == null
                ? null
                : Text(_descriptionError!),
            control: FTextFieldControl.managed(
              controller: _descriptionController,
              onChange: (_) {
                if (_descriptionError != null) {
                  setState(() => _descriptionError = null);
                }
              },
            ),
          ),
          if (_status != null) ...[
            const SizedBox(height: 10),
            Semantics(
              liveRegion: true,
              child: Text(
                _status!,
                style: type.body.xs.copyWith(
                  color: _success ? colors.primary : colors.error,
                ),
              ),
            ),
          ],
          const SizedBox(height: 18),
          Row(
            mainAxisAlignment: MainAxisAlignment.end,
            children: [
              FButton(
                size: FButtonSizeVariant.sm,
                mainAxisSize: MainAxisSize.min,
                prefix: _isSubmitting
                    ? const SizedBox.square(
                        dimension: 14,
                        child: FCircularProgress(
                          size: FCircularProgressSizeVariant.xs,
                        ),
                      )
                    : const Icon(FLucideIcons.send, size: 15),
                onPress: _isSubmitting ? null : _submit,
                child: Text(_isSubmitting ? '正在发送' : '发送反馈'),
              ),
            ],
          ),
        ],
      ),
    );
  }
}

/// 在 sheet 中展示反馈表单。
Future<void> showFeedbackSheet(BuildContext context) => showFSheet<void>(
  context: context,
  side: FLayout.btt,
  mainAxisMaxRatio: .9,
  builder: (context) => const FeedbackForm(),
);
