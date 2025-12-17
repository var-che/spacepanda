import 'package:flutter/material.dart';
import '../../core/theme/app_theme.dart';
import '../../shared/models/models.dart';
import 'create_space_dialog.dart';

class SpacesSidebar extends StatelessWidget {
  final List<Space> spaces;
  final Space? selectedSpace;
  final Function(Space) onSpaceSelected;

  const SpacesSidebar({
    super.key,
    required this.spaces,
    required this.selectedSpace,
    required this.onSpaceSelected,
  });

  @override
  Widget build(BuildContext context) {
    return Container(
      width: 72,
      color: AppTheme.darkestBackground,
      child: Column(
        children: [
          const SizedBox(height: 12),
          // Home button
          _SpaceIcon(
            label: 'Home',
            icon: Icons.home,
            isSelected: selectedSpace == null,
            onTap: () {},
          ),
          const Padding(
            padding: EdgeInsets.symmetric(horizontal: 16, vertical: 8),
            child: Divider(height: 1),
          ),
          // Spaces list
          Expanded(
            child: ListView.builder(
              padding: const EdgeInsets.symmetric(vertical: 8),
              itemCount: spaces.length,
              itemBuilder: (context, index) {
                final space = spaces[index];
                return _SpaceIcon(
                  label: space.name,
                  iconUrl: space.iconUrl,
                  fallbackText: space.name.substring(0, 1).toUpperCase(),
                  isSelected: selectedSpace?.id == space.id,
                  onTap: () => onSpaceSelected(space),
                );
              },
            ),
          ),
          // Add server button
          _SpaceIcon(
            label: 'Add a Space',
            icon: Icons.add,
            isSelected: false,
            onTap: () {
              showDialog(
                context: context,
                builder: (context) => const CreateSpaceDialog(),
              );
            },
          ),
          const SizedBox(height: 12),
        ],
      ),
    );
  }
}

class _SpaceIcon extends StatefulWidget {
  final String label;
  final IconData? icon;
  final String? iconUrl;
  final String? fallbackText;
  final bool isSelected;
  final VoidCallback onTap;

  const _SpaceIcon({
    required this.label,
    this.icon,
    this.iconUrl,
    this.fallbackText,
    required this.isSelected,
    required this.onTap,
  });

  @override
  State<_SpaceIcon> createState() => _SpaceIconState();
}

class _SpaceIconState extends State<_SpaceIcon> {
  bool _isHovered = false;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.only(bottom: 8),
      child: Tooltip(
        message: widget.label,
        preferBelow: false,
        verticalOffset: 24,
        child: MouseRegion(
          onEnter: (_) => setState(() => _isHovered = true),
          onExit: (_) => setState(() => _isHovered = false),
          child: GestureDetector(
            onTap: widget.onTap,
            child: Center(
              child: Stack(
                alignment: Alignment.centerLeft,
                children: [
                  // Selection indicator
                  if (widget.isSelected || _isHovered)
                    Container(
                      width: 4,
                      height: widget.isSelected ? 40 : 20,
                      decoration: const BoxDecoration(
                        color: Colors.white,
                        borderRadius: BorderRadius.only(
                          topRight: Radius.circular(4),
                          bottomRight: Radius.circular(4),
                        ),
                      ),
                    ),
                  // Icon
                  Padding(
                    padding: const EdgeInsets.only(left: 12),
                    child: AnimatedContainer(
                      duration: const Duration(milliseconds: 200),
                      width: 48,
                      height: 48,
                      decoration: BoxDecoration(
                        color: widget.isSelected
                            ? AppTheme.accentColor
                            : _isHovered
                                ? AppTheme.accentColor
                                : AppTheme.darkerBackground,
                        borderRadius: BorderRadius.circular(
                          widget.isSelected || _isHovered ? 16 : 24,
                        ),
                      ),
                      child: widget.icon != null
                          ? Icon(
                              widget.icon,
                              color: widget.isSelected || _isHovered
                                  ? Colors.white
                                  : AppTheme.successColor,
                              size: 24,
                            )
                          : Center(
                              child: Text(
                                widget.fallbackText ?? '',
                                style: const TextStyle(
                                  color: Colors.white,
                                  fontSize: 18,
                                  fontWeight: FontWeight.w600,
                                ),
                              ),
                            ),
                    ),
                  ),
                ],
              ),
            ),
          ),
        ),
      ),
    );
  }
}
