import 'package:flutter/material.dart';

import '../../../data/services/feedback_service.dart';

class FeedbackDialog extends StatefulWidget {
  const FeedbackDialog({super.key, this.feedbackService});

  final FeedbackService? feedbackService;

  @override
  State<FeedbackDialog> createState() => _FeedbackDialogState();
}

class _FeedbackDialogState extends State<FeedbackDialog> {
  static const _issueTypes = ['播放问题', '搜索问题', '音源问题', '安装或更新', '功能建议', '其他'];
  static const _platforms = ['Windows 11', 'Windows 10', 'Android', '其他'];

  final _formKey = GlobalKey<FormState>();
  final _emailController = TextEditingController();
  final _descriptionController = TextEditingController();
  String? _issueType;
  String? _platform;
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

  Future<void> _submit() async {
    if (_isSubmitting || !_formKey.currentState!.validate()) return;
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
    final colors = Theme.of(context).colorScheme;
    return Dialog(
      child: ConstrainedBox(
        constraints: const BoxConstraints(maxWidth: 620),
        child: SingleChildScrollView(
          padding: const EdgeInsets.fromLTRB(24, 18, 24, 24),
          child: Form(
            key: _formKey,
            child: Column(
              mainAxisSize: MainAxisSize.min,
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                Row(
                  children: [
                    Icon(Icons.feedback_outlined, color: colors.primary),
                    const SizedBox(width: 12),
                    Expanded(
                      child: Text(
                        '问题反馈',
                        style: Theme.of(context).textTheme.headlineSmall,
                      ),
                    ),
                    IconButton(
                      onPressed: _isSubmitting
                          ? null
                          : () => Navigator.of(context).pop(),
                      tooltip: '关闭反馈',
                      icon: const Icon(Icons.close),
                    ),
                  ],
                ),
                const Padding(
                  padding: EdgeInsets.only(top: 4, bottom: 20),
                  child: Text('请写清出现问题前的操作和你看到的现象。'),
                ),
                LayoutBuilder(
                  builder: (context, constraints) {
                    final compact = constraints.maxWidth < 480;
                    final fields = [
                      Expanded(
                        child: DropdownButtonFormField<String>(
                          initialValue: _issueType,
                          decoration: const InputDecoration(labelText: '问题类型'),
                          items: [
                            for (final value in _issueTypes)
                              DropdownMenuItem(
                                value: value,
                                child: Text(value),
                              ),
                          ],
                          onChanged: _isSubmitting
                              ? null
                              : (value) => setState(() => _issueType = value),
                          validator: (value) =>
                              value == null ? '请选择问题类型' : null,
                        ),
                      ),
                      Expanded(
                        child: DropdownButtonFormField<String>(
                          initialValue: _platform,
                          decoration: const InputDecoration(labelText: '使用平台'),
                          items: [
                            for (final value in _platforms)
                              DropdownMenuItem(
                                value: value,
                                child: Text(value),
                              ),
                          ],
                          onChanged: _isSubmitting
                              ? null
                              : (value) => setState(() => _platform = value),
                          validator: (value) =>
                              value == null ? '请选择使用平台' : null,
                        ),
                      ),
                    ];
                    if (compact) {
                      return Column(
                        children: [
                          for (
                            var index = 0;
                            index < fields.length;
                            index++
                          ) ...[
                            Row(children: [fields[index]]),
                            if (index == 0) const SizedBox(height: 14),
                          ],
                        ],
                      );
                    }
                    return Row(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        fields.first,
                        const SizedBox(width: 14),
                        fields.last,
                      ],
                    );
                  },
                ),
                const SizedBox(height: 14),
                TextFormField(
                  controller: _emailController,
                  enabled: !_isSubmitting,
                  keyboardType: TextInputType.emailAddress,
                  autofillHints: const [AutofillHints.email],
                  decoration: const InputDecoration(
                    labelText: '联系邮箱（选填）',
                    hintText: 'you@example.com',
                  ),
                  validator: (value) {
                    final email = value?.trim() ?? '';
                    if (email.isEmpty) return null;
                    return RegExp(r'^[^\s@]+@[^\s@]+\.[^\s@]+$').hasMatch(email)
                        ? null
                        : '请输入有效邮箱';
                  },
                ),
                const SizedBox(height: 14),
                TextFormField(
                  controller: _descriptionController,
                  enabled: !_isSubmitting,
                  minLines: 5,
                  maxLines: 8,
                  maxLength: 2000,
                  decoration: const InputDecoration(
                    labelText: '问题描述',
                    alignLabelWithHint: true,
                    hintText: '发生了什么？怎样可以再次遇到这个问题？',
                  ),
                  validator: (value) =>
                      (value?.trim().length ?? 0) < 10 ? '请至少填写 10 个字' : null,
                ),
                if (_status != null) ...[
                  const SizedBox(height: 4),
                  Semantics(
                    liveRegion: true,
                    child: Text(
                      _status!,
                      style: TextStyle(
                        color: _success ? colors.primary : colors.error,
                      ),
                    ),
                  ),
                ],
                const SizedBox(height: 16),
                Align(
                  alignment: Alignment.centerRight,
                  child: FilledButton.icon(
                    onPressed: _isSubmitting ? null : _submit,
                    icon: _isSubmitting
                        ? const SizedBox.square(
                            dimension: 16,
                            child: CircularProgressIndicator(strokeWidth: 2),
                          )
                        : const Icon(Icons.send_outlined),
                    label: Text(_isSubmitting ? '正在发送' : '发送反馈'),
                  ),
                ),
              ],
            ),
          ),
        ),
      ),
    );
  }
}
