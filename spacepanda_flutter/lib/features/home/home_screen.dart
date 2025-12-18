import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import '../../shared/models/models.dart';
import '../../providers/space_providers.dart';
import '../../providers/message_providers.dart';
import '../../providers/api_providers.dart';
import '../spaces/spaces_sidebar.dart';
import '../channels/channels_sidebar.dart';
import '../chat/chat_view.dart';
import '../../core/debug/debug_panel.dart';

class HomeScreen extends ConsumerStatefulWidget {
  const HomeScreen({super.key});

  @override
  ConsumerState<HomeScreen> createState() => _HomeScreenState();
}

class _HomeScreenState extends ConsumerState<HomeScreen> {
  Space? selectedSpace;
  Channel? selectedChannel;

  void _onSpaceSelected(Space space) {
    setState(() {
      selectedSpace = space;
      selectedChannel = null; // Reset channel when space changes
    });
  }

  void _onChannelSelected(Channel channel) {
    setState(() {
      selectedChannel = channel;
    });
  }

  Future<void> _onSendMessage(String content) async {
    if (selectedChannel == null) return;

    try {
      await ref.read(messageSenderProvider.notifier).sendMessage(
            channelId: selectedChannel!.id,
            content: content,
          );
    } catch (e) {
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(content: Text('Failed to send message: $e')),
        );
      }
    }
  }

  @override
  Widget build(BuildContext context) {
    final spacesAsync = ref.watch(spacesProvider);
    final sessionToken = ref.watch(sessionTokenProvider);
    final isAuthenticated = ref.watch(isAuthenticatedProvider);

    // Debug: Log state on build
    debugPrint(
        'HomeScreen.build() - isAuthenticated: $isAuthenticated, hasToken: ${sessionToken != null}');
    if (sessionToken != null) {
      debugPrint(
          'HomeScreen.build() - token: ${sessionToken.substring(0, 8)}...');
    }

    return Scaffold(
      body: Column(
        children: [
          // Debug panel at top
          const DebugPanel(),

          // Main content
          Expanded(
            child: spacesAsync.when(
              data: (spaces) {
                // Auto-select first space and channel if none selected
                if (selectedSpace == null && spaces.isNotEmpty) {
                  WidgetsBinding.instance.addPostFrameCallback((_) {
                    if (mounted) {
                      setState(() {
                        selectedSpace = spaces.first;
                      });
                    }
                  });
                }

                return Row(
                  children: [
                    // Spaces sidebar (leftmost)
                    SpacesSidebar(
                      spaces: spaces,
                      selectedSpace: selectedSpace,
                      onSpaceSelected: _onSpaceSelected,
                    ),

                    // Channels sidebar
                    if (selectedSpace != null)
                      _buildChannelsSidebar(selectedSpace!),

                    // Chat view (main content)
                    Expanded(
                      child: selectedChannel != null
                          ? _buildChatView(selectedChannel!)
                          : const Center(
                              child: Text('Select a channel to start chatting'),
                            ),
                    ),
                  ],
                );
              },
              loading: () => const Center(child: CircularProgressIndicator()),
              error: (error, stack) => Center(
                child: Column(
                  mainAxisAlignment: MainAxisAlignment.center,
                  children: [
                    const Icon(Icons.error_outline,
                        size: 48, color: Colors.red),
                    const SizedBox(height: 16),
                    Text('Error loading spaces: $error'),
                    const SizedBox(height: 16),
                    ElevatedButton(
                      onPressed: () => ref.invalidate(spacesProvider),
                      child: const Text('Retry'),
                    ),
                  ],
                ),
              ),
            ),
          ),
        ],
      ),
    );
  }

  Widget _buildChannelsSidebar(Space space) {
    final channelsAsync = ref.watch(spaceChannelsProvider(space.id));

    return channelsAsync.when(
      data: (channels) {
        // Auto-select first channel if none selected
        if (selectedChannel == null && channels.isNotEmpty) {
          WidgetsBinding.instance.addPostFrameCallback((_) {
            if (mounted) {
              setState(() {
                selectedChannel = channels.first;
              });
            }
          });
        }

        return ChannelsSidebar(
          space: space,
          channels: channels,
          selectedChannel: selectedChannel,
          onChannelSelected: _onChannelSelected,
        );
      },
      loading: () => const SizedBox(
        width: 240,
        child: Center(child: CircularProgressIndicator()),
      ),
      error: (error, stack) => SizedBox(
        width: 240,
        child: Center(
          child: Column(
            mainAxisAlignment: MainAxisAlignment.center,
            children: [
              const Icon(Icons.error_outline, color: Colors.red),
              const SizedBox(height: 8),
              Text('Error: $error', textAlign: TextAlign.center),
            ],
          ),
        ),
      ),
    );
  }

  Widget _buildChatView(Channel channel) {
    final messagesAsync = ref.watch(channelMessagesProvider(channel.id));

    return messagesAsync.when(
      data: (messages) => ChatView(
        channel: channel,
        messages: messages,
        onSendMessage: _onSendMessage,
      ),
      loading: () => const Center(child: CircularProgressIndicator()),
      error: (error, stack) => Center(
        child: Column(
          mainAxisAlignment: MainAxisAlignment.center,
          children: [
            const Icon(Icons.error_outline, color: Colors.red),
            const SizedBox(height: 8),
            Text('Error loading messages: $error'),
          ],
        ),
      ),
    );
  }
}
