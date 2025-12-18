import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import '../../providers/space_providers.dart';

/// Dialog for creating a new channel in a space
class CreateChannelDialog extends ConsumerStatefulWidget {
  final String spaceId;
  final String spaceName;

  const CreateChannelDialog({
    required this.spaceId,
    required this.spaceName,
    super.key,
  });

  @override
  ConsumerState<CreateChannelDialog> createState() =>
      _CreateChannelDialogState();
}

class _CreateChannelDialogState extends ConsumerState<CreateChannelDialog> {
  final _formKey = GlobalKey<FormState>();
  final _nameController = TextEditingController();
  final _descriptionController = TextEditingController();
  bool _isPublic = true;
  bool _isLoading = false;

  @override
  void dispose() {
    _nameController.dispose();
    _descriptionController.dispose();
    super.dispose();
  }

  Future<void> _createChannel() async {
    if (!_formKey.currentState!.validate()) return;

    setState(() => _isLoading = true);

    try {
      debugPrint(
          'CreateChannelDialog: Creating channel "${_nameController.text}" in space ${widget.spaceId}');
      debugPrint(
          'CreateChannelDialog: Public: $_isPublic, Description: "${_descriptionController.text}"');

      await ref.read(channelCreatorProvider.notifier).createChannel(
            spaceId: widget.spaceId,
            name: _nameController.text,
            description: _descriptionController.text,
            isPublic: _isPublic,
          );

      debugPrint('CreateChannelDialog: Channel created successfully');

      // Refresh channels list
      ref.invalidate(spaceChannelsProvider(widget.spaceId));

      if (mounted) {
        Navigator.of(context).pop();
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(
            content: Text('Channel "${_nameController.text}" created!'),
            backgroundColor: Colors.green,
          ),
        );
      }
    } catch (e, stackTrace) {
      debugPrint('CreateChannelDialog: Error creating channel - $e');
      debugPrint('CreateChannelDialog: Stack trace: $stackTrace');

      if (mounted) {
        setState(() => _isLoading = false);
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(
            content: Text('Failed to create channel: $e'),
            backgroundColor: Colors.red,
          ),
        );
      }
    }
  }

  @override
  Widget build(BuildContext context) {
    return AlertDialog(
      title: Text('Create Channel in ${widget.spaceName}'),
      content: Form(
        key: _formKey,
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            TextFormField(
              controller: _nameController,
              decoration: const InputDecoration(
                labelText: 'Channel Name',
                hintText: 'general, announcements, etc.',
                prefixIcon: Icon(Icons.tag),
              ),
              validator: (value) {
                if (value == null || value.trim().isEmpty) {
                  return 'Please enter a channel name';
                }
                if (value.trim().length < 2) {
                  return 'Channel name must be at least 2 characters';
                }
                if (value.trim().length > 32) {
                  return 'Channel name must be less than 32 characters';
                }
                // Channel names should be lowercase with hyphens
                if (!RegExp(r'^[a-z0-9-]+$').hasMatch(value.trim())) {
                  return 'Use lowercase letters, numbers, and hyphens only';
                }
                return null;
              },
              autofocus: true,
              textCapitalization: TextCapitalization.none,
            ),
            const SizedBox(height: 16),
            TextFormField(
              controller: _descriptionController,
              decoration: const InputDecoration(
                labelText: 'Description (optional)',
                hintText: 'What is this channel about?',
                prefixIcon: Icon(Icons.description_outlined),
              ),
              maxLines: 3,
            ),
            const SizedBox(height: 16),
            SwitchListTile(
              title: const Text('Public Channel'),
              subtitle: Text(
                _isPublic
                    ? 'Everyone in the space can see this channel'
                    : 'Only invited members can see this channel',
              ),
              value: _isPublic,
              onChanged: _isLoading
                  ? null
                  : (value) {
                      setState(() => _isPublic = value);
                      debugPrint(
                          'CreateChannelDialog: Visibility changed to ${value ? "public" : "private"}');
                    },
            ),
          ],
        ),
      ),
      actions: [
        TextButton(
          onPressed: _isLoading
              ? null
              : () {
                  debugPrint('CreateChannelDialog: Cancelled');
                  Navigator.of(context).pop();
                },
          child: const Text('Cancel'),
        ),
        ElevatedButton(
          onPressed: _isLoading ? null : _createChannel,
          child: _isLoading
              ? const SizedBox(
                  width: 20,
                  height: 20,
                  child: CircularProgressIndicator(strokeWidth: 2),
                )
              : const Text('Create'),
        ),
      ],
    );
  }
}
