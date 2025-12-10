import 'package:flutter/material.dart';
import '../../../core/theme/app_theme.dart';

class MessageInput extends StatefulWidget {
  final Function(String) onSend;

  const MessageInput({
    super.key,
    required this.onSend,
  });

  @override
  State<MessageInput> createState() => _MessageInputState();
}

class _MessageInputState extends State<MessageInput> {
  final TextEditingController _controller = TextEditingController();
  bool _hasText = false;

  @override
  void initState() {
    super.initState();
    _controller.addListener(_onTextChanged);
  }

  @override
  void dispose() {
    _controller.removeListener(_onTextChanged);
    _controller.dispose();
    super.dispose();
  }

  void _onTextChanged() {
    setState(() {
      _hasText = _controller.text.trim().isNotEmpty;
    });
  }

  void _sendMessage() {
    final text = _controller.text.trim();
    if (text.isNotEmpty) {
      widget.onSend(text);
      _controller.clear();
    }
  }

  @override
  Widget build(BuildContext context) {
    return Container(
      padding: const EdgeInsets.all(16),
      child: Container(
        decoration: BoxDecoration(
          color: AppTheme.darkerBackground,
          borderRadius: BorderRadius.circular(8),
        ),
        child: Row(
          crossAxisAlignment: CrossAxisAlignment.end,
          children: [
            // Attachment button
            IconButton(
              icon: const Icon(Icons.add_circle_outline, size: 24),
              onPressed: () {},
              tooltip: 'Add attachments',
            ),
            
            // Text input
            Expanded(
              child: TextField(
                controller: _controller,
                maxLines: null,
                textInputAction: TextInputAction.newline,
                decoration: const InputDecoration(
                  hintText: 'Message...',
                  border: InputBorder.none,
                  contentPadding: EdgeInsets.symmetric(
                    horizontal: 8,
                    vertical: 12,
                  ),
                ),
                onSubmitted: (_) => _sendMessage(),
              ),
            ),
            
            // Emoji picker button
            IconButton(
              icon: const Icon(Icons.emoji_emotions_outlined, size: 24),
              onPressed: () {},
              tooltip: 'Emoji',
            ),
            
            // Send button (only visible when there's text)
            if (_hasText)
              IconButton(
                icon: Icon(
                  Icons.send,
                  size: 24,
                  color: AppTheme.accentColor,
                ),
                onPressed: _sendMessage,
                tooltip: 'Send message',
              ),
          ],
        ),
      ),
    );
  }
}
