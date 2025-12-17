import 'dart:io';
import 'package:grpc/grpc.dart';
import '../grpc_client.dart';
import '../../generated/spacepanda.pbgrpc.dart';
import '../../shared/models/models.dart' as app;

class SpaceRepository {
  final SpacePandaGrpcClient _client;
  static final _logFile = File('/tmp/spacepanda_flutter_debug.log');

  SpaceRepository(this._client);

  void _log(String message) {
    final timestamp = DateTime.now().toIso8601String();
    final logMessage = '[$timestamp] $message\n';
    print(message);
    try {
      _logFile.writeAsStringSync(logMessage, mode: FileMode.append);
    } catch (e) {
      print('Failed to write to log file: $e');
    }
  }

  /// List all spaces the user is a member of
  Future<List<app.Space>> listSpaces(String sessionToken) async {
    try {
      _log(
          'SpaceRepository: Listing spaces with token: ${sessionToken.substring(0, 8)}...');
      final response = await _client.spaces.listSpaces(
        ListSpacesRequest(sessionToken: sessionToken),
      );
      _log('SpaceRepository: Got ${response.spaces.length} spaces');
      return response.spaces.map(_mapSpace).toList();
    } on GrpcError catch (e) {
      _log(
          'SpaceRepository: GrpcError listing spaces - code: ${e.code}, message: ${e.message}');
      throw SpaceException('Failed to list spaces: ${e.message}');
    } catch (e, stackTrace) {
      _log('SpaceRepository: Unexpected error listing spaces - $e');
      _log('Stack trace: $stackTrace');
      throw SpaceException('Error listing spaces: $e');
    }
  }

  /// List channels in a space
  Future<List<app.Channel>> listChannels(
    String sessionToken,
    String spaceId,
  ) async {
    try {
      final response = await _client.spaces.listChannels(
        ListChannelsRequest(
          sessionToken: sessionToken,
          spaceId: spaceId,
        ),
      );
      return response.channels.map(_mapChannel).toList();
    } on GrpcError catch (e) {
      throw SpaceException('Failed to list channels: ${e.message}');
    }
  }

  /// Create a new space
  Future<app.Space> createSpace({
    required String sessionToken,
    required String name,
    String? description,
    bool isPublic = false,
  }) async {
    try {
      final response = await _client.spaces.createSpace(
        CreateSpaceRequest(
          sessionToken: sessionToken,
          name: name,
          description: description,
          visibility: isPublic
              ? SpaceVisibility.SPACE_VISIBILITY_PUBLIC
              : SpaceVisibility.SPACE_VISIBILITY_PRIVATE,
        ),
      );
      return _mapSpace(response.space);
    } on GrpcError catch (e) {
      throw SpaceException('Failed to create space: ${e.message}');
    }
  }

  /// Get a specific space by ID
  Future<app.Space> getSpace(String sessionToken, String spaceId) async {
    try {
      final response = await _client.spaces.getSpace(
        GetSpaceRequest(
          sessionToken: sessionToken,
          spaceId: spaceId,
        ),
      );
      return _mapSpace(response);
    } on GrpcError catch (e) {
      throw SpaceException('Failed to get space: ${e.message}');
    }
  }

  /// Create a new channel in a space
  Future<app.Channel> createChannel({
    required String sessionToken,
    required String spaceId,
    required String name,
    String? description,
    bool isPublic = true,
  }) async {
    try {
      _log('SpaceRepository: Creating channel "$name" in space $spaceId');
      final response = await _client.spaces.createChannel(
        CreateChannelRequest(
          sessionToken: sessionToken,
          spaceId: spaceId,
          name: name,
          description: description ?? '',
          visibility: isPublic
              ? ChannelVisibility.CHANNEL_VISIBILITY_PUBLIC
              : ChannelVisibility.CHANNEL_VISIBILITY_PRIVATE,
        ),
      );
      _log(
          'SpaceRepository: Channel created successfully - ${response.channel.id}');
      return _mapChannel(response.channel);
    } on GrpcError catch (e) {
      _log(
          'SpaceRepository: GrpcError creating channel - code: ${e.code}, message: ${e.message}');
      throw SpaceException('Failed to create channel: ${e.message}');
    } catch (e, stackTrace) {
      _log('SpaceRepository: Unexpected error creating channel - $e');
      _log('Stack trace: $stackTrace');
      throw SpaceException('Error creating channel: $e');
    }
  }

  // Helper methods to map proto models to app models
  app.Space _mapSpace(Space protoSpace) {
    return app.Space(
      id: protoSpace.id,
      name: protoSpace.name,
      description:
          protoSpace.description.isEmpty ? null : protoSpace.description,
      iconUrl: protoSpace.iconUrl.isEmpty ? null : protoSpace.iconUrl,
      visibility: _mapSpaceVisibility(protoSpace.visibility),
      ownerId: protoSpace.ownerId,
      memberIds: protoSpace.memberIds,
      channelIds: protoSpace.channelIds,
      createdAt:
          DateTime.fromMillisecondsSinceEpoch(protoSpace.createdAt.toInt()),
    );
  }

  app.Channel _mapChannel(Channel protoChannel) {
    return app.Channel(
      id: protoChannel.id,
      spaceId: protoChannel.spaceId,
      name: protoChannel.name,
      description:
          protoChannel.description.isEmpty ? null : protoChannel.description,
      visibility: _mapChannelVisibility(protoChannel.visibility),
      memberIds: protoChannel.memberIds,
      createdAt:
          DateTime.fromMillisecondsSinceEpoch(protoChannel.createdAt.toInt()),
    );
  }

  app.SpaceVisibility _mapSpaceVisibility(SpaceVisibility proto) {
    return proto == SpaceVisibility.SPACE_VISIBILITY_PUBLIC
        ? app.SpaceVisibility.public
        : app.SpaceVisibility.private;
  }

  app.ChannelVisibility _mapChannelVisibility(ChannelVisibility proto) {
    return proto == ChannelVisibility.CHANNEL_VISIBILITY_PUBLIC
        ? app.ChannelVisibility.public
        : app.ChannelVisibility.private;
  }
}

class SpaceException implements Exception {
  final String message;
  SpaceException(this.message);

  @override
  String toString() => message;
}
