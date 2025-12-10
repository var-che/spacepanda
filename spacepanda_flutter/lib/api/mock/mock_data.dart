import '../models/models.dart';

/// Mock data for UI development
class MockData {
  static final currentUser = User(
    id: 'user-1',
    username: 'alice',
    displayName: 'Alice',
    status: UserStatus.online,
  );

  static final users = [
    currentUser,
    User(
      id: 'user-2',
      username: 'bob',
      displayName: 'Bob',
      status: UserStatus.idle,
    ),
    User(
      id: 'user-3',
      username: 'charlie',
      displayName: 'Charlie',
      status: UserStatus.online,
    ),
    User(
      id: 'user-4',
      username: 'diana',
      displayName: 'Diana',
      status: UserStatus.dnd,
    ),
  ];

  static final spaces = [
    Space(
      id: 'space-1',
      name: 'Engineering Team',
      description: 'Development discussions',
      visibility: SpaceVisibility.private,
      ownerId: 'user-1',
      memberIds: ['user-1', 'user-2', 'user-3'],
      channelIds: ['channel-1', 'channel-2', 'channel-3'],
      createdAt: DateTime.now().subtract(const Duration(days: 30)),
    ),
    Space(
      id: 'space-2',
      name: 'Gaming Squad',
      description: 'Lets game!',
      visibility: SpaceVisibility.private,
      ownerId: 'user-2',
      memberIds: ['user-1', 'user-2', 'user-4'],
      channelIds: ['channel-4', 'channel-5'],
      createdAt: DateTime.now().subtract(const Duration(days: 15)),
    ),
    Space(
      id: 'space-3',
      name: 'Open Source',
      description: 'Public project discussions',
      visibility: SpaceVisibility.public,
      ownerId: 'user-3',
      memberIds: ['user-1', 'user-2', 'user-3', 'user-4'],
      channelIds: ['channel-6'],
      createdAt: DateTime.now().subtract(const Duration(days: 7)),
    ),
  ];

  static final channels = [
    // Engineering Team channels
    Channel(
      id: 'channel-1',
      spaceId: 'space-1',
      name: 'general',
      description: 'General discussion',
      visibility: ChannelVisibility.public,
      memberIds: ['user-1', 'user-2', 'user-3'],
      createdAt: DateTime.now().subtract(const Duration(days: 30)),
    ),
    Channel(
      id: 'channel-2',
      spaceId: 'space-1',
      name: 'backend',
      description: 'Backend development',
      visibility: ChannelVisibility.public,
      memberIds: ['user-1', 'user-2'],
      createdAt: DateTime.now().subtract(const Duration(days: 25)),
    ),
    Channel(
      id: 'channel-3',
      spaceId: 'space-1',
      name: 'frontend',
      description: 'Frontend development',
      visibility: ChannelVisibility.public,
      memberIds: ['user-1', 'user-3'],
      createdAt: DateTime.now().subtract(const Duration(days: 20)),
    ),
    // Gaming Squad channels
    Channel(
      id: 'channel-4',
      spaceId: 'space-2',
      name: 'game-chat',
      visibility: ChannelVisibility.public,
      memberIds: ['user-1', 'user-2', 'user-4'],
      createdAt: DateTime.now().subtract(const Duration(days: 15)),
    ),
    Channel(
      id: 'channel-5',
      spaceId: 'space-2',
      name: 'strategy',
      visibility: ChannelVisibility.private,
      memberIds: ['user-2', 'user-4'],
      createdAt: DateTime.now().subtract(const Duration(days: 10)),
    ),
    // Open Source channel
    Channel(
      id: 'channel-6',
      spaceId: 'space-3',
      name: 'announcements',
      visibility: ChannelVisibility.public,
      memberIds: ['user-1', 'user-2', 'user-3', 'user-4'],
      createdAt: DateTime.now().subtract(const Duration(days: 7)),
    ),
  ];

  static final messages = [
    Message(
      id: 'msg-1',
      channelId: 'channel-1',
      senderId: 'user-2',
      content: 'Hey team! How is the MLS integration going?',
      timestamp: DateTime.now().subtract(const Duration(hours: 2)),
      isEncrypted: true,
    ),
    Message(
      id: 'msg-2',
      channelId: 'channel-1',
      senderId: 'user-1',
      content: 'Almost done! Just finished the async manager integration.',
      timestamp: DateTime.now().subtract(const Duration(hours: 1, minutes: 55)),
      isEncrypted: true,
    ),
    Message(
      id: 'msg-3',
      channelId: 'channel-1',
      senderId: 'user-3',
      content: 'Nice! Let me know when you need help with the UI.',
      timestamp: DateTime.now().subtract(const Duration(hours: 1, minutes: 50)),
      isEncrypted: true,
    ),
    Message(
      id: 'msg-4',
      channelId: 'channel-1',
      senderId: 'user-1',
      content: 'Actually, we are starting the Flutter app now! 🚀',
      timestamp: DateTime.now().subtract(const Duration(minutes: 30)),
      isEncrypted: true,
    ),
    Message(
      id: 'msg-5',
      channelId: 'channel-1',
      senderId: 'user-2',
      content: 'Awesome! Going with gRPC for the backend communication?',
      timestamp: DateTime.now().subtract(const Duration(minutes: 25)),
      isEncrypted: true,
    ),
    Message(
      id: 'msg-6',
      channelId: 'channel-1',
      senderId: 'user-1',
      content: 'Yes! gRPC for efficiency and Riverpod for state management.',
      timestamp: DateTime.now().subtract(const Duration(minutes: 20)),
      isEncrypted: true,
    ),
  ];

  static User getUserById(String id) {
    return users.firstWhere((u) => u.id == id, orElse: () => users.first);
  }

  static Space getSpaceById(String id) {
    return spaces.firstWhere((s) => s.id == id, orElse: () => spaces.first);
  }

  static Channel getChannelById(String id) {
    return channels.firstWhere((c) => c.id == id, orElse: () => channels.first);
  }

  static List<Channel> getChannelsForSpace(String spaceId) {
    return channels.where((c) => c.spaceId == spaceId).toList();
  }

  static List<Message> getMessagesForChannel(String channelId) {
    return messages.where((m) => m.channelId == channelId).toList();
  }
}
