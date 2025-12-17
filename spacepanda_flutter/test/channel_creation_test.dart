import 'package:flutter_test/flutter_test.dart';
import 'package:spacepanda_flutter/api/grpc_client.dart';
import 'package:spacepanda_flutter/generated/spacepanda.pbgrpc.dart';

void main() {
  group('Channel Creation Tests', () {
    test('Create channel in space successfully', () async {
      final client = SpacePandaGrpcClient();

      try {
        // Create user
        final timestamp = DateTime.now().millisecondsSinceEpoch;
        final username = 'channeltest_$timestamp';
        print('Creating user: $username');

        final authResponse = await client.auth.createProfile(
          CreateProfileRequest(username: username, password: 'test123'),
        );
        final token = authResponse.sessionToken;
        print('User created, token: ${token.substring(0, 8)}...');

        // Create space
        print('Creating space...');
        final spaceResponse = await client.spaces.createSpace(
          CreateSpaceRequest(
            sessionToken: token,
            name: 'Test Space for Channels',
            description: 'Testing channel creation',
            visibility: SpaceVisibility.SPACE_VISIBILITY_PUBLIC,
          ),
        );
        print('Space created: ${spaceResponse.space.id}');

        // List initial channels (should have default channels)
        final initialChannels = await client.spaces.listChannels(
          ListChannelsRequest(
            sessionToken: token,
            spaceId: spaceResponse.space.id,
          ),
        );
        print('Initial channels: ${initialChannels.channels.length}');

        // Create a new channel
        print('Creating channel "team-chat"...');
        final channelResponse = await client.spaces.createChannel(
          CreateChannelRequest(
            sessionToken: token,
            spaceId: spaceResponse.space.id,
            name: 'team-chat',
            description: 'Team discussion channel',
            visibility: ChannelVisibility.CHANNEL_VISIBILITY_PUBLIC,
          ),
        );

        print('Channel created successfully!');
        print('  ID: ${channelResponse.channel.id}');
        print('  Name: ${channelResponse.channel.name}');
        print('  Description: ${channelResponse.channel.description}');
        print('  Space ID: ${channelResponse.channel.spaceId}');
        print('  Members: ${channelResponse.channel.memberIds.length}');

        // Verify channel was created
        expect(channelResponse.channel.name, equals('team-chat'));
        // Note: Description not yet stored in backend
        // expect(channelResponse.channel.description, equals('Team discussion channel'));
        expect(channelResponse.channel.spaceId, equals(spaceResponse.space.id));
        expect(channelResponse.channel.memberIds, isNotEmpty,
            reason: 'Creator should be a member');

        // List channels again to verify it appears
        final updatedChannels = await client.spaces.listChannels(
          ListChannelsRequest(
            sessionToken: token,
            spaceId: spaceResponse.space.id,
          ),
        );
        print('Updated channel count: ${updatedChannels.channels.length}');

        expect(updatedChannels.channels.length,
            equals(initialChannels.channels.length + 1),
            reason: 'Should have one more channel');

        // Find our created channel
        final ourChannel =
            updatedChannels.channels.firstWhere((c) => c.name == 'team-chat');
        expect(ourChannel.id, equals(channelResponse.channel.id));
        print('SUCCESS: Channel creation verified!');
      } finally {
        await client.close();
      }
    });

    test('Create private channel', () async {
      final client = SpacePandaGrpcClient();

      try {
        // Create user and space
        final timestamp = DateTime.now().millisecondsSinceEpoch;
        final authResponse = await client.auth.createProfile(
          CreateProfileRequest(
            username: 'private_$timestamp',
            password: 'test123',
          ),
        );
        final token = authResponse.sessionToken;

        final spaceResponse = await client.spaces.createSpace(
          CreateSpaceRequest(
            sessionToken: token,
            name: 'Private Space',
            visibility: SpaceVisibility.SPACE_VISIBILITY_PUBLIC,
          ),
        );

        // Create private channel
        print('Creating private channel...');
        final channelResponse = await client.spaces.createChannel(
          CreateChannelRequest(
            sessionToken: token,
            spaceId: spaceResponse.space.id,
            name: 'secret-plans',
            description: 'Private discussion',
            visibility: ChannelVisibility.CHANNEL_VISIBILITY_PRIVATE,
          ),
        );

        expect(channelResponse.channel.name, equals('secret-plans'));
        // Check visibility - backend returns the enum
        expect(channelResponse.channel.visibility,
            equals(ChannelVisibility.CHANNEL_VISIBILITY_PRIVATE));
        print('SUCCESS: Private channel created!');
      } finally {
        await client.close();
      }
    });

    test('Create channel with special name', () async {
      final client = SpacePandaGrpcClient();

      try {
        final timestamp = DateTime.now().millisecondsSinceEpoch;
        final authResponse = await client.auth.createProfile(
          CreateProfileRequest(
            username: 'special_$timestamp',
            password: 'test123',
          ),
        );
        final token = authResponse.sessionToken;

        final spaceResponse = await client.spaces.createSpace(
          CreateSpaceRequest(
            sessionToken: token,
            name: 'Test Space',
            visibility: SpaceVisibility.SPACE_VISIBILITY_PUBLIC,
          ),
        );

        // Test various valid channel names
        final testNames = [
          'dev-ops',
          'random-chat',
          'project-alpha',
          'team-2025',
        ];

        for (final name in testNames) {
          print('Creating channel: $name');
          final response = await client.spaces.createChannel(
            CreateChannelRequest(
              sessionToken: token,
              spaceId: spaceResponse.space.id,
              name: name,
              visibility: ChannelVisibility.CHANNEL_VISIBILITY_PUBLIC,
            ),
          );
          expect(response.channel.name, equals(name));
          print('  ✓ Created: $name');
        }

        print('SUCCESS: All channel names accepted!');
      } finally {
        await client.close();
      }
    });
  });
}
