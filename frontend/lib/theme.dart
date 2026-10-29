import 'package:flutter/material.dart';

/// DESIGN.md §2 — colors. Softened revision:
/// warmer near-blacks, gentler layering, desaturated accent, radius everywhere.
class T {
  // Soft dark: muted plum-charcoal surfaces, desaturated lavender accent,
  // gentle pastel semantics. Calm at night, no glow.
  static const bg = Color(0xFF131218); // deep soft plum-black
  static const bgSidebar = Color(0xFF0F0E14);
  static const panel = Color(0xFF1A1921); // raised surface
  static const panel2 = Color(0xFF23212C); // interactive surface
  static const input = Color(0xFF17161D);
  static const hairline = Color(0xFF2B2936); // barely-there stroke
  static const hairlineStrong = Color(0xFF3A3748);
  static const fg = Color(0xFFECEAF3);
  static const fg2 = Color(0xFFB3AFC0);
  static const fg3 = Color(0xFF736F80);
  static const fg4 = Color(0xFF3B3847);
  static const accent = Color(0xFFB9A1F0); // soft lavender, less neon
  static const accentStrong = Color(0xFF9C82E0);
  static const up = Color(0xFF93D7AC); // soft sage green
  static const down = Color(0xFFF0A9A4); // dusty rose
  static const warn = Color(0xFFE6C98F); // soft honey
  static const info = Color(0xFFA2B9E8); // pale periwinkle
  static const gold = Color(0xFFE2C792);

  // Soft geometry
  static const radius = 12.0; // panels / big surfaces
  static const radiusSm = 9.0; // buttons / inputs / chips
  static const radiusXs = 6.0; // tiny accents

  static const mono = 'monospace';
  static const sans = 'sans-serif';
}

/// DESIGN.md §3 — typography helpers (softened weights/spacing).
class Type {
  static const labelCaps = TextStyle(
    fontFamily: T.mono,
    fontSize: 10.5,
    fontWeight: FontWeight.w600,
    letterSpacing: 0.16,
    color: T.fg3,
  );
  static const panelTitle = TextStyle(
    fontFamily: T.mono,
    fontSize: 11.5,
    fontWeight: FontWeight.w600,
    letterSpacing: 0.16,
    color: T.fg,
  );
  static const hero = TextStyle(
    fontFamily: T.mono,
    fontSize: 22,
    fontWeight: FontWeight.w600,
    color: T.fg,
  );
  static const data = TextStyle(
    fontFamily: T.mono,
    fontSize: 12.5,
    color: T.fg,
    fontFeatures: [FontFeature.tabularFigures()],
  );
  static const dataBold = TextStyle(
    fontFamily: T.mono,
    fontSize: 12.5,
    fontWeight: FontWeight.w700,
    color: T.fg,
    fontFeatures: [FontFeature.tabularFigures()],
  );
  static const dataDim = TextStyle(
    fontFamily: T.mono,
    fontSize: 12.5,
    color: T.fg3,
    fontFeatures: [FontFeature.tabularFigures()],
  );
  static const body = TextStyle(
    fontFamily: T.sans,
    fontSize: 12.5,
    color: T.fg2,
    height: 1.45,
    letterSpacing: 0.01,
  );
}

/// DESIGN.md §5 — status chip coloring.
class StatusTone {
  Color get color => switch (state.toLowerCase()) {
        'paper_trading' || 'running' || 'shadow' || 'qualified' || 'filled' || 'active' => T.up,
        'paused' || 'pending' || 'queued' || 'stale' || 'partial' || 'open' =>
          T.warn,
        'faulted' || 'disabled' || 'rejected' || 'cancelled' || 'canceled' ||
        'closed' || 'failed' =>
          T.down,
        'ready' => T.fg3,
        _ => T.info,
      };
  final String state;
  StatusTone(this.state);
}

/// Full app theme — soft dark terminal.
ThemeData buildTheme() {
  final base = ThemeData(
    brightness: Brightness.dark,
    useMaterial3: true,
    scaffoldBackgroundColor: T.bg,
    fontFamily: T.sans,
    colorScheme: const ColorScheme.dark(
      primary: T.accent,
      secondary: T.warn,
      error: T.down,
      surface: T.panel,
      onSurface: T.fg,
      onPrimary: T.bg,
      outline: T.hairline,
    ),
  );

  return base.copyWith(
    textTheme: base.textTheme.apply(
      bodyColor: T.fg,
      displayColor: T.fg,
    ),
    dividerTheme: const DividerThemeData(color: T.hairline, thickness: 1, space: 1),
    cardTheme: CardThemeData(
      color: T.panel,
      surfaceTintColor: Colors.transparent,
      elevation: 0,
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(T.radius),
      ),
      margin: EdgeInsets.zero,
    ),
    snackBarTheme: SnackBarThemeData(
      backgroundColor: T.panel2,
      contentTextStyle: Type.data,
      shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(T.radiusSm)),
      behavior: SnackBarBehavior.floating,
    ),
    inputDecorationTheme: InputDecorationTheme(
      isDense: true,
      filled: true,
      fillColor: T.input,
      contentPadding: const EdgeInsets.symmetric(horizontal: 10, vertical: 9),
      hintStyle: Type.dataDim,
      labelStyle: Type.labelCaps,
      border: OutlineInputBorder(
        borderRadius: BorderRadius.circular(T.radiusSm),
        borderSide: const BorderSide(color: T.hairline),
      ),
      enabledBorder: OutlineInputBorder(
        borderRadius: BorderRadius.circular(T.radiusSm),
        borderSide: const BorderSide(color: T.hairline),
      ),
      focusedBorder: OutlineInputBorder(
        borderRadius: BorderRadius.circular(T.radiusSm),
        borderSide: const BorderSide(color: T.accent, width: 1.1),
      ),
    ),
    elevatedButtonTheme: ElevatedButtonThemeData(
      style: ElevatedButton.styleFrom(
        backgroundColor: T.accent,
        foregroundColor: T.bg,
        elevation: 0,
        minimumSize: const Size(0, 32),
        padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 4),
        shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(T.radiusSm)),
        textStyle: Type.labelCaps.copyWith(color: T.bg, letterSpacing: 0.12),
      ),
    ),
    outlinedButtonTheme: OutlinedButtonThemeData(
      style: OutlinedButton.styleFrom(
        foregroundColor: T.fg,
        backgroundColor: T.panel2,
        side: const BorderSide(color: T.hairline),
        minimumSize: const Size(0, 32),
        padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 4),
        shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(T.radiusSm)),
        textStyle: Type.labelCaps.copyWith(letterSpacing: 0.12),
      ),
    ),
    textButtonTheme: TextButtonThemeData(
      style: TextButton.styleFrom(
        foregroundColor: T.accent,
        textStyle: Type.labelCaps,
        shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(T.radiusSm)),
        minimumSize: const Size(0, 32),
        padding: const EdgeInsets.symmetric(horizontal: 10),
      ),
    ),
    scrollbarTheme: ScrollbarThemeData(
      thumbColor: WidgetStatePropertyAll(T.fg4),
      thumbVisibility: const WidgetStatePropertyAll(false),
      radius: const Radius.circular(4),
    ),
    tooltipTheme: TooltipThemeData(
      textStyle: Type.data.copyWith(fontSize: 11, color: T.fg),
      decoration: BoxDecoration(
        color: T.panel2,
        borderRadius: BorderRadius.circular(T.radiusSm),
        border: Border.all(color: T.hairline),
      ),
    ),
    dataTableTheme: DataTableThemeData(
      headingTextStyle: Type.labelCaps,
      dataTextStyle: Type.data,
      headingRowColor: const WidgetStatePropertyAll(T.bg),
      headingRowHeight: 30,
      dataRowMinHeight: 28,
      dataRowMaxHeight: 30,
      horizontalMargin: 10,
      dividerThickness: 0,
    ),
  );
}

/// Terminal button — compact, rounded, caps mono.
class TermButton extends StatelessWidget {
  const TermButton({
    super.key,
    required this.label,
    this.onPressed,
    this.color,
    this.icon,
    this.tooltip,
  });

  final String label;
  final VoidCallback? onPressed;
  final Color? color;
  final IconData? icon;
  final String? tooltip;

  @override
  Widget build(BuildContext context) {
    final useAccent = color == null;
    final fg = useAccent ? T.bg : color;
    final style = ElevatedButton.styleFrom(
      backgroundColor: color ?? T.accent,
      foregroundColor: fg,
      elevation: 0,
      minimumSize: const Size(0, 30),
      padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 4),
      shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(T.radiusSm)),
      textStyle: Type.labelCaps.copyWith(color: fg, letterSpacing: 0.12),
      disabledBackgroundColor: T.fg4,
      disabledForegroundColor: T.fg3,
    );
    final btn = ElevatedButton.icon(
      style: style,
      onPressed: onPressed,
      icon: icon == null ? const SizedBox.shrink() : Icon(icon, size: 13),
      label: Padding(
        padding: const EdgeInsets.only(left: 3),
        child: Text(label),
      ),
    );
    if (tooltip == null) return btn;
    return Tooltip(message: tooltip!, child: btn);
  }
}