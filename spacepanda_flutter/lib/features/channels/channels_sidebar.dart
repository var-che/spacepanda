import 'package:flutter/material.dart';
import '../../core/theme/app_theme.dart';
import '../../shared/models/models.dart';

class ChannelsSidebar extends StatelessWidget {
  final Space space;
  final List<Channel> channels;
  final Channel? selectedChannel;
  final Function(Channel) onChannelSelected;

  const ChannelsSidebar({
    super.key,
    required this.space,
    required this.channels,
    required this.selectedChannel,
    required this.onChannelSelected,
  });

  @override
  Widget build(BuildContext context) {
    return Container(
      width: 240,
      color: AppTheme.darkerBackground,
      child: Column(
        children: [
          // Space header
          _SpaceHeader(space: space),
          const Divider(height: 1),
          
          // Channels list
          Expanded(
            child: ListView(
              padding: const EdgeInsets.symmetric(vertical: 8),
              children: [
                _ChannelCategory(title: 'TEXT CHANNELS'),
                ...channels.map((channel) => _ChannelItem(
                      channel: channel,
                      isSelected: selectedChannel?.id == channel.id,
                      onTap: () => onChannelSelected(channel),
                    )),
              ],
            ),
          ),
          
          // User panel at bottom
          _UserPanel(),
        ],
      ),
    );
  }
}

class _SpaceHeader extends StatelessWidget {
  final Space space;

  const _SpaceHeader({required this.space});

  @override
  Widget build(BuildContext context) {
    return Material(
      color: AppTheme.darkerBackground,
      child: InkWell(
        onTap: () {
          // TODO: Show space menu
        },
        child: Container(
          height: 48,
          padding: const EdgeInsets.symmetric(horizontal: 16),
          child: Row(
            children: [
              Expanded(
                child: Text(
                  space.name,
                  style: const TextStyle(
                    fontSize: 16,
                    fontWeight: FontWeight.w600,
                  ),
                  overflow: TextOverflow.ellipsis,
                ),
              ),
              const Icon(Icons.expand_more, size: 20),
            ],
          ),
        ),
      ),
    );
  }
}

class _ChannelCategory extends StatelessWidget {
  final String title;

  const _ChannelCategory({required this.title});

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.fromLTRB(16, 16, 8, 4),
      child: Row(
        children: [
          Expanded(
            child: Text(
              title,
              style: TextStyle(
                fontSize: 12,
                fontWeight: FontWeight.w600,
                color: AppTheme.textMuted,
              ),
            ),
          ),
          Icon(Icons.add, size: 16, color: AppTheme.textMuted),
        ],
      ),
    );
  }
}

class _ChannelItem extends StatefulWidget {
  final Channel channel;
  final bool isSelected;
  final VoidCallback onTap;

  const _ChannelItem({
    required this.channel,
    required this.isSelected,
    required this.onTap,
  });

  @override
  State<_ChannelItem> createState() => _ChannelItemState();
}

class _ChannelItemState extends State<_ChannelItem> {
  bool _isHovered = false;

  @override
  Widget build(BuildContext context) {
    return MouseRegion(
      onEnter: (_) => setState(() => _isHovered = true),
      onExit: (_) => setState(() => _isHovered = false),
      child: Material(
        color: widget.isSelected
            ? AppTheme.channelHover
            : _isHovered
                ? AppTheme.channelHover.withOpacity(0.6)
                : Colors.transparent,
        child: InkWell(
          onTap: widget.onTap,
          child: Container(
            padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 6),
            margin: const EdgeInsets.symmetric(horizontal: 8, vertical: 1),
            child: Row(
              children: [
                Icon(
                  widget.channel.visibility == ChannelVisibility.private
                      ? Icons.lock
                      : Icons.tag,
                  size: 20,
                  color: widget.isSelected
                      ? Colors.white
                      : AppTheme.textMuted,
                ),
                const SizedBox(width: 8),
                Expanded(
                  child: Text(
                    widget.channel.name,
                    style: TextStyle(
                      fontSize: 16,
                      fontWeight: widget.isSelected
                          ? FontWeight.w500
                          : FontWeight.normal,
                      color: widget.isSelected
                          ? Colors.white
                          : const Color(0xFF96989D),
                    ),
                    overflow: TextOverflow.ellipsis,
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

class _UserPanel extends StatelessWidget {
  @override
  Widget build(BuildContext context) {
    return Container(
      height: 52,
      color: AppTheme.darkestBackground.withOpacity(0.3),
      padding: const EdgeInsets.symmetric(horizontal: 8),
      child: Row(
        children: [
          // Avatar
          Container(
            width: 32,
            height: 32,
            decoration: BoxDecoration(
              color: AppTheme.accentColor,
              shape: BoxShape.circle,
            ),
            child: const Center(
              child: Text(
                'A',
                style: TextStyle(
                  color: Colors.white,
                  fontSize: 14,
                  fontWeight: FontWeight.w600,
                ),
              ),
            ),
          ),
          const SizedBox(width: 8),
          // Username and status
          const Expanded(
            child: Column(
              mainAxisAlignment: MainAxisAlignment.center,
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  'Alice',
                  style: TextStyle(
                    fontSize: 14,
                    fontWeight: FontWeight.w600,
                  ),
                ),
                Text(
                  'Online',
                  style: TextStyle(
                    fontSize: 12,
                    color: AppTheme.textMuted,
                  ),
                ),
              ],
            ),
          ),
          // Settings icons
          IconButton(
            icon: const Icon(Icons.mic, size: 20),
            onPressed: () {},
            tooltip: 'Mute',
          ),
          IconButton(
            icon: const Icon(Icons.headset, size: 20),
            onPressed: () {},
            tooltip: 'Deafen',
          ),
          IconButton(
            icon: const Icon(Icons.settings, size: 20),
            onPressed: () {},
            tooltip: 'User Settings',
          ),
        ],
      ),
    );
  }
}
