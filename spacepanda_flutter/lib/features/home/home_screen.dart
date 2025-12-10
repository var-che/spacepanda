import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import '../../api/mock/mock_data.dart';
import '../../core/theme/app_theme.dart';
import '../../shared/models/models.dart';
import '../spaces/spaces_sidebar.dart';
import '../channels/channels_sidebar.dart';
import '../chat/chat_view.dart';

class HomeScreen extends ConsumerStatefulWidget {
  const HomeScreen({super.key});

  @override
  ConsumerState<HomeScreen> createState() => _HomeScreenState();
}

class _HomeScreenState extends ConsumerState<HomeScreen> {
  Space? selectedSpace;
  Channel? selectedChannel;

  @override
  void initState() {
    super.initState();
    // Select first space and channel by default
    if (MockData.spaces.isNotEmpty) {
      selectedSpace = MockData.spaces.first;
      final channels = MockData.getChannelsForSpace(selectedSpace!.id);
      if (channels.isNotEmpty) {
        selectedChannel = channels.first;
      }
    }
  }

  void _onSpaceSelected(Space space) {
    setState(() {
      selectedSpace = space;
      final channels = MockData.getChannelsForSpace(space.id);
      selectedChannel = channels.isNotEmpty ? channels.first : null;
    });
  }

  void _onChannelSelected(Channel channel) {
    setState(() {
      selectedChannel = channel;
    });
  }

  void _onSendMessage(String content) {
    // TODO: Send message via API
    print('Sending message: $content');
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      body: Row(
        children: [
          // Spaces sidebar (leftmost)
          SpacesSidebar(
            spaces: MockData.spaces,
            selectedSpace: selectedSpace,
            onSpaceSelected: _onSpaceSelected,
          ),
          
          // Channels sidebar
          if (selectedSpace != null)
            ChannelsSidebar(
              space: selectedSpace!,
              channels: MockData.getChannelsForSpace(selectedSpace!.id),
              selectedChannel: selectedChannel,
              onChannelSelected: _onChannelSelected,
            ),
          
          // Chat view (main content)
          Expanded(
            child: selectedChannel != null
                ? ChatView(
                    channel: selectedChannel!,
                    messages: MockData.getMessagesForChannel(selectedChannel!.id),
                    onSendMessage: _onSendMessage,
                  )
                : const Center(
                    child: Text('Select a channel to start chatting'),
                  ),
          ),
        ],
      ),
    );
  }
}
