import 'package:flutter/material.dart';

/// Paleta de cores oficial do Catppuccin Mocha
/// https://catppuccin.com/palette
class CatppuccinMocha {
  static const Color rosewater = Color(0xFFf5e0dc);
  static const Color flamingo = Color(0xFFf2cdcd);
  static const Color pink = Color(0xFFf5c2e7);
  static const Color mauve = Color(0xFFcba6f7); // Acento roxo principal
  static const Color red = Color(0xFFf38ba8);
  static const Color maroon = Color(0xFFeba0ac);
  static const Color peach = Color(0xFFfab387);
  static const Color yellow = Color(0xFFf9e2af);
  static const Color green = Color(0xFFa6e3a1);
  static const Color teal = Color(0xFF94e2d5);
  static const Color sky = Color(0xFF89dceb);
  static const Color sapphire = Color(0xFF74c7ec);
  static const Color blue = Color(0xFF89b4fa); // Acento azul principal
  static const Color lavender = Color(0xFFb4befe);
  static const Color text = Color(0xFFcdd6f4);
  static const Color subtext1 = Color(0xFFbac2de);
  static const Color subtext0 = Color(0xFFa6adc8);
  static const Color overlay2 = Color(0xFF9399b2);
  static const Color overlay1 = Color(0xFF7f849c);
  static const Color overlay0 = Color(0xFF6c7086);
  static const Color surface2 = Color(0xFF585b70);
  static const Color surface1 = Color(0xFF45475a);
  static const Color surface0 = Color(0xFF313244);
  static const Color base = Color(0xFF1e1e2e); // Fundo principal
  static const Color mantle = Color(0xFF181825); // Fundo de painéis laterais
  static const Color crust = Color(0xFF11111b); // Fundo mais escuro
}

final ThemeData aetherTheme = ThemeData(
  useMaterial3: true,
  brightness: Brightness.dark,
  scaffoldBackgroundColor: CatppuccinMocha.base,
  colorScheme: const ColorScheme.dark(
    primary: CatppuccinMocha.mauve,
    secondary: CatppuccinMocha.blue,
    surface: CatppuccinMocha.base,
    onSurface: CatppuccinMocha.text,
    surfaceContainerHighest: CatppuccinMocha.surface0, // Substitui onSurfaceVariant no Material 3
  ),
  textTheme: const TextTheme(
    bodyLarge: TextStyle(color: CatppuccinMocha.text),
    bodyMedium: TextStyle(color: CatppuccinMocha.text),
    titleLarge: TextStyle(color: CatppuccinMocha.text, fontWeight: FontWeight.bold),
    titleMedium: TextStyle(color: CatppuccinMocha.text),
  ),
  dividerTheme: const DividerThemeData(
    color: CatppuccinMocha.surface0,
    thickness: 1,
  ),
  iconTheme: const IconThemeData(
    color: CatppuccinMocha.text,
  ),
);
