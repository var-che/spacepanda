import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import '../../providers/space_providers.dart';

class CreateSpaceDialog extends ConsumerStatefulWidget {
  const CreateSpaceDialog({super.key});

  @override
  ConsumerState<CreateSpaceDialog> createState() => _CreateSpaceDialogState();
}

class _CreateSpaceDialogState extends ConsumerState<CreateSpaceDialog> {
  final _formKey = GlobalKey<FormState>();
  final _nameController = TextEditingController();
  final _descriptionController = TextEditingController();
  bool _isPublic = false;
  bool _isLoading = false;

  @override
  void dispose() {
    _nameController.dispose();
    _descriptionController.dispose();
    super.dispose();
  }

  Future<void> _createSpace() async {
    if (!_formKey.currentState!.validate()) {
      debugPrint('CreateSpaceDialog: Form validation failed');
      return;
    }

    debugPrint(
        'CreateSpaceDialog: Starting space creation - name: "${_nameController.text.trim()}"');
    setState(() => _isLoading = true);

    try {
      debugPrint(
          'CreateSpaceDialog: Calling spaceCreatorProvider.createSpace()');
      await ref.read(spaceCreatorProvider.notifier).createSpace(
            name: _nameController.text.trim(),
            description: _descriptionController.text.trim().isEmpty
                ? null
                : _descriptionController.text.trim(),
            isPublic: _isPublic,
          );

      debugPrint(
          'CreateSpaceDialog: Space created successfully, closing dialog');
      if (mounted) {
        // Invalidate spaces provider to refresh the list
        ref.invalidate(spacesProvider);
        Navigator.of(context).pop();
      }
    } catch (e, stackTrace) {
      debugPrint('CreateSpaceDialog: ERROR - $e');
      debugPrint('CreateSpaceDialog: Stack trace - $stackTrace');
      if (mounted) {
        setState(() => _isLoading = false);
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(
            content: Text('Failed to create space: $e'),
            backgroundColor: Colors.red,
          ),
        );
      }
    }
  }

  @override
  Widget build(BuildContext context) {
    return AlertDialog(
      title: const Text('Create a Space'),
      content: Form(
        key: _formKey,
        child: SizedBox(
          width: 400,
          child: Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              const Text(
                'Create a new space for your community',
                style: TextStyle(fontSize: 14, color: Colors.grey),
              ),
              const SizedBox(height: 24),

              // Space name
              TextFormField(
                controller: _nameController,
                decoration: const InputDecoration(
                  labelText: 'Space Name',
                  hintText: 'My Awesome Space',
                  border: OutlineInputBorder(),
                ),
                validator: (value) {
                  if (value == null || value.trim().isEmpty) {
                    return 'Please enter a space name';
                  }
                  if (value.trim().length < 2) {
                    return 'Name must be at least 2 characters';
                  }
                  return null;
                },
                enabled: !_isLoading,
                autofocus: true,
              ),

              const SizedBox(height: 16),

              // Description (optional)
              TextFormField(
                controller: _descriptionController,
                decoration: const InputDecoration(
                  labelText: 'Description (optional)',
                  hintText: 'A place for awesome discussions',
                  border: OutlineInputBorder(),
                ),
                maxLines: 3,
                enabled: !_isLoading,
              ),

              const SizedBox(height: 16),

              // Public toggle
              SwitchListTile(
                title: const Text('Public Space'),
                subtitle: const Text(
                  'Anyone can discover and join this space',
                  style: TextStyle(fontSize: 12),
                ),
                value: _isPublic,
                onChanged: _isLoading
                    ? null
                    : (value) => setState(() => _isPublic = value),
              ),
            ],
          ),
        ),
      ),
      actions: [
        TextButton(
          onPressed: _isLoading ? null : () => Navigator.of(context).pop(),
          child: const Text('Cancel'),
        ),
        ElevatedButton(
          onPressed: _isLoading ? null : _createSpace,
          child: _isLoading
              ? const SizedBox(
                  width: 16,
                  height: 16,
                  child: CircularProgressIndicator(strokeWidth: 2),
                )
              : const Text('Create'),
        ),
      ],
    );
  }
}
