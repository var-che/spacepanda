import 'package:flutter/foundation.dart';
import 'package:riverpod_annotation/riverpod_annotation.dart';
import '../shared/models/models.dart';
import 'api_providers.dart';

part 'space_providers.g.dart';

/// List of spaces the user is a member of
@riverpod
Future<List<Space>> spaces(SpacesRef ref) async {
  final sessionToken = ref.watch(sessionTokenProvider);
  if (sessionToken == null) {
    throw Exception('Not authenticated');
  }

  final repository = ref.watch(spaceRepositoryProvider);
  return await repository.listSpaces(sessionToken);
}

/// List of channels in a specific space
@riverpod
Future<List<Channel>> spaceChannels(
  SpaceChannelsRef ref,
  String spaceId,
) async {
  final sessionToken = ref.watch(sessionTokenProvider);
  if (sessionToken == null) {
    throw Exception('Not authenticated');
  }

  final repository = ref.watch(spaceRepositoryProvider);
  return await repository.listChannels(sessionToken, spaceId);
}

/// Get a specific space by ID
@riverpod
Future<Space> space(SpaceRef ref, String spaceId) async {
  final sessionToken = ref.watch(sessionTokenProvider);
  if (sessionToken == null) {
    throw Exception('Not authenticated');
  }

  final repository = ref.watch(spaceRepositoryProvider);
  return await repository.getSpace(sessionToken, spaceId);
}

/// Create a new space
@riverpod
class SpaceCreator extends _$SpaceCreator {
  @override
  FutureOr<Space?> build() {
    debugPrint('SpaceCreator: Initialized');
    return null;
  }

  Future<Space> createSpace({
    required String name,
    String? description,
    bool isPublic = false,
  }) async {
    debugPrint('SpaceCreator: Creating space "$name" (public: $isPublic)');

    final sessionToken = ref.read(sessionTokenProvider);
    if (sessionToken == null) {
      debugPrint('SpaceCreator: ERROR - Not authenticated');
      throw Exception('Not authenticated');
    }

    state = const AsyncValue.loading();
    debugPrint('SpaceCreator: Set state to loading');

    try {
      final repository = ref.read(spaceRepositoryProvider);
      debugPrint('SpaceCreator: Calling repository.createSpace()');

      final space = await repository.createSpace(
        sessionToken: sessionToken,
        name: name,
        description: description,
        isPublic: isPublic,
      );

      debugPrint('SpaceCreator: Space created successfully - ${space.id}');
      state = AsyncValue.data(space);

      // Invalidate the spaces list to refetch
      ref.invalidate(spacesProvider);
      debugPrint('SpaceCreator: Invalidated spaces list');

      return space;
    } catch (error, stackTrace) {
      debugPrint('SpaceCreator: ERROR - $error');
      debugPrint('SpaceCreator: Stack trace: $stackTrace');
      state = AsyncValue.error(error, stackTrace);
      rethrow;
    }
  }
}

/// Create a new channel in a space
@riverpod
class ChannelCreator extends _$ChannelCreator {
  @override
  FutureOr<Channel?> build() {
    debugPrint('ChannelCreator: Initialized');
    return null;
  }

  Future<Channel> createChannel({
    required String spaceId,
    required String name,
    String? description,
    bool isPublic = true,
  }) async {
    debugPrint('ChannelCreator: Creating channel "$name" in space $spaceId');

    final sessionToken = ref.read(sessionTokenProvider);
    if (sessionToken == null) {
      debugPrint('ChannelCreator: ERROR - Not authenticated');
      throw Exception('Not authenticated');
    }

    state = const AsyncValue.loading();
    debugPrint('ChannelCreator: Set state to loading');

    try {
      debugPrint('ChannelCreator: Calling backend API to create channel');
      final repository = ref.read(spaceRepositoryProvider);
      final channel = await repository.createChannel(
        sessionToken: sessionToken,
        spaceId: spaceId,
        name: name,
        description: description,
        isPublic: isPublic,
      );

      debugPrint(
          'ChannelCreator: Channel created successfully - ${channel.id}');
      state = AsyncValue.data(channel);

      // Invalidate the channels list for this space
      ref.invalidate(spaceChannelsProvider(spaceId));
      debugPrint(
          'ChannelCreator: Invalidated channels list for space $spaceId');

      return channel;
    } catch (error, stackTrace) {
      debugPrint('ChannelCreator: ERROR - $error');
      debugPrint('ChannelCreator: Stack trace: $stackTrace');
      state = AsyncValue.error(error, stackTrace);
      rethrow;
    }
  }
}
