import 'package:flutter/material.dart';

class AppTheme {
  // Discord-like color palette
  static const _darkBackground = Color(0xFF36393F);
  static const _darkerBackground = Color(0xFF2F3136);
  static const _darkestBackground = Color(0xFF202225);
  static const _channelHover = Color(0xFF3A3C43);
  static const _accentColor = Color(0xFF5865F2); // Discord Blurple
  static const _successColor = Color(0xFF3BA55D);
  static const _dangerColor = Color(0xFFED4245);
  static const _textPrimary = Color(0xFFDCDDDE);
  static const _textSecondary = Color(0xFFB9BBBE);
  static const _textMuted = Color(0xFF72767D);

  static ThemeData get darkTheme {
    return ThemeData(
      useMaterial3: true,
      brightness: Brightness.dark,
      scaffoldBackgroundColor: _darkBackground,

      colorScheme: const ColorScheme.dark(
        primary: _accentColor,
        secondary: _accentColor,
        surface: _darkerBackground,
        error: _dangerColor,
      ),

      // App Bar
      appBarTheme: const AppBarTheme(
        backgroundColor: _darkestBackground,
        elevation: 0,
        centerTitle: false,
        titleTextStyle: TextStyle(
          color: _textPrimary,
          fontSize: 16,
          fontWeight: FontWeight.w600,
        ),
      ),

      // Text Theme
      textTheme: const TextTheme(
        displayLarge:
            TextStyle(color: _textPrimary, fontWeight: FontWeight.bold),
        displayMedium:
            TextStyle(color: _textPrimary, fontWeight: FontWeight.bold),
        displaySmall:
            TextStyle(color: _textPrimary, fontWeight: FontWeight.bold),
        headlineMedium:
            TextStyle(color: _textPrimary, fontWeight: FontWeight.w600),
        titleLarge: TextStyle(color: _textPrimary, fontWeight: FontWeight.w600),
        titleMedium:
            TextStyle(color: _textPrimary, fontWeight: FontWeight.w500),
        bodyLarge: TextStyle(color: _textPrimary),
        bodyMedium: TextStyle(color: _textSecondary),
        bodySmall: TextStyle(color: _textMuted),
      ),

      // Card
      cardTheme: CardThemeData(
        color: _darkerBackground,
        elevation: 0,
        shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(8)),
      ),

      // Input Decoration
      inputDecorationTheme: InputDecorationTheme(
        filled: true,
        fillColor: _darkestBackground,
        border: OutlineInputBorder(
          borderRadius: BorderRadius.circular(8),
          borderSide: BorderSide.none,
        ),
        contentPadding:
            const EdgeInsets.symmetric(horizontal: 16, vertical: 12),
      ),

      // Icon Theme
      iconTheme: const IconThemeData(color: _textSecondary),

      // Divider
      dividerTheme: const DividerThemeData(
        color: _darkestBackground,
        thickness: 1,
        space: 0,
      ),

      // List Tile
      listTileTheme: const ListTileThemeData(
        selectedColor: _textPrimary,
        selectedTileColor: _channelHover,
        iconColor: _textSecondary,
        textColor: _textSecondary,
      ),

      // Floating Action Button
      floatingActionButtonTheme: const FloatingActionButtonThemeData(
        backgroundColor: _accentColor,
        foregroundColor: Colors.white,
      ),

      // Elevated Button
      elevatedButtonTheme: ElevatedButtonThemeData(
        style: ElevatedButton.styleFrom(
          backgroundColor: _accentColor,
          foregroundColor: Colors.white,
          elevation: 0,
          padding: const EdgeInsets.symmetric(horizontal: 24, vertical: 12),
          shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(4)),
        ),
      ),

      // Text Button
      textButtonTheme: TextButtonThemeData(
        style: TextButton.styleFrom(
          foregroundColor: _accentColor,
        ),
      ),
    );
  }

  static ThemeData get lightTheme {
    return ThemeData(
      useMaterial3: true,
      brightness: Brightness.light,
      colorScheme: ColorScheme.light(
        primary: _accentColor,
        secondary: _accentColor,
        surface: Colors.grey[100]!,
        error: _dangerColor,
      ),
    );
  }

  // Custom colors for specific use cases
  static const darkBackground = _darkBackground;
  static const channelHover = _channelHover;
  static const darkestBackground = _darkestBackground;
  static const darkerBackground = _darkerBackground;
  static const accentColor = _accentColor;
  static const successColor = _successColor;
  static const dangerColor = _dangerColor;
  static const textMuted = _textMuted;
}
