import 'package:flutter/material.dart';
import '../../core/theme/app_theme.dart';
import '../../shared/models/models.dart';
import 'widgets/message_item.dart';
import 'widgets/message_input.dart';

class ChatView extends StatelessWidget {
  final Channel channel;
  final List<Message> messages;
  final Function(String) onSendMessage;

  const ChatView({
    super.key,
    required this.channel,
    required this.messages,
    required this.onSendMessage,
  });

  @override
  Widget build(BuildContext context) {
    return Container(
      color: AppTheme.darkBackground,
      child: Column(
        children: [
          // Channel header
          _ChannelHeader(channel: channel),
          const Divider(height: 1),
          
          // Messages list
          Expanded(
            child: ListView.builder(
              padding: const EdgeInsets.all(16),
              itemCount: messages.length,
              itemBuilder: (context, index) {
                final message = messages[index];
                final showAvatar = index == 0 ||
                    messages[index - 1].senderId != message.senderId;
                
                return MessageItem(
                  message: message,
                  showAvatar: showAvatar,
                );
              },
            ),
          ),
          
          // Message input
          MessageInput(onSend: onSendMessage),
        ],
      ),
    );
  }
}

class _ChannelHeader extends StatelessWidget {
  final Channel channel;

  const _ChannelHeader({required this.channel});

  @override
  Widget build(BuildContext context) {
    return Container(
      height: 48,
      padding: const EdgeInsets.symmetric(horizontal: 16),
      child: Row(
        children: [
          Icon(
            channel.visibility == ChannelVisibility.private
                ? Icons.lock
                : Icons.tag,
            size: 24,
            color: AppTheme.textMuted,
          ),
          const SizedBox(width: 8),
          Text(
            channel.name,
            style: const TextStyle(
              fontSize: 16,
              fontWeight: FontWeight.w600,
            ),
          ),
          if (channel.description?.isNotEmpty ?? false) ...[
            const SizedBox(width: 8),
            Container(
              width: 1,
              height: 24,
              color: AppTheme.textMuted.withOpacity(0.3),
            ),
            const SizedBox(width: 8),
            Expanded(
              child: Text(
                channel.description!,
                style: TextStyle(
                  fontSize: 14,
                  color: AppTheme.textMuted,
                ),
                overflow: TextOverflow.ellipsis,
              ),
            ),
          ],
          const Spacer(),
          // Channel actions
          IconButton(
            icon: const Icon(Icons.notifications_outlined, size: 20),
            onPressed: () {},
            tooltip: 'Notification Settings',
          ),
          IconButton(
            icon: const Icon(Icons.push_pin_outlined, size: 20),
            onPressed: () {},
            tooltip: 'Pinned Messages',
          ),
          IconButton(
            icon: const Icon(Icons.people_outline, size: 20),
            onPressed: () {},
            tooltip: 'Show Member List',
          ),
          IconButton(
            icon: const Icon(Icons.search, size: 20),
            onPressed: () {},
            tooltip: 'Search',
          ),
        ],
      ),
    );
  }
}
