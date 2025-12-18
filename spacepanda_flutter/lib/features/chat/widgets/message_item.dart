import 'package:flutter/material.dart';
import 'package:intl/intl.dart';
import '../../../core/theme/app_theme.dart';
import '../../../shared/models/models.dart';

class MessageItem extends StatefulWidget {
  final Message message;
  final bool showAvatar;
  final String? senderName;

  const MessageItem({
    super.key,
    required this.message,
    required this.showAvatar,
    this.senderName,
  });

  @override
  State<MessageItem> createState() => _MessageItemState();
}

class _MessageItemState extends State<MessageItem> {
  bool _isHovered = false;

  @override
  Widget build(BuildContext context) {
    // Use sender name from message or provided name
    final displayName =
        widget.senderName ?? widget.message.senderId.substring(0, 8);

    return MouseRegion(
      onEnter: (_) => setState(() => _isHovered = true),
      onExit: (_) => setState(() => _isHovered = false),
      child: Container(
        padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 2),
        color: _isHovered ? Colors.black.withOpacity(0.05) : Colors.transparent,
        child: Row(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            // Avatar or spacing
            if (widget.showAvatar)
              _UserAvatar(displayName: displayName)
            else
              const SizedBox(width: 40),

            const SizedBox(width: 16),

            // Message content
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  if (widget.showAvatar) ...[
                    _MessageHeader(
                      displayName: displayName,
                      timestamp: widget.message.timestamp,
                    ),
                    const SizedBox(height: 2),
                  ],
                  _MessageContent(
                    content: widget.message.content,
                    isEncrypted: widget.message.isE2ee,
                  ),
                ],
              ),
            ),

            // Message actions (visible on hover)
            if (_isHovered) _MessageActions(),
          ],
        ),
      ),
    );
  }
}

class _UserAvatar extends StatelessWidget {
  final String displayName;

  const _UserAvatar({required this.displayName});

  @override
  Widget build(BuildContext context) {
    return Container(
      width: 40,
      height: 40,
      decoration: const BoxDecoration(
        color: AppTheme.accentColor,
        shape: BoxShape.circle,
      ),
      child: Center(
        child: Text(
          displayName.isNotEmpty ? displayName[0].toUpperCase() : '?',
          style: const TextStyle(
            color: Colors.white,
            fontSize: 16,
            fontWeight: FontWeight.w600,
          ),
        ),
      ),
    );
  }
}

class _MessageHeader extends StatelessWidget {
  final String displayName;
  final DateTime timestamp;

  const _MessageHeader({
    required this.displayName,
    required this.timestamp,
  });

  @override
  Widget build(BuildContext context) {
    final timeStr = DateFormat('HH:mm').format(timestamp);

    return Row(
      children: [
        Text(
          displayName,
          style: const TextStyle(
            fontSize: 16,
            fontWeight: FontWeight.w500,
          ),
        ),
        const SizedBox(width: 8),
        Text(
          timeStr,
          style: const TextStyle(
            fontSize: 12,
            color: AppTheme.textMuted,
          ),
        ),
      ],
    );
  }
}

class _MessageContent extends StatelessWidget {
  final String content;
  final bool isEncrypted;

  const _MessageContent({
    required this.content,
    required this.isEncrypted,
  });

  @override
  Widget build(BuildContext context) {
    return Row(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Expanded(
          child: Text(
            content,
            style: const TextStyle(
              fontSize: 16,
              height: 1.375,
            ),
          ),
        ),
        if (isEncrypted) ...[
          const SizedBox(width: 8),
          const Tooltip(
            message: 'End-to-end encrypted',
            child: Icon(
              Icons.lock,
              size: 14,
              color: AppTheme.successColor,
            ),
          ),
        ],
      ],
    );
  }
}

class _MessageActions extends StatelessWidget {
  @override
  Widget build(BuildContext context) {
    return Container(
      decoration: BoxDecoration(
        color: AppTheme.darkerBackground,
        borderRadius: BorderRadius.circular(4),
        border: Border.all(
          color: AppTheme.darkestBackground,
        ),
      ),
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          IconButton(
            icon: const Icon(Icons.add_reaction_outlined, size: 18),
            onPressed: () {},
            tooltip: 'Add Reaction',
            padding: const EdgeInsets.all(4),
            constraints: const BoxConstraints(
              minWidth: 32,
              minHeight: 32,
            ),
          ),
          IconButton(
            icon: const Icon(Icons.edit, size: 18),
            onPressed: () {},
            tooltip: 'Edit Message',
            padding: const EdgeInsets.all(4),
            constraints: const BoxConstraints(
              minWidth: 32,
              minHeight: 32,
            ),
          ),
          IconButton(
            icon: const Icon(Icons.more_horiz, size: 18),
            onPressed: () {},
            tooltip: 'More',
            padding: const EdgeInsets.all(4),
            constraints: const BoxConstraints(
              minWidth: 32,
              minHeight: 32,
            ),
          ),
        ],
      ),
    );
  }
}
