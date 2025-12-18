// This is a generated file - do not edit.
//
// Generated from spacepanda.proto.

// @dart = 3.3

// ignore_for_file: annotate_overrides, camel_case_types, comment_references
// ignore_for_file: constant_identifier_names
// ignore_for_file: curly_braces_in_flow_control_structures
// ignore_for_file: deprecated_member_use_from_same_package, library_prefixes
// ignore_for_file: non_constant_identifier_names, prefer_relative_imports

import 'dart:core' as $core;

import 'package:protobuf/protobuf.dart' as $pb;

class UserStatus extends $pb.ProtobufEnum {
  static const UserStatus USER_STATUS_UNSPECIFIED =
      UserStatus._(0, _omitEnumNames ? '' : 'USER_STATUS_UNSPECIFIED');
  static const UserStatus USER_STATUS_ONLINE =
      UserStatus._(1, _omitEnumNames ? '' : 'USER_STATUS_ONLINE');
  static const UserStatus USER_STATUS_IDLE =
      UserStatus._(2, _omitEnumNames ? '' : 'USER_STATUS_IDLE');
  static const UserStatus USER_STATUS_DND =
      UserStatus._(3, _omitEnumNames ? '' : 'USER_STATUS_DND');
  static const UserStatus USER_STATUS_OFFLINE =
      UserStatus._(4, _omitEnumNames ? '' : 'USER_STATUS_OFFLINE');

  static const $core.List<UserStatus> values = <UserStatus>[
    USER_STATUS_UNSPECIFIED,
    USER_STATUS_ONLINE,
    USER_STATUS_IDLE,
    USER_STATUS_DND,
    USER_STATUS_OFFLINE,
  ];

  static final $core.List<UserStatus?> _byValue =
      $pb.ProtobufEnum.$_initByValueList(values, 4);
  static UserStatus? valueOf($core.int value) =>
      value < 0 || value >= _byValue.length ? null : _byValue[value];

  const UserStatus._(super.value, super.name);
}

class SpaceVisibility extends $pb.ProtobufEnum {
  static const SpaceVisibility SPACE_VISIBILITY_UNSPECIFIED = SpaceVisibility._(
      0, _omitEnumNames ? '' : 'SPACE_VISIBILITY_UNSPECIFIED');
  static const SpaceVisibility SPACE_VISIBILITY_PUBLIC =
      SpaceVisibility._(1, _omitEnumNames ? '' : 'SPACE_VISIBILITY_PUBLIC');
  static const SpaceVisibility SPACE_VISIBILITY_PRIVATE =
      SpaceVisibility._(2, _omitEnumNames ? '' : 'SPACE_VISIBILITY_PRIVATE');

  static const $core.List<SpaceVisibility> values = <SpaceVisibility>[
    SPACE_VISIBILITY_UNSPECIFIED,
    SPACE_VISIBILITY_PUBLIC,
    SPACE_VISIBILITY_PRIVATE,
  ];

  static final $core.List<SpaceVisibility?> _byValue =
      $pb.ProtobufEnum.$_initByValueList(values, 2);
  static SpaceVisibility? valueOf($core.int value) =>
      value < 0 || value >= _byValue.length ? null : _byValue[value];

  const SpaceVisibility._(super.value, super.name);
}

class ChannelVisibility extends $pb.ProtobufEnum {
  static const ChannelVisibility CHANNEL_VISIBILITY_UNSPECIFIED =
      ChannelVisibility._(
          0, _omitEnumNames ? '' : 'CHANNEL_VISIBILITY_UNSPECIFIED');
  static const ChannelVisibility CHANNEL_VISIBILITY_PUBLIC =
      ChannelVisibility._(1, _omitEnumNames ? '' : 'CHANNEL_VISIBILITY_PUBLIC');
  static const ChannelVisibility CHANNEL_VISIBILITY_PRIVATE =
      ChannelVisibility._(
          2, _omitEnumNames ? '' : 'CHANNEL_VISIBILITY_PRIVATE');

  static const $core.List<ChannelVisibility> values = <ChannelVisibility>[
    CHANNEL_VISIBILITY_UNSPECIFIED,
    CHANNEL_VISIBILITY_PUBLIC,
    CHANNEL_VISIBILITY_PRIVATE,
  ];

  static final $core.List<ChannelVisibility?> _byValue =
      $pb.ProtobufEnum.$_initByValueList(values, 2);
  static ChannelVisibility? valueOf($core.int value) =>
      value < 0 || value >= _byValue.length ? null : _byValue[value];

  const ChannelVisibility._(super.value, super.name);
}

const $core.bool _omitEnumNames =
    $core.bool.fromEnvironment('protobuf.omit_enum_names');
