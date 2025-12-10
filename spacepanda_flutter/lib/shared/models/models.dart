import 'package:freezed_annotation/freezed_annotation.dart';

part 'models.freezed.dart';
part 'models.g.dart';

/// User model
@freezed
class User with _$User {
  const factory User({
    required String id,
    required String username,
    required String displayName,
    String? avatarUrl,
    @Default(UserStatus.online) UserStatus status,
  }) = _User;

  factory User.fromJson(Map<String, dynamic> json) => _$UserFrom Json(json);
}

enum UserStatus { online, idle, dnd, offline }

/// Space model (like Discord server)
@freezed
class Space with _$Space {
  const factory Space({
    required String id,
    required String name,
    String? description,
    String? iconUrl,
    required SpaceVisibility visibility,
    required String ownerId,
    required List<String> memberIds,
    required List<String> channelIds,
    required DateTime createdAt,
  }) = _Space;

  factory Space.fromJson(Map<String, dynamic> json) => _$SpaceFromJson(json);
}

enum SpaceVisibility { public, private }

/// Channel model
@freezed
class Channel with _$Channel {
  const factory Channel({
    required String id,
    required String spaceId,
    required String name,
    String? description,
    required ChannelVisibility visibility,
    required List<String> memberIds,
    required DateTime createdAt,
  }) = _Channel;

  factory Channel.fromJson(Map<String, dynamic> json) => _$ChannelFromJson(json);
}

enum ChannelVisibility { public, private }

/// Message model
@freezed
class Message with _$Message {
  const factory Message({
    required String id,
    required String channelId,
    required String senderId,
    required String content,
    required DateTime timestamp,
    @Default(false) bool isEncrypted,
    @Default([]) List<String> attachments,
  }) = _Message;

  factory Message.fromJson(Map<String, dynamic> json) => _$MessageFromJson(json);
}

/// Space invite model
@freezed
class SpaceInvite with _$SpaceInvite {
  const factory SpaceInvite({
    required String id,
    required String spaceId,
    required String code,
    required String createdBy,
    int? maxUses,
    int? useCount,
    DateTime? expiresAt,
    @Default(false) bool revoked,
  }) = _SpaceInvite;

  factory SpaceInvite.fromJson(Map<String, dynamic> json) => _$SpaceInviteFromJson(json);
}
